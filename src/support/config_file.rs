use core::ffi::c_char;

use crate::support::filesystem::{file_can_execute, file_exists};

pub fn get_config_file(filename: &str) -> Option<String> {
    if let Some(xdg_home) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg_home.is_empty()
    {
        let buffer = format!("{}/yabai/{}", xdg_home.to_string_lossy(), filename);
        if file_exists(&buffer) {
            return Some(buffer);
        }
    }

    let home = std::env::var_os("HOME")?;

    let buffer = format!("{}/.config/yabai/{}", home.to_string_lossy(), filename);
    if file_exists(&buffer) {
        return Some(buffer);
    }

    let buffer = format!("{}/.{}", home.to_string_lossy(), filename);
    file_exists(&buffer).then_some(buffer)
}

pub fn exec_config_file(config_file: String) {
    let config_file = if config_file.is_empty() {
        match get_config_file("yabairc") {
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

    if !file_exists(&config_file) {
        crate::warn!(
            "yabai: configuration file '{}' does not exist..\n",
            config_file
        );
        crate::notify!("configuration", "file '{}' does not exist..", config_file);
        return;
    }

    let config_file_argument = std::ffi::CString::new(config_file.as_str()).unwrap();
    let exec: [*const c_char; 5] = if file_can_execute(&config_file) {
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
