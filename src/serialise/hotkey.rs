use serde::Serialize;

use crate::hotkey::action::HotkeyAction;
use crate::hotkey::table::{Hotkey, HotkeyTable};

#[derive(Serialize, Debug)]
pub(crate) struct HotkeySnapshot {
    pub(crate) index: usize,
    pub(crate) label: String,
    pub(crate) chord: String,
    pub(crate) description: String,
    pub(crate) action: HotkeyAction,
}

pub(crate) fn every_hotkey_as_pretty_json(hotkey_table: &HotkeyTable) -> String {
    let snapshots: Vec<HotkeySnapshot> = hotkey_table
        .hotkeys_in_listing_order()
        .iter()
        .enumerate()
        .map(|(index, hotkey)| snapshot_of_hotkey(index, hotkey))
        .collect();
    serde_json::to_string_pretty(&snapshots).unwrap_or_default()
}

fn snapshot_of_hotkey(index: usize, hotkey: &Hotkey) -> HotkeySnapshot {
    HotkeySnapshot {
        index,
        label: hotkey.label.clone(),
        chord: hotkey.chord.to_string(),
        description: hotkey.description.clone(),
        action: hotkey.action.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::every_hotkey_as_pretty_json;
    use crate::command::yabai_command_of_a_hotkey::YabaiCommandOfAHotkey;
    use crate::hotkey::action::HotkeyAction;
    use crate::hotkey::table::{Hotkey, HotkeyTable};

    #[test]
    fn hotkeys_list_their_index_label_chord_description_and_action_in_that_order() {
        let mut hotkey_table = HotkeyTable::new();
        hotkey_table.add_replacing_any_with_the_same_chord_or_label(Hotkey {
            chord: "cmd+alt-h".parse().unwrap(),
            label: String::from("focus-west"),
            description: String::from("Focus the window to the west"),
            action: HotkeyAction::Yabai(
                YabaiCommandOfAHotkey::parsed_from_its_arguments(vec![
                    String::from("window"),
                    String::from("focus"),
                    String::from("west"),
                ])
                .unwrap(),
            ),
        });
        hotkey_table.add_replacing_any_with_the_same_chord_or_label(Hotkey {
            chord: "rcmd".parse().unwrap(),
            label: String::new(),
            description: String::new(),
            action: HotkeyAction::Shell(String::from("yabai input-source cycle")),
        });

        let listing: serde_json::Value =
            serde_json::from_str(&every_hotkey_as_pretty_json(&hotkey_table)).unwrap();

        assert_eq!(
            listing,
            serde_json::json!([
                {
                    "index": 0,
                    "label": "focus-west",
                    "chord": "alt + cmd - h",
                    "description": "Focus the window to the west",
                    "action": { "yabai": ["window", "focus", "west"] }
                },
                {
                    "index": 1,
                    "label": "",
                    "chord": "rcmd",
                    "description": "",
                    "action": { "shell": "yabai input-source cycle" }
                }
            ])
        );
        let keys_in_order: Vec<&str> = listing[0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys_in_order,
            ["index", "label", "chord", "description", "action"]
        );
    }

    #[test]
    fn no_hotkey_lists_as_an_empty_array() {
        assert_eq!(every_hotkey_as_pretty_json(&HotkeyTable::new()), "[]");
    }
}
