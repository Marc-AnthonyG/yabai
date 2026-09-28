use core::ffi::{c_char, c_short};
use core::ptr::{null, null_mut};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub enum SpawnedProgramOutput {
    DiscardedIntoDevNull,
    SharedWithThisProcess,
}

pub fn run_program_and_read_its_exit_status_even_while_child_exits_are_ignored(
    program_path: &Path,
    arguments_after_the_program_path: &[&str],
    spawned_program_output: SpawnedProgramOutput,
) -> Option<i32> {
    let process_id = spawn_program_suspended(
        program_path,
        arguments_after_the_program_path,
        spawned_program_output,
    )?;
    let exit_status = resume_the_suspended_program_and_wait_for_its_exit_status(process_id);
    reap_the_exited_program_unless_the_system_already_did(process_id);
    exit_status
}

fn spawn_program_suspended(
    program_path: &Path,
    arguments_after_the_program_path: &[&str],
    spawned_program_output: SpawnedProgramOutput,
) -> Option<libc::pid_t> {
    let program_path = CString::new(program_path.as_os_str().as_bytes()).ok()?;
    let mut argument_strings = vec![program_path.clone()];
    for argument in arguments_after_the_program_path {
        argument_strings.push(CString::new(*argument).ok()?);
    }
    let argument_pointers = null_terminated_argument_pointers(&argument_strings);

    let mut attributes: libc::posix_spawnattr_t = unsafe { std::mem::zeroed() };
    unsafe { libc::posix_spawnattr_init(&mut attributes) };
    unsafe {
        libc::posix_spawnattr_setflags(
            &mut attributes,
            libc::POSIX_SPAWN_START_SUSPENDED as c_short,
        )
    };

    let mut file_actions: libc::posix_spawn_file_actions_t = unsafe { std::mem::zeroed() };
    unsafe { libc::posix_spawn_file_actions_init(&mut file_actions) };
    if let SpawnedProgramOutput::DiscardedIntoDevNull = spawned_program_output {
        send_standard_output_and_error_into_dev_null(&mut file_actions);
    }

    let mut process_id: libc::pid_t = 0;
    let spawn_result = unsafe {
        libc::posix_spawn(
            &mut process_id,
            program_path.as_ptr(),
            &file_actions,
            &attributes,
            argument_pointers.as_ptr(),
            null(),
        )
    };
    unsafe { libc::posix_spawn_file_actions_destroy(&mut file_actions) };
    unsafe { libc::posix_spawnattr_destroy(&mut attributes) };

    (spawn_result == 0).then_some(process_id)
}

fn null_terminated_argument_pointers(argument_strings: &[CString]) -> Vec<*mut c_char> {
    let mut argument_pointers: Vec<*mut c_char> = argument_strings
        .iter()
        .map(|argument| argument.as_ptr() as *mut c_char)
        .collect();
    argument_pointers.push(null_mut());
    argument_pointers
}

fn send_standard_output_and_error_into_dev_null(
    file_actions: &mut libc::posix_spawn_file_actions_t,
) {
    for standard_stream in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        unsafe {
            libc::posix_spawn_file_actions_addopen(
                file_actions,
                standard_stream,
                c"/dev/null".as_ptr(),
                libc::O_WRONLY | libc::O_APPEND,
                0,
            )
        };
    }
}

fn resume_the_suspended_program_and_wait_for_its_exit_status(
    process_id: libc::pid_t,
) -> Option<i32> {
    let event_queue = unsafe { libc::kqueue() };
    let is_watching_for_the_exit =
        event_queue != -1 && start_watching_for_the_exit_of_the_program(event_queue, process_id);
    unsafe { libc::kill(process_id, libc::SIGCONT) };

    let exit_status = if is_watching_for_the_exit {
        wait_for_the_exit_status_of_the_watched_program(event_queue)
    } else {
        None
    };
    if event_queue != -1 {
        unsafe { libc::close(event_queue) };
    }
    exit_status
}

fn start_watching_for_the_exit_of_the_program(
    event_queue: libc::c_int,
    process_id: libc::pid_t,
) -> bool {
    let exit_watch = libc::kevent {
        ident: process_id as libc::uintptr_t,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_ONESHOT,
        fflags: libc::NOTE_EXIT | libc::NOTE_EXITSTATUS,
        data: 0,
        udata: null_mut(),
    };
    let registration_result =
        unsafe { libc::kevent(event_queue, &exit_watch, 1, null_mut(), 0, null()) };
    registration_result == 0
}

fn wait_for_the_exit_status_of_the_watched_program(event_queue: libc::c_int) -> Option<i32> {
    let mut exit_event: libc::kevent = unsafe { std::mem::zeroed() };
    loop {
        let event_count =
            unsafe { libc::kevent(event_queue, null(), 0, &mut exit_event, 1, null()) };
        if event_count == 1 {
            break;
        }
        if event_count == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
        {
            continue;
        }
        return None;
    }

    if exit_event.fflags & libc::NOTE_EXIT == 0 {
        return None;
    }
    let wait_status = exit_event.data as libc::c_int;
    libc::WIFEXITED(wait_status).then(|| libc::WEXITSTATUS(wait_status))
}

fn reap_the_exited_program_unless_the_system_already_did(process_id: libc::pid_t) {
    unsafe { libc::waitpid(process_id, null_mut(), libc::WNOHANG) };
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        SpawnedProgramOutput,
        run_program_and_read_its_exit_status_even_while_child_exits_are_ignored,
    };

    fn exit_status_of_shell_command(shell_command: &str) -> Option<i32> {
        run_program_and_read_its_exit_status_even_while_child_exits_are_ignored(
            Path::new("/bin/sh"),
            &["-c", shell_command],
            SpawnedProgramOutput::DiscardedIntoDevNull,
        )
    }

    #[test]
    fn the_exit_status_a_program_ends_with_is_read_back_unchanged() {
        let expected_exit_statuses = [
            ("exit 0", Some(0)),
            ("exit 1", Some(1)),
            ("exit 42", Some(42)),
            ("exit 255", Some(255)),
        ];

        for (shell_command, expected_exit_status) in expected_exit_statuses {
            assert_eq!(
                exit_status_of_shell_command(shell_command),
                expected_exit_status,
                "{shell_command}"
            );
        }
    }

    #[test]
    fn the_exit_status_is_still_read_while_this_process_ignores_child_exits() {
        let previous_child_exit_disposition = unsafe { libc::signal(libc::SIGCHLD, libc::SIG_IGN) };
        let exit_status = exit_status_of_shell_command("exit 1");
        unsafe { libc::signal(libc::SIGCHLD, previous_child_exit_disposition) };

        assert_eq!(exit_status, Some(1));
    }

    #[test]
    fn a_program_ended_by_a_signal_has_no_exit_status() {
        assert_eq!(exit_status_of_shell_command("kill -KILL $$"), None);
    }

    #[test]
    fn a_program_that_cannot_be_spawned_has_no_exit_status() {
        assert_eq!(
            run_program_and_read_its_exit_status_even_while_child_exits_are_ignored(
                Path::new("/nonexistent/yabai-test-program"),
                &[],
                SpawnedProgramOutput::DiscardedIntoDevNull,
            ),
            None
        );
    }
}
