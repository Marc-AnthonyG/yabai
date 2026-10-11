use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum HotkeyAction {
    Yabai(Vec<String>),
    Shell(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum KeyDownOfAHotkey {
    Pressed,
    RepeatedWhileHeld,
}

impl HotkeyAction {
    pub(crate) fn runs_on(&self, key_down: KeyDownOfAHotkey) -> bool {
        match (self, key_down) {
            (_, KeyDownOfAHotkey::Pressed) | (HotkeyAction::Yabai(_), _) => true,
            (HotkeyAction::Shell(_), KeyDownOfAHotkey::RepeatedWhileHeld) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HotkeyAction, KeyDownOfAHotkey};

    #[test]
    fn every_action_runs_when_its_key_is_pressed() {
        for action in [
            HotkeyAction::Yabai(vec![String::from("window"), String::from("focus")]),
            HotkeyAction::Shell(String::from("open -na Terminal")),
        ] {
            assert!(action.runs_on(KeyDownOfAHotkey::Pressed), "{action:?}");
        }
    }

    #[test]
    fn a_yabai_command_runs_again_while_its_key_repeats_but_a_shell_command_does_not() {
        assert!(
            HotkeyAction::Yabai(vec![String::from("window"), String::from("focus")])
                .runs_on(KeyDownOfAHotkey::RepeatedWhileHeld)
        );
        assert!(
            !HotkeyAction::Shell(String::from("open -na Terminal"))
                .runs_on(KeyDownOfAHotkey::RepeatedWhileHeld)
        );
    }
}
