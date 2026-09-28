use std::process::Command;

use crate::support::filesystem::can_owner_execute_file;

pub fn run_config_file_in_a_shell_without_waiting_for_it(config_file: &str) {
    let mut shell = Command::new("/usr/bin/env");
    shell.arg("sh");
    if can_owner_execute_file(config_file) {
        shell.arg("-c");
    }
    shell.arg(config_file);

    if shell.spawn().is_err() {
        crate::warn!("yabai: failed to load config file '{}'\n", config_file);
        crate::notify!("configuration", "failed to load file '{}'", config_file);
    }
}
