mod application;
mod cli;
mod command;
mod config_file;
mod display;
mod event;
mod ffi;
mod layout;
mod message;
mod mouse;
mod notifications;
mod process;
mod protocol;
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

use std::path::PathBuf;

use clap::Parser;

use crate::cli::client::send_command_to_the_running_window_manager_and_print_its_reply;
use crate::cli::local_action::{run_launchd_service_action, run_scripting_addition_action};
use crate::cli::screen_recording_permission_report::exit_status_reporting_whether_screen_recording_is_granted;
use crate::command::{CommandLine, TopLevelCommand};
use crate::startup::order::start_the_daemon_and_enter_the_main_run_loop;
use crate::startup::requirements::ask_for_missing_permissions_then_exit_or_wait_until_the_system_meets_the_daemon_requirements;
use crate::startup::runtime::{
    install_panic_hook_that_aborts_the_process, restore_default_sigpipe_disposition,
};
use crate::startup::screen_recording_grant_watcher::start_watching_for_a_requested_screen_recording_grant_to_relaunch_the_daemon;
use crate::startup::settings_and_lock_file::configure_settings_and_acquire_lock_or_exit;
use crate::state::process_wide::CONFIG_FILE_PATH;
use crate::support::log::set_verbose_debug_output_enabled;

fn main() {
    install_panic_hook_that_aborts_the_process();
    restore_default_sigpipe_disposition();
    let command_line = CommandLine::parse();

    if command_line.report_screen_recording_permission {
        std::process::exit(exit_status_reporting_whether_screen_recording_is_granted());
    }

    match command_line.command {
        Some(TopLevelCommand::Service(action)) => {
            std::process::exit(run_launchd_service_action(action))
        }
        Some(TopLevelCommand::ScriptingAddition(action)) => {
            std::process::exit(run_scripting_addition_action(action))
        }
        Some(TopLevelCommand::SentToTheRunningWindowManager(command)) => std::process::exit(
            send_command_to_the_running_window_manager_and_print_its_reply(command),
        ),
        None => run_the_window_manager(command_line.config, command_line.verbose),
    }
}

fn run_the_window_manager(config_file: Option<PathBuf>, verbose: bool) {
    set_verbose_debug_output_enabled(verbose);
    let _ = CONFIG_FILE_PATH.set(
        config_file
            .map(|config_file| config_file.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    let screen_recording_permission_at_start_up =
        ask_for_missing_permissions_then_exit_or_wait_until_the_system_meets_the_daemon_requirements();
    configure_settings_and_acquire_lock_or_exit();
    start_watching_for_a_requested_screen_recording_grant_to_relaunch_the_daemon(
        screen_recording_permission_at_start_up,
    );
    start_the_daemon_and_enter_the_main_run_loop();
}
