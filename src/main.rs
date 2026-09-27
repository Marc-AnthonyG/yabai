mod ffi;
mod service;
mod support;
mod state;
mod layout;
mod scripting_addition;
mod event;
mod signal;
mod message;
mod display;
mod space;
mod window;
mod process;
mod application;
mod mouse;
mod notifications;
mod query;
mod serialise;
mod cli;
mod startup;

use crate::cli::arguments::store_config_file_path_from_command_line;
use crate::startup::order::start_the_daemon_and_enter_the_main_run_loop;
use crate::startup::requirements::exit_unless_the_system_meets_the_daemon_requirements;
use crate::startup::runtime::{
    install_panic_hook_that_aborts_the_process, restore_default_sigpipe_disposition,
};
use crate::startup::settings_and_lock_file::configure_settings_and_acquire_lock_or_exit;

fn main() {
    install_panic_hook_that_aborts_the_process();
    restore_default_sigpipe_disposition();
    store_config_file_path_from_command_line();
    exit_unless_the_system_meets_the_daemon_requirements();
    configure_settings_and_acquire_lock_or_exit();
    start_the_daemon_and_enter_the_main_run_loop();
}
