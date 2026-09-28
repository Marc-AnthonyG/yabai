mod application;
mod cli;
mod config_file;
mod display;
mod event;
mod ffi;
mod layout;
mod message;
mod mouse;
mod notifications;
mod process;
mod query;
mod scripting_addition;
mod serialise;
mod service;
mod signal;
mod space;
mod startup;
mod state;
mod support;
mod window;

use crate::cli::arguments::store_config_file_path_from_command_line;
use crate::startup::order::start_the_daemon_and_enter_the_main_run_loop;
use crate::startup::requirements::ask_for_missing_permissions_then_exit_or_wait_until_the_system_meets_the_daemon_requirements;
use crate::startup::runtime::{
    install_panic_hook_that_aborts_the_process, restore_default_sigpipe_disposition,
};
use crate::startup::screen_recording_grant_watcher::start_watching_for_a_requested_screen_recording_grant_to_relaunch_the_daemon;
use crate::startup::settings_and_lock_file::configure_settings_and_acquire_lock_or_exit;

fn main() {
    install_panic_hook_that_aborts_the_process();
    restore_default_sigpipe_disposition();
    store_config_file_path_from_command_line();
    let screen_recording_permission_at_start_up =
        ask_for_missing_permissions_then_exit_or_wait_until_the_system_meets_the_daemon_requirements();
    configure_settings_and_acquire_lock_or_exit();
    start_watching_for_a_requested_screen_recording_grant_to_relaunch_the_daemon(
        screen_recording_permission_at_start_up,
    );
    start_the_daemon_and_enter_the_main_run_loop();
}
