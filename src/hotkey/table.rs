use std::sync::Mutex;

use crate::hotkey::action::HotkeyAction;
use crate::hotkey::chord::{Chord, HotkeyModifiers};
use crate::hotkey::lone_modifier::LoneModifier;

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Hotkey {
    pub(crate) chord: Chord,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) action: HotkeyAction,
}

#[derive(Debug)]
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

    pub(crate) fn hotkey_of_lone_modifier_tap(
        &self,
        lone_modifier: LoneModifier,
    ) -> Option<&Hotkey> {
        self.hotkeys_in_listing_order
            .iter()
            .find(|hotkey| hotkey.chord == Chord::LoneModifierTap(lone_modifier))
    }

    pub(crate) fn lone_modifiers_with_a_hotkey(&self) -> impl Iterator<Item = LoneModifier> + '_ {
        self.hotkeys_in_listing_order
            .iter()
            .filter_map(|hotkey| match hotkey.chord {
                Chord::LoneModifierTap(lone_modifier) => Some(lone_modifier),
                Chord::KeyWithModifiers { .. } => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{Hotkey, HotkeyTable};
    use crate::hotkey::action::HotkeyAction;
    use crate::hotkey::chord::HotkeyModifiers;
    use crate::hotkey::lone_modifier::LoneModifier;

    const KEYCODE_OF_H: u16 = 0x04;
    const KEYCODE_OF_F2: u16 = 0x78;

    fn hotkey(chord: &str, label: &str, shell_command: &str) -> Hotkey {
        Hotkey {
            chord: chord.parse().unwrap(),
            label: String::from(label),
            description: String::new(),
            action: HotkeyAction::Shell(String::from(shell_command)),
        }
    }

    fn table_of(hotkeys: impl IntoIterator<Item = Hotkey>) -> HotkeyTable {
        let mut table = HotkeyTable::new();
        for hotkey in hotkeys {
            table.add_replacing_any_with_the_same_chord_or_label(hotkey);
        }
        table
    }

    fn shell_command_of(hotkey: &Hotkey) -> &str {
        match &hotkey.action {
            HotkeyAction::Shell(shell_command) => shell_command,
            HotkeyAction::Yabai(_) => panic!("every hotkey here runs a shell command"),
        }
    }

    fn shell_commands_in_listing_order(table: &HotkeyTable) -> Vec<&str> {
        table
            .hotkeys_in_listing_order()
            .iter()
            .map(shell_command_of)
            .collect()
    }

    fn shell_command_pressed_by(
        table: &HotkeyTable,
        ansi_virtual_keycode: u16,
        modifiers_held: HotkeyModifiers,
    ) -> Option<&str> {
        table
            .hotkey_pressed_by(ansi_virtual_keycode, modifiers_held)
            .map(shell_command_of)
    }

    #[test]
    fn hotkeys_list_in_the_order_they_were_added() {
        let table = table_of([
            hotkey("alt - h", "", "first"),
            hotkey("alt - j", "", "second"),
            hotkey("rcmd", "", "third"),
        ]);

        assert_eq!(
            shell_commands_in_listing_order(&table),
            ["first", "second", "third"]
        );
    }

    #[test]
    fn adding_a_chord_already_bound_replaces_that_hotkey_in_its_place() {
        let mut table = table_of([
            hotkey("alt - h", "", "first"),
            hotkey("alt - j", "", "second"),
            hotkey("alt - k", "", "third"),
        ]);

        table.add_replacing_any_with_the_same_chord_or_label(hotkey("alt - j", "", "replacement"));

        assert_eq!(
            shell_commands_in_listing_order(&table),
            ["first", "replacement", "third"]
        );
    }

    #[test]
    fn adding_a_chord_spelled_another_way_still_replaces_the_hotkey_bound_to_it() {
        let mut table = table_of([hotkey("alt + cmd - h", "", "first")]);

        table.add_replacing_any_with_the_same_chord_or_label(hotkey(
            "cmd+alt-h",
            "",
            "replacement",
        ));

        assert_eq!(shell_commands_in_listing_order(&table), ["replacement"]);
    }

    #[test]
    fn adding_a_hotkey_with_the_label_of_another_replaces_that_hotkey_in_its_place() {
        let mut table = table_of([
            hotkey("alt - h", "focus", "first"),
            hotkey("alt - j", "swap", "second"),
            hotkey("alt - k", "warp", "third"),
        ]);

        table.add_replacing_any_with_the_same_chord_or_label(hotkey(
            "ctrl - s",
            "swap",
            "replacement",
        ));

        assert_eq!(
            shell_commands_in_listing_order(&table),
            ["first", "replacement", "third"]
        );
        assert_eq!(
            table.hotkeys_in_listing_order()[1].chord,
            "ctrl - s".parse().unwrap()
        );
    }

    #[test]
    fn a_hotkey_replacing_one_by_chord_and_another_by_label_takes_the_place_of_the_first_of_them() {
        let mut table = table_of([
            hotkey("alt - h", "", "first"),
            hotkey("alt - j", "swap", "second"),
            hotkey("alt - k", "", "third"),
            hotkey("alt - l", "", "fourth"),
        ]);

        table.add_replacing_any_with_the_same_chord_or_label(hotkey(
            "alt - k",
            "swap",
            "replacement",
        ));

        assert_eq!(
            shell_commands_in_listing_order(&table),
            ["first", "replacement", "fourth"]
        );
    }

    #[test]
    fn hotkeys_without_a_label_never_replace_each_other_by_label() {
        let table = table_of([
            hotkey("alt - h", "", "first"),
            hotkey("alt - j", "", "second"),
        ]);

        assert_eq!(shell_commands_in_listing_order(&table), ["first", "second"]);
    }

    #[test]
    fn adding_the_same_hotkeys_again_keeps_the_listing_order() {
        let hotkeys_of_the_config_file = [
            hotkey("alt - h", "focus", "first"),
            hotkey("alt - j", "", "second"),
            hotkey("rcmd", "cycle", "third"),
        ];
        let mut table = table_of(hotkeys_of_the_config_file.clone());

        for hotkey in hotkeys_of_the_config_file {
            table.add_replacing_any_with_the_same_chord_or_label(hotkey);
        }

        assert_eq!(
            shell_commands_in_listing_order(&table),
            ["first", "second", "third"]
        );
    }

    #[test]
    fn removing_at_a_listing_index_keeps_the_order_of_the_others() {
        let mut table = table_of([
            hotkey("alt - h", "", "first"),
            hotkey("alt - j", "", "second"),
            hotkey("alt - k", "", "third"),
        ]);

        assert!(table.remove_at_listing_index(1));

        assert_eq!(shell_commands_in_listing_order(&table), ["first", "third"]);
    }

    #[test]
    fn removing_at_an_index_past_the_listing_changes_nothing_and_says_so() {
        let mut table = table_of([hotkey("alt - h", "", "first")]);

        assert!(!table.remove_at_listing_index(1));

        assert_eq!(shell_commands_in_listing_order(&table), ["first"]);
    }

    #[test]
    fn removing_by_label_removes_that_hotkey_and_says_whether_one_was_there() {
        let mut table = table_of([
            hotkey("alt - h", "focus", "first"),
            hotkey("alt - j", "swap", "second"),
            hotkey("alt - k", "", "third"),
        ]);

        assert!(table.remove_with_label("swap"));
        assert!(!table.remove_with_label("swap"));

        assert_eq!(shell_commands_in_listing_order(&table), ["first", "third"]);
    }

    #[test]
    fn removing_every_hotkey_empties_the_table() {
        let mut table = table_of([hotkey("alt - h", "", "first"), hotkey("rcmd", "", "second")]);

        table.remove_every_hotkey();

        assert!(table.hotkeys_in_listing_order().is_empty());
    }

    #[test]
    fn a_key_down_presses_the_hotkey_whose_chord_names_exactly_the_modifiers_held() {
        let table = table_of([
            hotkey("alt - h", "", "focus"),
            hotkey("shift + alt - h", "", "swap"),
        ]);

        assert_eq!(
            shell_command_pressed_by(&table, KEYCODE_OF_H, HotkeyModifiers::ALT),
            Some("focus")
        );
        assert_eq!(
            shell_command_pressed_by(
                &table,
                KEYCODE_OF_H,
                HotkeyModifiers::ALT | HotkeyModifiers::SHIFT
            ),
            Some("swap")
        );
        assert_eq!(
            shell_command_pressed_by(&table, KEYCODE_OF_H, HotkeyModifiers::CMD),
            None
        );
    }

    #[test]
    fn a_chord_naming_fn_wins_over_the_same_chord_without_it_while_fn_is_held_whatever_the_order() {
        let alt_f2 = hotkey("alt - f2", "", "without fn");
        let fn_alt_f2 = hotkey("fn + alt - f2", "", "with fn");

        for table in [
            table_of([alt_f2.clone(), fn_alt_f2.clone()]),
            table_of([fn_alt_f2, alt_f2]),
        ] {
            assert_eq!(
                shell_command_pressed_by(
                    &table,
                    KEYCODE_OF_F2,
                    HotkeyModifiers::ALT | HotkeyModifiers::FN
                ),
                Some("with fn")
            );
            assert_eq!(
                shell_command_pressed_by(&table, KEYCODE_OF_F2, HotkeyModifiers::ALT),
                Some("without fn")
            );
        }
    }

    #[test]
    fn a_function_key_bound_without_fn_fires_while_the_fn_flag_it_carries_is_held() {
        let table = table_of([hotkey("alt - f2", "", "without fn")]);

        assert_eq!(
            shell_command_pressed_by(
                &table,
                KEYCODE_OF_F2,
                HotkeyModifiers::ALT | HotkeyModifiers::FN
            ),
            Some("without fn")
        );
    }

    #[test]
    fn a_lone_modifier_tap_finds_the_hotkey_bound_to_that_side_only() {
        let table = table_of([hotkey("alt - h", "", "focus"), hotkey("rcmd", "", "cycle")]);

        assert_eq!(
            table
                .hotkey_of_lone_modifier_tap(LoneModifier::RightCommand)
                .map(|hotkey| hotkey.chord.to_string()),
            Some(String::from("rcmd"))
        );
        assert_eq!(
            table.hotkey_of_lone_modifier_tap(LoneModifier::LeftCommand),
            None
        );
    }

    #[test]
    fn the_lone_modifiers_with_a_hotkey_are_those_a_lone_modifier_chord_binds() {
        let table = table_of([
            hotkey("alt - h", "", "focus"),
            hotkey("rcmd", "", "cycle"),
            hotkey("shift + cmd - k", "", "swap"),
            hotkey("lshift", "", "caps"),
        ]);

        assert_eq!(
            table.lone_modifiers_with_a_hotkey().collect::<Vec<_>>(),
            [LoneModifier::RightCommand, LoneModifier::LeftShift]
        );
        assert_eq!(HotkeyTable::new().lone_modifiers_with_a_hotkey().count(), 0);
    }
}
