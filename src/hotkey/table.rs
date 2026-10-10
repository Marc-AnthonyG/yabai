use std::sync::Mutex;

use crate::hotkey::action::HotkeyAction;
use crate::hotkey::chord::{Chord, HotkeyModifiers};

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Hotkey {
    pub(crate) chord: Chord,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) action: HotkeyAction,
}

#[derive(Default, Debug)]
pub(crate) struct HotkeyTable {
    hotkeys_in_listing_order: Vec<Hotkey>,
}

pub(crate) static HOTKEYS_THE_KEYBOARD_TAP_MATCHES: Mutex<HotkeyTable> =
    Mutex::new(HotkeyTable::new());

impl HotkeyTable {
    pub(crate) const fn new() -> HotkeyTable {
        HotkeyTable {
            hotkeys_in_listing_order: Vec::new(),
        }
    }

    pub(crate) fn hotkeys_in_listing_order(&self) -> &[Hotkey] {
        &self.hotkeys_in_listing_order
    }

    pub(crate) fn add_replacing_any_with_the_same_chord_or_label(&mut self, hotkey: Hotkey) {
        let is_replaced_by_the_new_hotkey = |existing: &Hotkey| {
            existing.chord == hotkey.chord
                || (!hotkey.label.is_empty() && existing.label == hotkey.label)
        };
        let position_of_the_first_replaced = self
            .hotkeys_in_listing_order
            .iter()
            .position(is_replaced_by_the_new_hotkey);
        self.hotkeys_in_listing_order
            .retain(|existing| !is_replaced_by_the_new_hotkey(existing));
        match position_of_the_first_replaced {
            Some(position) => self.hotkeys_in_listing_order.insert(position, hotkey),
            None => self.hotkeys_in_listing_order.push(hotkey),
        }
    }

    pub(crate) fn remove_at_listing_index(&mut self, index: usize) -> bool {
        if index >= self.hotkeys_in_listing_order.len() {
            return false;
        }
        self.hotkeys_in_listing_order.remove(index);
        true
    }

    pub(crate) fn remove_with_label(&mut self, label: &str) -> bool {
        let count_before = self.hotkeys_in_listing_order.len();
        self.hotkeys_in_listing_order
            .retain(|hotkey| hotkey.label != label);
        self.hotkeys_in_listing_order.len() != count_before
    }

    pub(crate) fn remove_every_hotkey(&mut self) {
        self.hotkeys_in_listing_order.clear();
    }

    pub(crate) fn hotkey_pressed_by(
        &self,
        ansi_virtual_keycode: u16,
        modifiers_held: HotkeyModifiers,
    ) -> Option<&Hotkey> {
        self.hotkeys_in_listing_order
            .iter()
            .filter(|hotkey| {
                hotkey
                    .chord
                    .is_pressed_by(ansi_virtual_keycode, modifiers_held)
            })
            .max_by_key(|hotkey| hotkey.chord.names_fn())
    }
}
