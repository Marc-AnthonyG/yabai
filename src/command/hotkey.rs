use clap::builder::NonEmptyStringValueParser;
use clap::error::ErrorKind;
use clap::{ArgGroup, ArgMatches, Args, Command, FromArgMatches, Subcommand};
use serde::{Deserialize, Serialize};

use crate::command::selectors::IndexOrLabelSelector;
use crate::command::{DaemonCommand, parse_daemon_command, refusal_of_clap_error_without_usage};
use crate::hotkey::action::HotkeyAction;
use crate::hotkey::chord::Chord;

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum HotkeyCommand {
    /// Run a yabai command, or a shell command, whenever CHORD is pressed
    ///
    /// CHORD is [MODIFIER [+ MODIFIER]... -] KEY, or a lone modifier. MODIFIER is alt, shift,
    /// cmd, ctrl or fn, on either side of the keyboard; every other one of alt, shift, cmd and
    /// ctrl must be up, caps lock does not count, and fn only counts when the chord names it.
    /// KEY is a-z, 0-9, return, tab, space, escape, backspace, delete (forward delete), f1-f20,
    /// left, right, up, down, home, end, pageup, pagedown, minus, equal, leftbracket,
    /// rightbracket, semicolon, quote, comma, period, slash, backslash or grave, named after the
    /// key at that place on an ANSI keyboard whatever the layout.
    ///
    /// A lone modifier, lcmd, rcmd, lalt, ralt, lctrl, rctrl, lshift or rshift, runs the action
    /// when that key is pressed and released within a second with nothing else in between, and
    /// never while secure input is on, as it is in a password field.
    ///
    /// A hotkey whose chord is already bound, or whose label another hotkey has, replaces that
    /// hotkey. The pressed key is swallowed and runs the action again while it repeats; its
    /// release, and every other key, reach the focused application.
    ///
    /// When the window manager could not create its keyboard tap, usually because yabai is not
    /// allowed under Accessibility, the hotkey is still added but the command fails, since no
    /// hotkey can fire.
    #[command(
        arg_required_else_help = true,
        override_usage = "yabai hotkey add <CHORD> [--label <LABEL>] [--description <TEXT>] \
                          (--shell <COMMAND> | -- <YABAI COMMAND ARGUMENTS>...)"
    )]
    Add(HotkeyDefinition),
    /// Remove a hotkey, or every hotkey
    #[command(override_usage = "yabai hotkey remove (<HOTKEY> | --all)")]
    Remove {
        /// The hotkey to remove, by index from 0 or by label
        #[arg(value_name = "HOTKEY", required_unless_present = "all")]
        hotkey: Option<IndexOrLabelSelector>,
        /// Remove every hotkey
        #[arg(long, conflicts_with = "hotkey")]
        all: bool,
    },
    /// Print every hotkey as a JSON array, in the order they were added, a hotkey that replaced
    /// others taking the place of the first of them
    List,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct HotkeyDefinition {
    pub(crate) chord: Chord,
    pub(crate) label: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) action: HotkeyAction,
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("hotkey_action")
        .args(["shell", "yabai_command_arguments"])
        .required(true)
))]
struct HotkeyDefinitionAsWritten {
    /// The keys that run the action: [MODIFIER [+ MODIFIER]... -] KEY, or a lone modifier
    #[arg(value_name = "CHORD")]
    chord: Chord,
    /// A later hotkey with the same label replaces this one
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    label: Option<String>,
    /// What the hotkey does, for `hotkey list` and the cheatsheets built from it
    #[arg(long, value_name = "TEXT")]
    description: Option<String>,
    /// The shell command to run through /usr/bin/env sh -c, without waiting for it
    #[arg(long, value_name = "COMMAND", value_parser = NonEmptyStringValueParser::new())]
    shell: Option<String>,
    /// The yabai command to run, written as on the command line after `yabai`
    #[arg(last = true, value_name = "YABAI COMMAND ARGUMENTS")]
    yabai_command_arguments: Vec<String>,
}

impl TryFrom<HotkeyDefinitionAsWritten> for HotkeyDefinition {
    type Error = String;

    fn try_from(written: HotkeyDefinitionAsWritten) -> Result<HotkeyDefinition, String> {
        let action = match (written.shell, written.yabai_command_arguments) {
            (Some(shell_command), yabai_command_arguments)
                if yabai_command_arguments.is_empty() =>
            {
                HotkeyAction::Shell(shell_command)
            }
            (None, yabai_command_arguments) if !yabai_command_arguments.is_empty() => {
                HotkeyAction::Yabai(yabai_command_a_hotkey_may_run(yabai_command_arguments)?)
            }
            _ => {
                return Err(String::from(
                    "a hotkey runs either --shell COMMAND or -- YABAI COMMAND ARGUMENTS",
                ));
            }
        };
        Ok(HotkeyDefinition {
            chord: written.chord,
            label: written.label,
            description: written.description,
            action,
        })
    }
}

fn yabai_command_a_hotkey_may_run(
    yabai_command_arguments: Vec<String>,
) -> Result<Vec<String>, String> {
    match parse_daemon_command(&yabai_command_arguments) {
        Ok(DaemonCommand::Hotkey(_)) => Err(String::from(
            "a hotkey cannot run a hotkey command; run it from a shell action instead",
        )),
        Ok(_) => Ok(yabai_command_arguments),
        Err(error) => Err(format!(
            "`{}` is not a yabai command: {}",
            yabai_command_arguments.join(" "),
            refusal_of_clap_error_without_usage(&error)
        )),
    }
}

impl FromArgMatches for HotkeyDefinition {
    fn from_arg_matches(matches: &ArgMatches) -> Result<HotkeyDefinition, clap::Error> {
        HotkeyDefinition::try_from(HotkeyDefinitionAsWritten::from_arg_matches(matches)?)
            .map_err(|refusal| clap::Error::raw(ErrorKind::ValueValidation, refusal))
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = HotkeyDefinition::from_arg_matches(matches)?;
        Ok(())
    }
}

impl Args for HotkeyDefinition {
    fn augment_args(command: Command) -> Command {
        HotkeyDefinitionAsWritten::augment_args(command)
    }

    fn augment_args_for_update(command: Command) -> Command {
        HotkeyDefinitionAsWritten::augment_args_for_update(command)
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{HotkeyCommand, HotkeyDefinition};
    use crate::command::selectors::IndexOrLabelSelector;
    use crate::command::{CommandLine, DaemonCommand, TopLevelCommand};
    use crate::hotkey::action::HotkeyAction;
    use crate::hotkey::chord::Chord;
    use crate::hotkey::lone_modifier::LoneModifier;

    fn parse_hotkey_command(arguments: &[&str]) -> Result<HotkeyCommand, clap::Error> {
        let command_line = CommandLine::try_parse_from(
            ["yabai", "hotkey"]
                .into_iter()
                .chain(arguments.iter().copied()),
        )?;
        match command_line.command {
            Some(TopLevelCommand::SentToTheRunningWindowManager(DaemonCommand::Hotkey(
                hotkey_command,
            ))) => Ok(hotkey_command),
            _ => panic!("{arguments:?} should parse as a hotkey command"),
        }
    }

    fn parse_hotkey_add(arguments: &[&str]) -> Result<HotkeyDefinition, clap::Error> {
        match parse_hotkey_command(&[&["add"], arguments].concat())? {
            HotkeyCommand::Add(definition) => Ok(definition),
            _ => panic!("{arguments:?} should parse as hotkey add"),
        }
    }

    fn refusal_of_hotkey_add(arguments: &[&str]) -> String {
        parse_hotkey_add(arguments)
            .expect_err("hotkey add should be refused")
            .to_string()
    }

    #[test]
    fn a_hotkey_takes_its_chord_its_flags_then_the_yabai_command_it_runs_after_a_double_dash() {
        let definition = parse_hotkey_add(&[
            "cmd+alt-h",
            "--description",
            "Focus the window to the west",
            "--label",
            "focus-west",
            "--",
            "window",
            "focus",
            "west",
        ])
        .unwrap();

        assert_eq!(definition.chord, "alt + cmd - h".parse::<Chord>().unwrap());
        assert_eq!(definition.label.as_deref(), Some("focus-west"));
        assert_eq!(
            definition.description.as_deref(),
            Some("Focus the window to the west")
        );
        assert_eq!(
            definition.action,
            HotkeyAction::Yabai(vec![
                String::from("window"),
                String::from("focus"),
                String::from("west"),
            ])
        );
    }

    #[test]
    fn a_hotkey_takes_a_shell_command_instead_and_leaves_its_label_and_description_unset() {
        let definition =
            parse_hotkey_add(&["rcmd", "--shell", "yabai input-source cycle"]).unwrap();

        assert_eq!(
            definition.chord,
            Chord::LoneModifierTap(LoneModifier::RightCommand)
        );
        assert_eq!(
            definition.action,
            HotkeyAction::Shell(String::from("yabai input-source cycle"))
        );
        assert_eq!(definition.label, None);
        assert_eq!(definition.description, None);
    }

    #[test]
    fn a_hotkey_needs_exactly_one_of_a_shell_command_and_a_yabai_command() {
        assert!(parse_hotkey_add(&["alt - h"]).is_err());
        assert!(parse_hotkey_add(&["alt - h", "--label", "focus"]).is_err());
        assert!(
            parse_hotkey_add(&["alt - h", "--shell", "true", "--", "window", "focus"]).is_err()
        );
    }

    #[test]
    fn a_hotkey_refuses_a_chord_that_does_not_parse() {
        assert!(
            refusal_of_hotkey_add(&["alt - hh", "--shell", "true"]).contains("'hh' is not a key")
        );
        assert!(parse_hotkey_add(&["rcmd - h", "--shell", "true"]).is_err());
        assert!(parse_hotkey_add(&["cmd", "--shell", "true"]).is_err());
    }

    #[test]
    fn a_hotkey_refuses_an_empty_label_or_an_empty_shell_command() {
        assert!(parse_hotkey_add(&["alt - h", "--label", "", "--shell", "true"]).is_err());
        assert!(parse_hotkey_add(&["alt - h", "--shell", ""]).is_err());
    }

    #[test]
    fn a_hotkey_refuses_a_yabai_command_that_does_not_parse_and_says_why() {
        let refusal = refusal_of_hotkey_add(&["alt - h", "--", "window", "--frobnicate"]);

        assert_eq!(
            refusal.lines().next(),
            Some(
                "error: `window --frobnicate` is not a yabai command: unexpected argument \
                 '--frobnicate' found"
            )
        );
    }

    #[test]
    fn a_hotkey_refuses_a_command_line_that_is_not_sent_to_the_window_manager() {
        assert!(
            refusal_of_hotkey_add(&["alt - h", "--", "service", "start"])
                .contains("`service start` is not a yabai command")
        );
    }

    #[test]
    fn a_hotkey_cannot_run_a_hotkey_command() {
        assert!(
            refusal_of_hotkey_add(&["alt - h", "--", "hotkey", "list"])
                .contains("a hotkey cannot run a hotkey command")
        );
    }

    #[test]
    fn hotkey_remove_takes_an_index_or_a_label_or_all_but_not_both() {
        assert!(matches!(
            parse_hotkey_command(&["remove", "2"]).unwrap(),
            HotkeyCommand::Remove {
                hotkey: Some(IndexOrLabelSelector::Index(2)),
                all: false
            }
        ));
        assert!(matches!(
            parse_hotkey_command(&["remove", "focus-west"]).unwrap(),
            HotkeyCommand::Remove {
                hotkey: Some(IndexOrLabelSelector::Label(label)),
                all: false
            } if label == "focus-west"
        ));
        assert!(matches!(
            parse_hotkey_command(&["remove", "--all"]).unwrap(),
            HotkeyCommand::Remove {
                hotkey: None,
                all: true
            }
        ));
        assert!(parse_hotkey_command(&["remove"]).is_err());
        assert!(parse_hotkey_command(&["remove", "2", "--all"]).is_err());
    }

    #[test]
    fn hotkey_list_takes_no_argument() {
        assert!(matches!(
            parse_hotkey_command(&["list"]).unwrap(),
            HotkeyCommand::List
        ));
        assert!(parse_hotkey_command(&["list", "--all"]).is_err());
    }
}
