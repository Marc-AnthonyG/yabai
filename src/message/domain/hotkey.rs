use std::sync::{Mutex, MutexGuard, PoisonError};

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
    match command {
        HotkeyCommand::Add(definition) => {
            let hotkey = hotkey_of_definition(definition);
            change_the_hotkey_table(hotkey_table, keyboard_tap_state, |hotkey_table| {
                hotkey_table.add_replacing_any_with_the_same_chord_or_label(hotkey)
            });
            if keyboard_tap_state.status() == KeyboardTapStatus::FailedToStart {
                return Err(String::from(
                    "the keyboard tap could not be created, so hotkeys will not fire; check \
                     Accessibility for yabai.",
                ));
            }
            Ok(String::new())
        }
        HotkeyCommand::Remove { all: true, .. } => {
            change_the_hotkey_table(
                hotkey_table,
                keyboard_tap_state,
                HotkeyTable::remove_every_hotkey,
            );
            Ok(String::new())
        }
        HotkeyCommand::Remove {
            hotkey: Some(IndexOrLabelSelector::Index(index)),
            ..
        } => {
            let was_removed =
                change_the_hotkey_table(hotkey_table, keyboard_tap_state, |hotkey_table| {
                    hotkey_table.remove_at_listing_index(index as usize)
                });
            if !was_removed {
                return Err(format!("hotkey with index '{index}' not found."));
            }
            Ok(String::new())
        }
        HotkeyCommand::Remove {
            hotkey: Some(IndexOrLabelSelector::Label(label)),
            ..
        } => {
            let was_removed =
                change_the_hotkey_table(hotkey_table, keyboard_tap_state, |hotkey_table| {
                    hotkey_table.remove_with_label(&label)
                });
            if !was_removed {
                return Err(format!("hotkey with label '{label}' not found."));
            }
            Ok(String::new())
        }
        HotkeyCommand::Remove { hotkey: None, .. } => Err(String::from(
            "name the hotkey to remove, or remove every hotkey with --all.",
        )),
        HotkeyCommand::List => {
            let every_hotkey_in_listing_order = lock_the_hotkey_table(hotkey_table)
                .hotkeys_in_listing_order()
                .to_vec();
            Ok(every_hotkey_as_pretty_json(&every_hotkey_in_listing_order) + "\n")
        }
    }
}

fn change_the_hotkey_table<Outcome>(
    hotkey_table: &Mutex<HotkeyTable>,
    keyboard_tap_state: &KeyboardTapState,
    change: impl FnOnce(&mut HotkeyTable) -> Outcome,
) -> Outcome {
    let mut hotkey_table = lock_the_hotkey_table(hotkey_table);
    let outcome = change(&mut hotkey_table);
    keyboard_tap_state
        .lone_modifier_tap_detector
        .arm_only_for_the_lone_modifiers_with_a_hotkey(hotkey_table.lone_modifiers_with_a_hotkey());
    outcome
}

fn lock_the_hotkey_table(hotkey_table: &Mutex<HotkeyTable>) -> MutexGuard<'_, HotkeyTable> {
    hotkey_table.lock().unwrap_or_else(PoisonError::into_inner)
}

fn hotkey_of_definition(definition: HotkeyDefinition) -> Hotkey {
    Hotkey {
        chord: definition.chord,
        label: definition.label.unwrap_or_default(),
        description: definition.description.unwrap_or_default(),
        action: definition.action,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::run_hotkey_command;
    use crate::command::hotkey::HotkeyCommand;
    use crate::command::{DaemonCommand, parse_daemon_command};
    use crate::hotkey::action::HotkeyAction;
    use crate::hotkey::keyboard_tap_state::{KeyboardTapState, KeyboardTapStatus};
    use crate::hotkey::lone_modifier::LoneModifier;
    use crate::hotkey::lone_modifier_tap::KeyboardTapEventSeenByTheLoneModifierTapDetector;
    use crate::hotkey::table::HotkeyTable;

    fn hotkey_command_of(arguments: &[&str]) -> HotkeyCommand {
        match parse_daemon_command(&[&["hotkey"], arguments].concat()) {
            Ok(DaemonCommand::Hotkey(hotkey_command)) => hotkey_command,
            outcome => panic!("{arguments:?} should parse as a hotkey command: {outcome:?}"),
        }
    }

    fn keyboard_tap_state_with_status(status: KeyboardTapStatus) -> KeyboardTapState {
        let keyboard_tap_state = KeyboardTapState::new();
        keyboard_tap_state.record_status(status);
        keyboard_tap_state
    }

    fn run_every_hotkey_command(
        command_lines: &[&[&str]],
        hotkey_table: &Mutex<HotkeyTable>,
        keyboard_tap_state: &KeyboardTapState,
    ) {
        for arguments in command_lines {
            run_hotkey_command(
                hotkey_command_of(arguments),
                hotkey_table,
                keyboard_tap_state,
            )
            .unwrap();
        }
    }

    fn chords_in_listing_order(hotkey_table: &Mutex<HotkeyTable>) -> Vec<String> {
        hotkey_table
            .lock()
            .unwrap()
            .hotkeys_in_listing_order()
            .iter()
            .map(|hotkey| hotkey.chord.to_string())
            .collect()
    }

    #[test]
    fn adding_a_hotkey_stores_it_with_an_unset_label_and_description_left_empty() {
        let hotkey_table = Mutex::new(HotkeyTable::new());
        let keyboard_tap_state = keyboard_tap_state_with_status(KeyboardTapStatus::Running);

        let outcome = run_hotkey_command(
            hotkey_command_of(&["add", "rcmd", "--shell", "yabai input-source cycle"]),
            &hotkey_table,
            &keyboard_tap_state,
        );

        assert_eq!(outcome, Ok(String::new()));
        let hotkey_table = hotkey_table.lock().unwrap();
        let hotkey = &hotkey_table.hotkeys_in_listing_order()[0];
        assert_eq!(hotkey.chord.to_string(), "rcmd");
        assert_eq!(hotkey.label, "");
        assert_eq!(hotkey.description, "");
        assert_eq!(
            hotkey.action,
            HotkeyAction::Shell(String::from("yabai input-source cycle"))
        );
    }

    #[test]
    fn adding_a_hotkey_while_the_keyboard_tap_is_still_starting_succeeds() {
        let hotkey_table = Mutex::new(HotkeyTable::new());
        let keyboard_tap_state = keyboard_tap_state_with_status(KeyboardTapStatus::StillStarting);

        let outcome = run_hotkey_command(
            hotkey_command_of(&["add", "alt - h", "--", "window", "focus", "west"]),
            &hotkey_table,
            &keyboard_tap_state,
        );

        assert_eq!(outcome, Ok(String::new()));
        assert_eq!(chords_in_listing_order(&hotkey_table), ["alt - h"]);
    }

    #[test]
    fn adding_a_hotkey_after_the_keyboard_tap_failed_to_start_stores_it_but_fails() {
        let hotkey_table = Mutex::new(HotkeyTable::new());
        let keyboard_tap_state = keyboard_tap_state_with_status(KeyboardTapStatus::FailedToStart);

        let outcome = run_hotkey_command(
            hotkey_command_of(&["add", "alt - h", "--", "window", "focus", "west"]),
            &hotkey_table,
            &keyboard_tap_state,
        );

        assert_eq!(
            outcome,
            Err(String::from(
                "the keyboard tap could not be created, so hotkeys will not fire; check \
                 Accessibility for yabai."
            ))
        );
        assert_eq!(chords_in_listing_order(&hotkey_table), ["alt - h"]);
    }

    #[test]
    fn running_the_same_hotkey_add_lines_again_keeps_the_listing_order() {
        let hotkey_table = Mutex::new(HotkeyTable::new());
        let keyboard_tap_state = keyboard_tap_state_with_status(KeyboardTapStatus::Running);
        let config_file_lines: &[&[&str]] = &[
            &[
                "add",
                "alt - h",
                "--label",
                "focus-west",
                "--",
                "window",
                "focus",
                "west",
            ],
            &["add", "shift + alt - h", "--", "window", "swap", "west"],
            &["add", "rcmd", "--shell", "yabai input-source cycle"],
        ];

        run_every_hotkey_command(config_file_lines, &hotkey_table, &keyboard_tap_state);
        run_every_hotkey_command(config_file_lines, &hotkey_table, &keyboard_tap_state);

        assert_eq!(
            chords_in_listing_order(&hotkey_table),
            ["alt - h", "alt + shift - h", "rcmd"]
        );
    }

    #[test]
    fn hotkeys_are_removed_by_listing_index_by_label_or_all_at_once() {
        let hotkey_table = Mutex::new(HotkeyTable::new());
        let keyboard_tap_state = keyboard_tap_state_with_status(KeyboardTapStatus::Running);
        run_every_hotkey_command(
            &[
                &["add", "alt - h", "--", "window", "focus", "west"],
                &[
                    "add",
                    "alt - j",
                    "--label",
                    "focus-south",
                    "--",
                    "window",
                    "focus",
                    "south",
                ],
                &["add", "alt - k", "--", "window", "focus", "north"],
                &["add", "alt - l", "--", "window", "focus", "east"],
            ],
            &hotkey_table,
            &keyboard_tap_state,
        );

        run_every_hotkey_command(
            &[&["remove", "focus-south"], &["remove", "1"]],
            &hotkey_table,
            &keyboard_tap_state,
        );
        assert_eq!(
            chords_in_listing_order(&hotkey_table),
            ["alt - h", "alt - l"]
        );

        run_every_hotkey_command(&[&["remove", "--all"]], &hotkey_table, &keyboard_tap_state);
        assert!(chords_in_listing_order(&hotkey_table).is_empty());
    }

    #[test]
    fn removing_a_hotkey_that_is_not_there_fails_naming_it() {
        let hotkey_table = Mutex::new(HotkeyTable::new());
        let keyboard_tap_state = keyboard_tap_state_with_status(KeyboardTapStatus::Running);

        assert_eq!(
            run_hotkey_command(
                hotkey_command_of(&["remove", "3"]),
                &hotkey_table,
                &keyboard_tap_state
            ),
            Err(String::from("hotkey with index '3' not found."))
        );
        assert_eq!(
            run_hotkey_command(
                hotkey_command_of(&["remove", "focus-west"]),
                &hotkey_table,
                &keyboard_tap_state
            ),
            Err(String::from("hotkey with label 'focus-west' not found."))
        );
        assert_eq!(
            run_hotkey_command(
                HotkeyCommand::Remove {
                    hotkey: None,
                    all: false
                },
                &hotkey_table,
                &keyboard_tap_state
            ),
            Err(String::from(
                "name the hotkey to remove, or remove every hotkey with --all."
            ))
        );
    }

    #[test]
    fn hotkey_list_prints_the_listing_followed_by_a_newline() {
        let hotkey_table = Mutex::new(HotkeyTable::new());
        let keyboard_tap_state = keyboard_tap_state_with_status(KeyboardTapStatus::Running);
        run_every_hotkey_command(
            &[&["add", "alt - h", "--", "window", "focus", "west"]],
            &hotkey_table,
            &keyboard_tap_state,
        );

        let listing = run_hotkey_command(
            hotkey_command_of(&["list"]),
            &hotkey_table,
            &keyboard_tap_state,
        )
        .unwrap();

        assert!(listing.ends_with("]\n"), "{listing:?}");
        let listing: serde_json::Value = serde_json::from_str(&listing).unwrap();
        assert_eq!(listing[0]["chord"], "alt - h");
    }

    fn right_command_tap_seen_by(keyboard_tap_state: &KeyboardTapState) -> Option<LoneModifier> {
        const RIGHT_COMMAND_KEYCODE: u16 = 54;
        const RIGHT_COMMAND_HELD_EVENT_FLAGS: u64 = 0x10_0000 | 0x10;
        const PRESSED_AT_NANOSECONDS: u64 = 5_000_000_000;
        const RELEASED_A_TENTH_OF_A_SECOND_LATER: u64 = PRESSED_AT_NANOSECONDS + 100_000_000;

        let detector = &keyboard_tap_state.lone_modifier_tap_detector;
        detector.lone_modifier_tapped_by(
            KeyboardTapEventSeenByTheLoneModifierTapDetector::FlagsChanged {
                flags_changed_keycode: RIGHT_COMMAND_KEYCODE,
                event_flags: RIGHT_COMMAND_HELD_EVENT_FLAGS,
                now_in_nanoseconds: PRESSED_AT_NANOSECONDS,
            },
        );
        detector.lone_modifier_tapped_by(
            KeyboardTapEventSeenByTheLoneModifierTapDetector::FlagsChanged {
                flags_changed_keycode: RIGHT_COMMAND_KEYCODE,
                event_flags: 0,
                now_in_nanoseconds: RELEASED_A_TENTH_OF_A_SECOND_LATER,
            },
        )
    }

    #[test]
    fn a_lone_modifier_is_tapped_only_while_a_hotkey_binds_it() {
        let hotkey_table = Mutex::new(HotkeyTable::new());
        let keyboard_tap_state = keyboard_tap_state_with_status(KeyboardTapStatus::Running);

        run_every_hotkey_command(
            &[&["add", "alt - h", "--", "window", "focus", "west"]],
            &hotkey_table,
            &keyboard_tap_state,
        );
        assert_eq!(right_command_tap_seen_by(&keyboard_tap_state), None);

        run_every_hotkey_command(
            &[&[
                "add",
                "rcmd",
                "--label",
                "cycle",
                "--",
                "input-source",
                "cycle",
            ]],
            &hotkey_table,
            &keyboard_tap_state,
        );
        assert_eq!(
            right_command_tap_seen_by(&keyboard_tap_state),
            Some(LoneModifier::RightCommand)
        );

        run_every_hotkey_command(&[&["remove", "cycle"]], &hotkey_table, &keyboard_tap_state);
        assert_eq!(right_command_tap_seen_by(&keyboard_tap_state), None);
    }
}
