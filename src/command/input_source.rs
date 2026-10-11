use clap::Subcommand;
use clap::builder::NonEmptyStringValueParser;
use serde::{Deserialize, Serialize};

#[derive(Subcommand, Clone, Serialize, Deserialize, Debug)]
pub(crate) enum InputSourceCommand {
    /// Select the next enabled keyboard layout or input mode, wrapping around; with two of them,
    /// it toggles between them
    Cycle,
    /// Select the keyboard layout or input mode ID, such as com.apple.keylayout.Canadian-CSA
    Select {
        #[arg(value_name = "ID", value_parser = NonEmptyStringValueParser::new())]
        input_source_id: String,
    },
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::InputSourceCommand;
    use crate::command::{CommandLine, DaemonCommand, TopLevelCommand};

    fn parse_input_source_command(arguments: &[&str]) -> Result<InputSourceCommand, clap::Error> {
        let command_line = CommandLine::try_parse_from(
            ["yabai", "input-source"]
                .into_iter()
                .chain(arguments.iter().copied()),
        )?;
        match command_line.command {
            Some(TopLevelCommand::SentToTheRunningWindowManager(DaemonCommand::InputSource(
                input_source_command,
            ))) => Ok(input_source_command),
            _ => panic!("{arguments:?} should parse as an input-source command"),
        }
    }

    #[test]
    fn input_source_cycle_takes_no_argument() {
        assert!(matches!(
            parse_input_source_command(&["cycle"]).unwrap(),
            InputSourceCommand::Cycle
        ));
        assert!(parse_input_source_command(&["cycle", "com.apple.keylayout.US"]).is_err());
    }

    #[test]
    fn input_source_select_takes_one_non_empty_input_source_id() {
        assert!(matches!(
            parse_input_source_command(&["select", "com.apple.keylayout.Canadian-CSA"]).unwrap(),
            InputSourceCommand::Select { input_source_id }
                if input_source_id == "com.apple.keylayout.Canadian-CSA"
        ));
        assert!(parse_input_source_command(&["select"]).is_err());
        assert!(parse_input_source_command(&["select", ""]).is_err());
        assert!(parse_input_source_command(&["select", "a", "b"]).is_err());
    }

    #[test]
    fn input_source_takes_no_other_action() {
        assert!(parse_input_source_command(&["next"]).is_err());
    }
}
