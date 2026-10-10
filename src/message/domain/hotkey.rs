use std::sync::{Mutex, PoisonError};

use crate::command::hotkey::{HotkeyCommand, HotkeyDefinition};
use crate::command::selectors::IndexOrLabelSelector;
use crate::hotkey::keyboard_tap_state::{KeyboardTapState, KeyboardTapStatus};
use crate::hotkey::table::{Hotkey, HotkeyTable};
use crate::serialise::hotkey::every_hotkey_as_pretty_json;

pub(crate) fn run_hotkey_command(
    command: HotkeyCommand,
    hotkey_table: &Mutex<HotkeyTable>,
    keyboard_tap_state: &KeyboardTapState,
) -> Result<String, String> {
    let mut hotkey_table = hotkey_table.lock().unwrap_or_else(PoisonError::into_inner);
    match command {
        HotkeyCommand::Add(definition) => {
            hotkey_table
                .add_replacing_any_with_the_same_chord_or_label(hotkey_of_definition(definition));
            if keyboard_tap_state.status() == KeyboardTapStatus::FailedToStart {
                return Err(String::from(
                    "the keyboard tap could not be created, so hotkeys will not fire; check \
                     Accessibility for yabai.",
                ));
            }
            Ok(String::new())
        }
        HotkeyCommand::Remove { all: true, .. } => {
            hotkey_table.remove_every_hotkey();
            Ok(String::new())
        }
        HotkeyCommand::Remove {
            hotkey: Some(IndexOrLabelSelector::Index(index)),
            ..
        } => {
            if !hotkey_table.remove_at_listing_index(index as usize) {
                return Err(format!("hotkey with index '{index}' not found."));
            }
            Ok(String::new())
        }
        HotkeyCommand::Remove {
            hotkey: Some(IndexOrLabelSelector::Label(label)),
            ..
        } => {
            if !hotkey_table.remove_with_label(&label) {
                return Err(format!("hotkey with label '{label}' not found."));
            }
            Ok(String::new())
        }
        HotkeyCommand::Remove { hotkey: None, .. } => Err(String::from(
            "name the hotkey to remove, or remove every hotkey with --all.",
        )),
        HotkeyCommand::List => Ok(every_hotkey_as_pretty_json(&hotkey_table) + "\n"),
    }
}

fn hotkey_of_definition(definition: HotkeyDefinition) -> Hotkey {
    Hotkey {
        chord: definition.chord,
        label: definition.label.unwrap_or_default(),
        description: definition.description.unwrap_or_default(),
        action: definition.action,
    }
}
