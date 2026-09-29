pub mod config;
pub mod display;
pub mod labels;
#[cfg(test)]
mod manual_page;
pub mod query;
pub mod rule;
pub mod scratchpad;
pub mod selectors;
pub mod signal;
pub mod space;
pub mod values;
pub mod window;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use crate::command::config::ConfigCommand;
use crate::command::display::DisplayCommand;
use crate::command::query::QueryCommand;
use crate::command::rule::RuleCommand;
use crate::command::scratchpad::ScratchpadCommand;
use crate::command::signal::SignalCommand;
use crate::command::space::SpaceCommand;
use crate::command::window::WindowCommand;

#[derive(Parser)]
#[command(
    name = "yabai",
    version,
    about = "Tiling window manager for macOS",
    long_about = "Tiling window manager for macOS.\n\nWithout a command, runs the window manager \
                  itself; the launchd service runs it that way. Every other command is sent to \
                  the running window manager, except `service` and `scripting-addition`.",
    after_long_help = include_str!("help_on_selectors_output_and_exit_status.txt"),
    args_conflicts_with_subcommands = true,
    propagate_version = true
)]
pub(crate) struct CommandLine {
    /// Config file the window manager runs at start-up [default: the first yabairc found]
    ///
    /// Without this option, the first of $XDG_CONFIG_HOME/yabai/yabairc, ~/.config/yabai/yabairc
    /// and ~/.yabairc that exists. An executable file runs as `sh -c FILE`, any other as
    /// `sh FILE`.
    #[arg(short, long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,
    /// Print debug information to stdout
    #[arg(short, long)]
    pub(crate) verbose: bool,
    #[arg(long, hide = true, exclusive = true)]
    pub(crate) report_screen_recording_permission: bool,
    #[command(subcommand)]
    pub(crate) command: Option<TopLevelCommand>,
}

#[derive(Subcommand)]
pub(crate) enum TopLevelCommand {
    /// Manage the launchd service (~/Library/LaunchAgents/com.asmvik.yabai.plist)
    #[command(subcommand)]
    Service(LaunchdServiceAction),
    /// Load or uninstall the Dock scripting addition (System Integrity Protection must be
    /// partially disabled)
    #[command(subcommand, visible_alias = "sa")]
    ScriptingAddition(ScriptingAdditionAction),
    #[command(flatten)]
    SentToTheRunningWindowManager(DaemonCommand),
}

#[derive(Subcommand, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LaunchdServiceAction {
    /// Write the launchd service file
    Install,
    /// Remove the launchd service file
    Uninstall,
    /// Install the service file if it is missing, then enable, load and start the service
    Start,
    /// Restart the running service
    Restart,
    /// Stop the running service
    Stop,
}

#[derive(Subcommand, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ScriptingAdditionAction {
    /// Install the scripting addition if it is missing or outdated, then load it into the Dock
    /// (run as root)
    Load,
    /// Remove the scripting addition (run as root)
    Uninstall,
}

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum DaemonCommand {
    /// Change or print the settings of the window manager
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Focus, label or show a space on a display
    Display(DisplayCommand),
    /// Focus, create, move, re-tile or label a space
    Space(SpaceCommand),
    /// Focus, move, resize, tile or toggle a window
    ///
    /// Relative selectors in the arguments of an action (west, next, stack.next…) start from the
    /// acting window.
    Window(WindowCommand),
    /// Show and hide windows by name
    #[command(subcommand)]
    Scratchpad(ScratchpadCommand),
    /// Print displays, spaces and windows as JSON
    #[command(subcommand)]
    Query(QueryCommand),
    /// Add, apply, remove or list the rules new windows get
    #[command(subcommand)]
    Rule(RuleCommand),
    /// Add, remove or list the shell commands run after events
    #[command(subcommand)]
    Signal(SignalCommand),
}

#[cfg(test)]
pub(crate) fn parse_daemon_command(arguments: &[&str]) -> Result<DaemonCommand, clap::Error> {
    let command_line =
        CommandLine::try_parse_from(std::iter::once("yabai").chain(arguments.iter().copied()))?;
    match command_line.command {
        Some(TopLevelCommand::SentToTheRunningWindowManager(command)) => Ok(command),
        _ => panic!("{arguments:?} should parse as a command sent to the window manager"),
    }
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{
        CommandLine, DaemonCommand, LaunchdServiceAction, ScriptingAdditionAction, TopLevelCommand,
    };
    use crate::cli::screen_recording_permission_report::REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION;

    fn parse(arguments: &[&str]) -> Result<CommandLine, clap::Error> {
        CommandLine::try_parse_from(std::iter::once("yabai").chain(arguments.iter().copied()))
    }

    #[test]
    fn the_command_tree_is_consistent() {
        CommandLine::command().debug_assert();
    }

    #[test]
    fn the_window_manager_takes_an_optional_config_file_and_verbose_output_but_no_command() {
        let command_line = parse(&[]).unwrap();
        assert!(command_line.command.is_none());
        assert!(command_line.config.is_none());
        assert!(!command_line.verbose);

        let command_line = parse(&["-c", "/tmp/yabairc", "-v"]).unwrap();
        assert_eq!(command_line.config.unwrap().to_str(), Some("/tmp/yabairc"));
        assert!(command_line.verbose);

        assert!(parse(&["-c", "/tmp/yabairc", "service", "start"]).is_err());
    }

    #[test]
    fn the_screen_recording_permission_report_stands_alone_and_out_of_the_help() {
        assert!(
            parse(&[REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION])
                .unwrap()
                .report_screen_recording_permission
        );
        assert!(
            parse(&[
                REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION,
                "-v"
            ])
            .is_err()
        );
        assert!(
            !CommandLine::command()
                .render_long_help()
                .to_string()
                .contains("report-screen-recording-permission")
        );
    }

    #[test]
    fn service_and_scripting_addition_actions_are_subcommands() {
        assert!(matches!(
            parse(&["service", "restart"]).unwrap().command,
            Some(TopLevelCommand::Service(LaunchdServiceAction::Restart))
        ));
        assert!(matches!(
            parse(&["sa", "load"]).unwrap().command,
            Some(TopLevelCommand::ScriptingAddition(
                ScriptingAdditionAction::Load
            ))
        ));
        assert!(matches!(
            parse(&["scripting-addition", "uninstall"]).unwrap().command,
            Some(TopLevelCommand::ScriptingAddition(
                ScriptingAdditionAction::Uninstall
            ))
        ));
    }

    #[test]
    fn every_kind_of_command_crosses_the_socket_unchanged() {
        for arguments in [
            &[
                "config",
                "set",
                "--mouse-follows-focus",
                "on",
                "--insert-feedback-color",
                "0xff0a7aff",
            ][..],
            &["config", "set", "--space", "2", "--layout", "stack"],
            &["query", "displays", "--window", "--fields", "id"],
            &["query", "windows", "--space", "recent"],
            &["query", "windows"],
            &[
                "rule",
                "add",
                "--app",
                "Raycast",
                "--display",
                "2",
                "--opacity",
                "0.9",
            ],
            &[
                "signal",
                "add",
                "space-changed",
                "--action",
                "true",
                "--label",
                "bar",
            ],
            &["display", "-d", "external", "label"],
            &["display", "show-space", "code"],
            &["space", "-s", "2", "padding", "by", "-10", "0", "5", "0"],
            &["space", "rotate", "90"],
            &["window", "-w", "stack.3", "resize", "top-left", "-10", "0"],
            &["window", "focus"],
            &["scratchpad", "assign", "terminal", "-w", "mouse"],
        ] {
            let command = super::parse_daemon_command(arguments).unwrap();
            let command_as_json = serde_json::to_string(&command).unwrap();

            let command_read_back: DaemonCommand = serde_json::from_str(&command_as_json).unwrap();

            assert_eq!(
                serde_json::to_string(&command_read_back).unwrap(),
                command_as_json,
                "{arguments:?}"
            );
        }
    }
}
