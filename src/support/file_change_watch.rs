use core::ptr::{null, null_mut};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;

const CHANGES_TO_THE_CONTENTS_OR_THE_NAME_OF_THE_FILE: u32 = libc::NOTE_WRITE
    | libc::NOTE_EXTEND
    | libc::NOTE_DELETE
    | libc::NOTE_RENAME
    | libc::NOTE_REVOKE;
const CHANGE_EVENTS_READ_AT_ONCE: usize = 8;

pub struct FileChangeWatch {
    event_queue: OwnedFd,
    _watched_file: File,
}

impl FileChangeWatch {
    pub fn start_watching_the_file(file_path: &Path) -> io::Result<FileChangeWatch> {
        let watched_file = open_only_to_be_told_about_changes(file_path)?;
        let event_queue = create_event_queue()?;
        register_the_vnode_change_watch(&event_queue, &watched_file)?;

        Ok(FileChangeWatch {
            event_queue,
            _watched_file: watched_file,
        })
    }

    pub fn wait_for_a_change_then_until_changes_stop_for(
        &self,
        quiet_period_that_ends_the_changes: Duration,
    ) -> io::Result<()> {
        count_change_events_arriving_within(&self.event_queue, None)?;
        while count_change_events_arriving_within(
            &self.event_queue,
            Some(quiet_period_that_ends_the_changes),
        )? > 0
        {}
        Ok(())
    }
}

fn open_only_to_be_told_about_changes(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_EVTONLY)
        .open(path)
}

fn create_event_queue() -> io::Result<OwnedFd> {
    let event_queue = unsafe { libc::kqueue() };
    if event_queue == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(event_queue) })
}

fn register_the_vnode_change_watch(event_queue: &OwnedFd, watched_file: &File) -> io::Result<()> {
    let registration = libc::kevent {
        ident: watched_file.as_raw_fd() as libc::uintptr_t,
        filter: libc::EVFILT_VNODE,
        flags: libc::EV_ADD | libc::EV_CLEAR,
        fflags: CHANGES_TO_THE_CONTENTS_OR_THE_NAME_OF_THE_FILE,
        data: 0,
        udata: null_mut(),
    };
    let registration_result = unsafe {
        libc::kevent(
            event_queue.as_raw_fd(),
            &registration,
            1,
            null_mut(),
            0,
            null(),
        )
    };
    if registration_result == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn count_change_events_arriving_within(
    event_queue: &OwnedFd,
    timeout: Option<Duration>,
) -> io::Result<usize> {
    let mut received_events: [libc::kevent; CHANGE_EVENTS_READ_AT_ONCE] =
        unsafe { std::mem::zeroed() };
    let timeout = timeout.map(|timeout| libc::timespec {
        tv_sec: timeout.as_secs() as libc::time_t,
        tv_nsec: timeout.subsec_nanos() as libc::c_long,
    });
    let timeout_pointer = timeout
        .as_ref()
        .map_or(null(), |timeout| timeout as *const libc::timespec);

    loop {
        let event_count = unsafe {
            libc::kevent(
                event_queue.as_raw_fd(),
                null(),
                0,
                received_events.as_mut_ptr(),
                CHANGE_EVENTS_READ_AT_ONCE as libc::c_int,
                timeout_pointer,
            )
        };
        if event_count >= 0 {
            return Ok(event_count as usize);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use super::FileChangeWatch;

    const QUIET_PERIOD_FOR_TESTS: Duration = Duration::from_millis(50);
    const LONGEST_WAIT_BEFORE_A_SEEN_CHANGE_COUNTS_AS_MISSED: Duration = Duration::from_secs(5);

    struct RemoveDirectoryOnDrop(PathBuf);

    impl Drop for RemoveDirectoryOnDrop {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn create_empty_directory_for_the_test(test_name: &str) -> RemoveDirectoryOnDrop {
        let directory = std::env::temp_dir().join(format!(
            "yabai-file-change-watch-{}-{test_name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        RemoveDirectoryOnDrop(std::fs::canonicalize(directory).unwrap())
    }

    fn does_the_wait_end_after(watched_path: &Path, change_to_make: impl FnOnce()) -> bool {
        let file_change_watch = FileChangeWatch::start_watching_the_file(watched_path).unwrap();
        let (wait_ended_sender, wait_ended_receiver) = channel();
        std::thread::spawn(move || {
            let wait_result = file_change_watch
                .wait_for_a_change_then_until_changes_stop_for(QUIET_PERIOD_FOR_TESTS);
            let _ = wait_ended_sender.send(wait_result.is_ok());
        });

        change_to_make();

        wait_ended_receiver
            .recv_timeout(LONGEST_WAIT_BEFORE_A_SEEN_CHANGE_COUNTS_AS_MISSED)
            .unwrap_or(false)
    }

    #[test]
    fn writing_the_file_in_place_ends_the_wait() {
        let directory = create_empty_directory_for_the_test("in-place");
        let watched_file = directory.0.join("yabairc");
        std::fs::write(&watched_file, "before").unwrap();

        let wait_ended = does_the_wait_end_after(&watched_file, || {
            std::fs::write(&watched_file, "after").unwrap()
        });

        assert!(wait_ended);
    }

    #[test]
    fn saving_by_renaming_a_new_file_over_the_watched_one_ends_the_wait() {
        let directory = create_empty_directory_for_the_test("rename-over");
        let watched_file = directory.0.join("yabairc");
        let replacement_file = directory.0.join("yabairc.tmp");
        std::fs::write(&watched_file, "before").unwrap();

        let wait_ended = does_the_wait_end_after(&watched_file, || {
            std::fs::write(&replacement_file, "after").unwrap();
            std::fs::rename(&replacement_file, &watched_file).unwrap();
        });

        assert!(wait_ended);
    }

    #[test]
    fn a_save_by_rename_in_the_real_directory_ends_a_wait_started_through_a_symlinked_one() {
        let directory = create_empty_directory_for_the_test("symlinked-directory");
        let real_directory = directory.0.join("dotfiles");
        let symlinked_directory = directory.0.join("config");
        std::fs::create_dir(&real_directory).unwrap();
        std::os::unix::fs::symlink(&real_directory, &symlinked_directory).unwrap();
        std::fs::write(real_directory.join("yabairc"), "before").unwrap();

        let wait_ended = does_the_wait_end_after(&symlinked_directory.join("yabairc"), || {
            std::fs::write(real_directory.join("yabairc.tmp"), "after").unwrap();
            std::fs::rename(
                real_directory.join("yabairc.tmp"),
                real_directory.join("yabairc"),
            )
            .unwrap();
        });

        assert!(wait_ended);
    }

    #[test]
    fn a_missing_file_cannot_be_watched() {
        let directory = create_empty_directory_for_the_test("missing");

        let file_change_watch =
            FileChangeWatch::start_watching_the_file(&directory.0.join("yabairc"));

        assert!(file_change_watch.is_err());
    }
}
