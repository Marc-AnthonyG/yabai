use crate::service::plist::{populate_plist_path, service_install_internal};
use crate::support::filesystem::file_exists;
use crate::{error, warn};

const _PATH_LAUNCHCTL: &str = "/bin/launchctl";
const _NAME_YABAI_PLIST: &str = "com.asmvik.yabai";

//
// NOTE(asmvik): A launchd service has the following states:
//
//          1. Installed / Uninstalled
//          2. Active (Enable / Disable)
//          3. Bootstrapped (Load / Unload)
//          4. Running (Start / Stop)
//

fn safe_exec(arguments: &[&str], suppress_output: bool) -> i32 {
    let argument_strings: Vec<std::ffi::CString> = arguments
        .iter()
        .map(|argument| std::ffi::CString::new(*argument).unwrap())
        .collect();
    let mut argument_pointers: Vec<*mut libc::c_char> = argument_strings
        .iter()
        .map(|argument| argument.as_ptr() as *mut libc::c_char)
        .collect();
    argument_pointers.push(core::ptr::null_mut());

    let mut process_id: libc::pid_t = 0;
    let mut actions: libc::posix_spawn_file_actions_t = unsafe { std::mem::zeroed() };
    unsafe { libc::posix_spawn_file_actions_init(&mut actions) };

    if suppress_output {
        let dev_null = std::ffi::CString::new("/dev/null").unwrap();
        unsafe {
            libc::posix_spawn_file_actions_addopen(
                &mut actions,
                libc::STDOUT_FILENO,
                dev_null.as_ptr(),
                libc::O_WRONLY | libc::O_APPEND,
                0,
            );
            libc::posix_spawn_file_actions_addopen(
                &mut actions,
                libc::STDERR_FILENO,
                dev_null.as_ptr(),
                libc::O_WRONLY | libc::O_APPEND,
                0,
            );
        }
    }

    let mut status: libc::c_int = unsafe {
        libc::posix_spawn(
            &mut process_id,
            argument_pointers[0],
            &actions,
            core::ptr::null(),
            argument_pointers.as_ptr(),
            core::ptr::null(),
        )
    };
    if status != 0 {
        return 1;
    }

    while unsafe { libc::waitpid(process_id, &mut status, 0) } == -1
        && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
    {
        unsafe { libc::usleep(1000) };
    }

    if libc::WIFSIGNALED(status) {
        1
    } else if libc::WIFSTOPPED(status) {
        1
    } else {
        libc::WEXITSTATUS(status)
    }
}

pub fn service_start() -> i32 {
    let yabai_plist_path = populate_plist_path();
    if !file_exists(&yabai_plist_path) {
        warn!(
            "yabai: service file '{}' is not installed! attempting installation..\n",
            yabai_plist_path
        );

        let result = service_install_internal(&yabai_plist_path);
        if result != 0 {
            error!(
                "yabai: service file '{}' could not be installed! abort..\n",
                yabai_plist_path
            );
        }
    }

    let service_target = format!(
        "gui/{}/{}",
        unsafe { libc::getuid() } as i32,
        _NAME_YABAI_PLIST
    );

    let domain_target = format!("gui/{}", unsafe { libc::getuid() } as i32);

    //
    // NOTE(asmvik): Check if service is bootstrapped
    //

    let print_arguments = [_PATH_LAUNCHCTL, "print", &service_target];
    let is_bootstrapped = safe_exec(&print_arguments, true);

    if is_bootstrapped != 0 {
        //
        // NOTE(asmvik): Service is not bootstrapped and could be disabled.
        // There is no way to query if the service is disabled, and we cannot
        // bootstrap a disabled service. Try to enable the service. This will be
        // a no-op if the service is already enabled.
        //

        let enable_arguments = [_PATH_LAUNCHCTL, "enable", &service_target];
        safe_exec(&enable_arguments, false);

        //
        // NOTE(asmvik): Bootstrap service into the target domain.
        // This will also start the program **iff* RunAtLoad is set to true.
        //

        let bootstrap_arguments = [
            _PATH_LAUNCHCTL,
            "bootstrap",
            &domain_target,
            &yabai_plist_path,
        ];
        safe_exec(&bootstrap_arguments, false)
    } else {
        //
        // NOTE(asmvik): The service has already been bootstrapped.
        // Tell the bootstrapped service to launch immediately; it is an
        // error to bootstrap a service that has already been bootstrapped.
        //

        let kickstart_arguments = [_PATH_LAUNCHCTL, "kickstart", &service_target];
        safe_exec(&kickstart_arguments, false)
    }
}

pub fn service_restart() -> i32 {
    let yabai_plist_path = populate_plist_path();
    if !file_exists(&yabai_plist_path) {
        error!(
            "yabai: service file '{}' is not installed! abort..\n",
            yabai_plist_path
        );
    }

    let service_target = format!(
        "gui/{}/{}",
        unsafe { libc::getuid() } as i32,
        _NAME_YABAI_PLIST
    );

    let kickstart_arguments = [_PATH_LAUNCHCTL, "kickstart", "-k", &service_target];
    safe_exec(&kickstart_arguments, false)
}

pub fn service_stop() -> i32 {
    let yabai_plist_path = populate_plist_path();
    if !file_exists(&yabai_plist_path) {
        error!(
            "yabai: service file '{}' is not installed! abort..\n",
            yabai_plist_path
        );
    }

    let service_target = format!(
        "gui/{}/{}",
        unsafe { libc::getuid() } as i32,
        _NAME_YABAI_PLIST
    );

    let domain_target = format!("gui/{}", unsafe { libc::getuid() } as i32);

    //
    // NOTE(asmvik): Check if service is bootstrapped
    //

    let print_arguments = [_PATH_LAUNCHCTL, "print", &service_target];
    let is_bootstrapped = safe_exec(&print_arguments, true);

    if is_bootstrapped != 0 {
        //
        // NOTE(asmvik): Service is not bootstrapped, but the program
        // could still be running an instance that was started **while the service
        // was bootstrapped**, so we tell it to stop said service.
        //

        let kill_arguments = [_PATH_LAUNCHCTL, "kill", "SIGTERM", &service_target];
        safe_exec(&kill_arguments, false)
    } else {
        //
        // NOTE(asmvik): Service is bootstrapped; we stop a potentially
        // running instance of the program and unload the service, making it
        // not trigger automatically in the future.
        //
        // This is NOT the same as disabling the service, which will prevent
        // it from being boostrapped in the future (without explicitly re-enabling
        // it first).
        //

        let bootout_arguments = [
            _PATH_LAUNCHCTL,
            "bootout",
            &domain_target,
            &yabai_plist_path,
        ];
        safe_exec(&bootout_arguments, false);

        let disable_arguments = [_PATH_LAUNCHCTL, "disable", &service_target];
        safe_exec(&disable_arguments, false)
    }
}
