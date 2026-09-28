use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::command::selectors::IndexOrLabelSelector;
use crate::command::values::{OnOrOff, parse_regular_expression_that_compiles};
use crate::signal::definition::SignalType;

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum SignalCommand {
    /// Run a shell command (through /usr/bin/env sh -c) after every EVENT
    Add(SignalDefinition),
    /// Remove a signal
    Remove {
        /// The signal to remove, by index from 0 or by label
        #[arg(value_name = "SIGNAL")]
        signal: IndexOrLabelSelector,
    },
    /// Print every signal as a JSON array
    List,
}

#[derive(Args, Serialize, Deserialize, Debug)]
pub(crate) struct SignalDefinition {
    /// The event that runs the action
    pub(crate) event: SignalType,
    /// The shell command to run; the YABAI_* variables of the event are in its environment
    #[arg(long, value_name = "SHELL_COMMAND")]
    pub(crate) action: String,
    /// A later signal with the same label replaces this one
    #[arg(long)]
    pub(crate) label: Option<String>,
    /// Only for windows or applications whose application name matches REGEX
    #[arg(long, value_name = "REGEX", conflicts_with = "app_not", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) app: Option<String>,
    /// Only for windows or applications whose application name does not match REGEX
    #[arg(long, value_name = "REGEX", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) app_not: Option<String>,
    /// Only for windows whose title matches REGEX
    #[arg(long, value_name = "REGEX", conflicts_with = "title_not", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) title: Option<String>,
    /// Only for windows whose title does not match REGEX
    #[arg(long, value_name = "REGEX", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) title_not: Option<String>,
    /// Only when the window or application is the focused one (on) or is not (off)
    #[arg(long)]
    pub(crate) active: Option<OnOrOff>,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{SignalCommand, SignalDefinition};
    use crate::command::selectors::IndexOrLabelSelector;
    use crate::command::values::{
        OnOrOff, assert_every_value_is_spelled_alike_on_the_command_line_and_in_json,
    };
    use crate::command::{CommandLine, DaemonCommand, TopLevelCommand};
    use crate::signal::definition::SignalType;

    fn parse_signal_command(arguments: &[&str]) -> Result<SignalCommand, clap::Error> {
        let command_line = CommandLine::try_parse_from(
            ["yabai", "signal"]
                .into_iter()
                .chain(arguments.iter().copied()),
        )?;
        match command_line.command {
            Some(TopLevelCommand::SentToTheRunningWindowManager(DaemonCommand::Signal(
                signal_command,
            ))) => Ok(signal_command),
            _ => panic!("{arguments:?} should parse as a signal command"),
        }
    }

    fn parse_signal_add(arguments: &[&str]) -> Result<SignalDefinition, clap::Error> {
        match parse_signal_command(&[&["add"], arguments].concat())? {
            SignalCommand::Add(definition) => Ok(definition),
            _ => panic!("{arguments:?} should parse as signal add"),
        }
    }

    #[test]
    fn a_signal_takes_its_event_then_its_action_and_filters_as_flags() {
        let definition = parse_signal_add(&[
            "dock-did-restart",
            "--action",
            "sudo yabai scripting-addition load",
            "--label",
            "reload",
            "--app-not",
            "^Finder$",
            "--active",
            "on",
        ])
        .unwrap();

        assert_eq!(definition.event, SignalType::DockDidRestart);
        assert_eq!(definition.action, "sudo yabai scripting-addition load");
        assert_eq!(definition.label.as_deref(), Some("reload"));
        assert_eq!(definition.app_not.as_deref(), Some("^Finder$"));
        assert_eq!(definition.active, Some(OnOrOff::On));
    }

    #[test]
    fn a_signal_needs_a_known_event_and_an_action() {
        assert!(parse_signal_add(&["window-focused"]).is_err());
        assert!(parse_signal_add(&["window_focused", "--action", "true"]).is_err());
        assert!(parse_signal_add(&["unknown", "--action", "true"]).is_err());
        assert!(parse_signal_add(&["signal-type-unknown", "--action", "true"]).is_err());
        assert!(
            parse_signal_add(&["window-focused", "--action", "true", "--active", "yes"]).is_err()
        );
    }

    #[test]
    fn signal_remove_takes_an_index_or_a_label() {
        assert!(matches!(
            parse_signal_command(&["remove", "screen_padding_display_changed"]).unwrap(),
            SignalCommand::Remove {
                signal: IndexOrLabelSelector::Label(_)
            }
        ));
    }

    #[test]
    fn every_event_is_spelled_alike_on_the_command_line_and_in_json() {
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<SignalType>();
        assert_eq!(
            serde_json::to_string(&SignalType::DockDidChangePreferences).unwrap(),
            "\"dock-did-change-preferences\""
        );
    }
}
