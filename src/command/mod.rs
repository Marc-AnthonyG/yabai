pub mod config;
pub mod not_yet_typed;
pub mod selectors;
pub mod values;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use crate::command::config::ConfigCommand;

#[derive(Parser)]
#[command(
    name = "yabai",
    version,
    about = "Tiling window manager for macOS",
    long_about = "Tiling window manager for macOS.\n\nWithout a command, runs the window manager \
                  itself; the launchd service runs it that way. Every other command is sent to \
                  the running window manager, except `service` and `scripting-addition`.",
    args_conflicts_with_subcommands = true,
    propagate_version = true
)]
pub(crate) struct CommandLine {
    /// Config file the window manager runs at start-up [default: the first yabairc found]
    #[arg(short, long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,
    /// Print debug information to stdout
    #[arg(short, long)]
    pub(crate) verbose: bool,
    #[arg(long, hide = true, exclusive = true)]
    pub(crate) report_screen_recording_permission: bool,
    #[arg(
        short = 'm',
        long = "message",
        hide = true,
        num_args = 1..,
        allow_hyphen_values = true,
        value_name = "ARGUMENTS",
        conflicts_with_all = ["config", "verbose"]
    )]
    pub(crate) message_to_a_domain_not_yet_typed: Option<Vec<String>>,
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
    #[command(skip)]
    NotYetTyped { arguments: Vec<String> },
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
    fn no_arguments_run_the_window_manager_with_the_default_config_file() {
        let command_line = parse(&[]).unwrap();

        assert!(command_line.command.is_none());
        assert!(command_line.config.is_none());
        assert!(!command_line.verbose);
        assert!(command_line.message_to_a_domain_not_yet_typed.is_none());
    }

    #[test]
    fn the_window_manager_takes_a_config_file_and_verbose_output() {
        let command_line = parse(&["-c", "/tmp/yabairc", "-v"]).unwrap();

        assert_eq!(command_line.config.unwrap().to_str(), Some("/tmp/yabairc"));
        assert!(command_line.verbose);
    }

    #[test]
    fn a_config_file_is_refused_next_to_a_command() {
        assert!(parse(&["-c", "/tmp/yabairc", "service", "start"]).is_err());
    }

    #[test]
    fn the_screen_recording_permission_report_stands_alone() {
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
    }

    #[test]
    fn the_screen_recording_permission_report_stays_out_of_the_help() {
        let help = CommandLine::command().render_long_help().to_string();

        assert!(!help.contains("report-screen-recording-permission"));
        assert!(!help.contains("--message"));
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
    fn the_message_option_takes_every_argument_after_it_including_dashed_ones() {
        let command_line = parse(&["-m", "window", "-w", "3", "--resize", "abs:-10:20"]).unwrap();

        assert_eq!(
            command_line.message_to_a_domain_not_yet_typed.unwrap(),
            ["window", "-w", "3", "--resize", "abs:-10:20"]
        );
    }

    #[test]
    fn the_command_not_yet_typed_crosses_the_socket_unchanged() {
        let command = DaemonCommand::NotYetTyped {
            arguments: vec![String::from("space"), String::from("--balance")],
        };

        let command_read_back: DaemonCommand =
            serde_json::from_str(&serde_json::to_string(&command).unwrap()).unwrap();

        let DaemonCommand::NotYetTyped { arguments } = command_read_back else {
            panic!("the command should come back not yet typed");
        };
        assert_eq!(arguments, ["space", "--balance"]);
    }
}
