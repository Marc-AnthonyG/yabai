use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};

use crate::command::{DaemonCommand, parse_daemon_command, refusal_of_clap_error_without_usage};

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "Vec<String>")]
pub(crate) struct YabaiCommandOfAHotkey(Arc<YabaiCommandParsedOnceWithItsArguments>);

#[derive(Debug)]
struct YabaiCommandParsedOnceWithItsArguments {
    arguments_as_written: Vec<String>,
    command: DaemonCommand,
}

impl YabaiCommandOfAHotkey {
    pub(crate) fn parsed_from_its_arguments(
        arguments_as_written: Vec<String>,
    ) -> Result<YabaiCommandOfAHotkey, String> {
        match parse_daemon_command(&arguments_as_written) {
            Ok(DaemonCommand::Hotkey(_)) => Err(String::from(
                "a hotkey cannot run a hotkey command; run it from a shell action instead",
            )),
            Ok(command) => Ok(YabaiCommandOfAHotkey(Arc::new(
                YabaiCommandParsedOnceWithItsArguments {
                    arguments_as_written,
                    command,
                },
            ))),
            Err(error) => Err(format!(
                "`{}` is not a yabai command: {}",
                arguments_as_written.join(" "),
                refusal_of_clap_error_without_usage(&error)
            )),
        }
    }

    pub(crate) fn arguments_as_written(&self) -> &[String] {
        &self.0.arguments_as_written
    }

    pub(crate) fn command_to_run(&self) -> DaemonCommand {
        self.0.command.clone()
    }
}

impl TryFrom<Vec<String>> for YabaiCommandOfAHotkey {
    type Error = String;

    fn try_from(arguments_as_written: Vec<String>) -> Result<YabaiCommandOfAHotkey, String> {
        YabaiCommandOfAHotkey::parsed_from_its_arguments(arguments_as_written)
    }
}

impl Serialize for YabaiCommandOfAHotkey {
    fn serialize<SerializerOfTheArguments: Serializer>(
        &self,
        serializer: SerializerOfTheArguments,
    ) -> Result<SerializerOfTheArguments::Ok, SerializerOfTheArguments::Error> {
        self.arguments_as_written().serialize(serializer)
    }
}

impl PartialEq for YabaiCommandOfAHotkey {
    fn eq(&self, other: &YabaiCommandOfAHotkey) -> bool {
        self.arguments_as_written() == other.arguments_as_written()
    }
}

impl Eq for YabaiCommandOfAHotkey {}

#[cfg(test)]
mod tests {
    use super::YabaiCommandOfAHotkey;
    use crate::command::DaemonCommand;
    use crate::command::window::WindowAction;

    fn arguments(words: &[&str]) -> Vec<String> {
        words.iter().copied().map(String::from).collect()
    }

    #[test]
    fn the_command_is_parsed_once_from_its_arguments_and_each_run_gets_its_own_copy() {
        let yabai_command = YabaiCommandOfAHotkey::parsed_from_its_arguments(arguments(&[
            "window", "focus", "west",
        ]))
        .unwrap();

        for _ in 0..2 {
            assert!(matches!(
                yabai_command.command_to_run(),
                DaemonCommand::Window(window_command)
                    if matches!(window_command.action, WindowAction::Focus { .. })
            ));
        }
        assert_eq!(
            yabai_command.arguments_as_written(),
            ["window", "focus", "west"]
        );
    }

    #[test]
    fn a_hotkey_command_or_arguments_that_do_not_parse_are_refused() {
        assert!(
            YabaiCommandOfAHotkey::parsed_from_its_arguments(arguments(&["hotkey", "list"]))
                .unwrap_err()
                .contains("a hotkey cannot run a hotkey command")
        );
        assert!(
            YabaiCommandOfAHotkey::parsed_from_its_arguments(arguments(&[
                "window",
                "--frobnicate"
            ]))
            .unwrap_err()
            .starts_with("`window --frobnicate` is not a yabai command")
        );
    }

    #[test]
    fn it_crosses_the_socket_as_its_arguments_and_is_parsed_again_on_the_way_in() {
        let yabai_command =
            YabaiCommandOfAHotkey::parsed_from_its_arguments(arguments(&["space", "focus", "2"]))
                .unwrap();

        let json = serde_json::to_string(&yabai_command).unwrap();
        assert_eq!(json, r#"["space","focus","2"]"#);

        let decoded: YabaiCommandOfAHotkey = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, yabai_command);
        assert!(matches!(decoded.command_to_run(), DaemonCommand::Space(_)));
        assert!(serde_json::from_str::<YabaiCommandOfAHotkey>(r#"["hotkey","list"]"#).is_err());
    }
}
