use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::command::labels::parse_display_label_that_no_display_selector_reads_otherwise;
use crate::command::selectors::{DisplaySelector, SpaceSelector};

#[derive(Args, Serialize, Deserialize, Debug)]
pub(crate) struct DisplayCommand {
    /// Act on this display instead of the one showing the active menu bar
    #[arg(short, long, global = true, value_name = "DISPLAY_SEL")]
    pub(crate) display: Option<DisplaySelector>,
    #[command(subcommand)]
    pub(crate) action: DisplayAction,
}

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum DisplayAction {
    /// Focus another display; relative selectors start from the acting display
    Focus {
        #[arg(value_name = "DISPLAY_SEL")]
        target: DisplaySelector,
    },
    /// Show a space of the acting display there without focusing it (needs the scripting
    /// addition)
    ShowSpace {
        #[arg(value_name = "SPACE_SEL")]
        space: SpaceSelector,
    },
    /// Label the acting display; without LABEL, remove its label
    Label {
        /// Any text a display selector would not read as something else
        #[arg(value_parser = parse_display_label_that_no_display_selector_reads_otherwise)]
        label: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::{DisplayAction, DisplayCommand};
    use crate::command::selectors::{CardinalDirection, DisplaySelector, SpaceSelector};
    use crate::command::{DaemonCommand, parse_daemon_command};

    fn parse_display_command(arguments: &[&str]) -> Result<DisplayCommand, clap::Error> {
        match parse_daemon_command(&[&["display"], arguments].concat())? {
            DaemonCommand::Display(display_command) => Ok(display_command),
            _ => panic!("{arguments:?} should parse as a display command"),
        }
    }

    #[test]
    fn a_display_command_takes_an_acting_display_and_one_action() {
        let display_command = parse_display_command(&["focus", "west", "-d", "2"]).unwrap();
        assert!(matches!(
            display_command.display,
            Some(DisplaySelector::ArrangementIndex(_))
        ));
        assert!(matches!(
            display_command.action,
            DisplayAction::Focus {
                target: DisplaySelector::InDirection(CardinalDirection::West)
            }
        ));
        assert!(matches!(
            parse_display_command(&["show-space", "last"])
                .unwrap()
                .action,
            DisplayAction::ShowSpace {
                space: SpaceSelector::Last
            }
        ));
        assert!(matches!(
            parse_display_command(&["label", "external"])
                .unwrap()
                .action,
            DisplayAction::Label { label: Some(_) }
        ));
        for refused in [
            &["focus"][..],
            &["label", "west"],
            &["--focus", "west"],
            &[],
        ] {
            assert!(parse_display_command(refused).is_err(), "{refused:?}");
        }
    }
}
