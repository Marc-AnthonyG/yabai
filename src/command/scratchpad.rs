use clap::Subcommand;
use clap::builder::NonEmptyStringValueParser;
use serde::{Deserialize, Serialize};

use crate::command::selectors::WindowSelector;

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum ScratchpadCommand {
    /// Make a window the scratchpad NAME; it floats from then on
    Assign {
        #[arg(value_parser = NonEmptyStringValueParser::new())]
        name: String,
        /// The window to assign [default: the focused window]
        #[arg(short, long, value_name = "WINDOW_SEL")]
        window: Option<WindowSelector>,
    },
    /// Stop a window from being a scratchpad; it tiles again
    Unassign {
        /// The window to unassign [default: the focused window]
        #[arg(short, long, value_name = "WINDOW_SEL")]
        window: Option<WindowSelector>,
    },
    /// Show the scratchpad NAME on the focused space, or hide it
    Toggle {
        #[arg(value_parser = NonEmptyStringValueParser::new())]
        name: String,
    },
    /// Bring every hidden scratchpad window back into view (needs the scripting addition)
    Recover,
}

#[cfg(test)]
mod tests {
    use super::ScratchpadCommand;
    use crate::command::selectors::WindowSelector;
    use crate::command::{DaemonCommand, parse_daemon_command};

    fn parse_scratchpad_command(arguments: &[&str]) -> Result<ScratchpadCommand, clap::Error> {
        match parse_daemon_command(&[&["scratchpad"], arguments].concat())? {
            DaemonCommand::Scratchpad(scratchpad_command) => Ok(scratchpad_command),
            _ => panic!("{arguments:?} should parse as a scratchpad command"),
        }
    }

    #[test]
    fn a_scratchpad_is_any_non_empty_name_and_a_window_defaults_to_the_focused_one() {
        assert!(matches!(
            parse_scratchpad_command(&["assign", "float", "-w", "123"]).unwrap(),
            ScratchpadCommand::Assign {
                window: Some(WindowSelector::Id(123)),
                ..
            }
        ));
        assert!(matches!(
            parse_scratchpad_command(&["unassign"]).unwrap(),
            ScratchpadCommand::Unassign { window: None }
        ));
        for refused in [
            &["assign", ""][..],
            &["toggle"],
            &["unassign", "terminal"],
            &["recover", "terminal"],
        ] {
            assert!(parse_scratchpad_command(refused).is_err(), "{refused:?}");
        }
    }
}
