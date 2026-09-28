use std::path::Path;

use crate::state::process_wide::CONFIG_FILE_PATH;

pub fn find_config_file_in_xdg_or_home_directories(filename: &str) -> Option<String> {
    if let Some(xdg_home) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg_home.is_empty()
    {
        let buffer = format!("{}/yabai/{}", xdg_home.to_string_lossy(), filename);
        if Path::new(&buffer).is_file() {
            return Some(buffer);
        }
    }

    let home = std::env::var_os("HOME")?;

    let buffer = format!("{}/.config/yabai/{}", home.to_string_lossy(), filename);
    if Path::new(&buffer).is_file() {
        return Some(buffer);
    }

    let buffer = format!("{}/.{}", home.to_string_lossy(), filename);
    Path::new(&buffer).is_file().then_some(buffer)
}

pub fn locate_the_config_file_warning_when_there_is_none() -> Option<String> {
    let config_file_from_the_command_line = CONFIG_FILE_PATH.get().map_or("", String::as_str);
    let config_file = if config_file_from_the_command_line.is_empty() {
        match find_config_file_in_xdg_or_home_directories("yabairc") {
            Some(config_file) => config_file,
            None => {
                crate::warn!("yabai: could not locate config file..\n");
                crate::notify!("configuration", "could not locate config file..");
                return None;
            }
        }
    } else {
        String::from(config_file_from_the_command_line)
    };

    if !Path::new(&config_file).is_file() {
        crate::warn!(
            "yabai: configuration file '{}' does not exist..\n",
            config_file
        );
        crate::notify!("configuration", "file '{}' does not exist..", config_file);
        return None;
    }

    Some(config_file)
}
