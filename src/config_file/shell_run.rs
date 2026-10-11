use std::process::Command;

use crate::ffi::libsystem::return_the_free_pages_of_every_malloc_zone_to_the_system;
use crate::support::filesystem::can_owner_execute_file;
use crate::support::spawned_program_exit_status::wait_until_the_spawned_program_exits;

pub fn run_config_file_in_a_shell_without_waiting_for_it(config_file: &str) {
    let mut shell = Command::new("/usr/bin/env");
    shell.arg("sh");
    if can_owner_execute_file(config_file) {
        shell.arg("-c");
    }
    shell.arg(config_file);

    match shell.spawn() {
        Ok(shell) => return_freed_memory_to_the_system_once_the_shell_exits(shell.id()),
        Err(_) => {
            crate::warn!("yabai: failed to load config file '{}'\n", config_file);
            crate::notify!("configuration", "failed to load file '{}'", config_file);
        }
    }
}

fn return_freed_memory_to_the_system_once_the_shell_exits(process_id_of_the_shell: u32) {
    let _ = std::thread::Builder::new()
        .name(String::from("yabai-config-file-run"))
        .spawn(move || {
            wait_until_the_spawned_program_exits(process_id_of_the_shell);
            return_the_free_pages_of_every_malloc_zone_to_the_system();
        });
}
