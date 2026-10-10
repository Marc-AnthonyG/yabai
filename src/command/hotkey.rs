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
    /// Print every hotkey as a JSON array, in the order they were added
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
