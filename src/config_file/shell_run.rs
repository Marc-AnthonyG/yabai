use core::ffi::c_char;

use crate::support::filesystem::can_owner_execute_file;

pub fn run_config_file_in_a_forked_shell(config_file: &str) {
    let config_file_argument = std::ffi::CString::new(config_file).unwrap();
    let exec: [*const c_char; 5] = if can_owner_execute_file(config_file) {
        [
            c"/usr/bin/env".as_ptr(),
            c"sh".as_ptr(),
            c"-c".as_ptr(),
            config_file_argument.as_ptr(),
            std::ptr::null(),
        ]
    } else {
        [
            c"/usr/bin/env".as_ptr(),
            c"sh".as_ptr(),
            config_file_argument.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
        ]
    };

    let process_id = unsafe { libc::fork() };
    if process_id == 0 {
        unsafe { libc::_exit(libc::execvp(exec[0], exec.as_ptr())) };
    } else if process_id == -1 {
        crate::warn!("yabai: failed to load config file '{}'\n", config_file);
        crate::notify!("configuration", "failed to load file '{}'", config_file);
    }
}
