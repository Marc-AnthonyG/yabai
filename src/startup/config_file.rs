use core::ffi::c_char;

use crate::support::filesystem::{
    can_owner_execute_file, is_existing_file_that_is_not_a_directory,
};

pub fn find_config_file_in_xdg_or_home_directories(filename: &str) -> Option<String> {
    if let Some(xdg_home) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg_home.is_empty()
    {
        let buffer = format!("{}/yabai/{}", xdg_home.to_string_lossy(), filename);
        if is_existing_file_that_is_not_a_directory(&buffer) {
            return Some(buffer);
        }
    }

    let home = std::env::var_os("HOME")?;

    let buffer = format!("{}/.config/yabai/{}", home.to_string_lossy(), filename);
    if is_existing_file_that_is_not_a_directory(&buffer) {
        return Some(buffer);
    }

    let buffer = format!("{}/.{}", home.to_string_lossy(), filename);
    is_existing_file_that_is_not_a_directory(&buffer).then_some(buffer)
}

pub fn run_config_file_in_a_forked_shell(config_file: String) {
    let config_file = if config_file.is_empty() {
        match find_config_file_in_xdg_or_home_directories("yabairc") {
            Some(config_file) => config_file,
            None => {
                crate::warn!("yabai: could not locate config file..\n");
                crate::notify!("configuration", "could not locate config file..");
                return;
            }
        }
    } else {
        config_file
    };

    if !is_existing_file_that_is_not_a_directory(&config_file) {
        crate::warn!(
            "yabai: configuration file '{}' does not exist..\n",
            config_file
        );
        crate::notify!("configuration", "file '{}' does not exist..", config_file);
        return;
    }

    let config_file_argument = std::ffi::CString::new(config_file.as_str()).unwrap();
    let exec: [*const c_char; 5] = if can_owner_execute_file(&config_file) {
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
