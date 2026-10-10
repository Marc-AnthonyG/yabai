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
