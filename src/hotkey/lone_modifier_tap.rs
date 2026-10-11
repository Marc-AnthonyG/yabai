use std::sync::atomic::{AtomicU8, AtomicU16, AtomicU64, Ordering};

use crate::hotkey::lone_modifier::LoneModifier;

const LONGEST_A_LONE_MODIFIER_MAY_BE_HELD_TO_COUNT_AS_A_TAP_IN_NANOSECONDS: u64 = 1_000_000_000;
const NO_LONE_MODIFIER_IS_ARMED: u8 = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum KeyboardTapEventSeenByTheLoneModifierTapDetector {
    FlagsChanged {
        flags_changed_keycode: u16,
        event_flags: u64,
        now_in_nanoseconds: u64,
    },
    FlagsChangedUnderSecureEventInput,
    MomentumScrollingTheUserNoLongerDrives,
    AnyOtherEvent,
}

pub(crate) struct LoneModifierTapDetector {
    armed_lone_modifier: AtomicU8,
    armed_at_nanoseconds: AtomicU64,
    bit_of_each_lone_modifier_with_a_hotkey: AtomicU16,
}

impl LoneModifierTapDetector {
    pub(crate) const fn new() -> LoneModifierTapDetector {
        LoneModifierTapDetector {
            armed_lone_modifier: AtomicU8::new(NO_LONE_MODIFIER_IS_ARMED),
            armed_at_nanoseconds: AtomicU64::new(0),
            bit_of_each_lone_modifier_with_a_hotkey: AtomicU16::new(0),
        }
    }

    pub(crate) fn arm_only_for_the_lone_modifiers_with_a_hotkey(
        &self,
        lone_modifiers_with_a_hotkey: impl IntoIterator<Item = LoneModifier>,
    ) {
        let bit_of_each_lone_modifier_with_a_hotkey = lone_modifiers_with_a_hotkey
            .into_iter()
            .fold(0, |bits, lone_modifier| {
                bits | bit_of_the_lone_modifier(lone_modifier)
            });
        self.bit_of_each_lone_modifier_with_a_hotkey
            .store(bit_of_each_lone_modifier_with_a_hotkey, Ordering::Relaxed);
    }

    pub(crate) fn lone_modifier_tapped_by(
        &self,
        event: KeyboardTapEventSeenByTheLoneModifierTapDetector,
    ) -> Option<LoneModifier> {
        match event {
            KeyboardTapEventSeenByTheLoneModifierTapDetector::FlagsChanged {
                flags_changed_keycode,
                event_flags,
                now_in_nanoseconds,
            } => self.lone_modifier_tapped_by_flags_change(
                flags_changed_keycode,
                event_flags,
                now_in_nanoseconds,
            ),
            KeyboardTapEventSeenByTheLoneModifierTapDetector::MomentumScrollingTheUserNoLongerDrives => {
                None
            }
            KeyboardTapEventSeenByTheLoneModifierTapDetector::FlagsChangedUnderSecureEventInput
            | KeyboardTapEventSeenByTheLoneModifierTapDetector::AnyOtherEvent => {
                self.forget_the_armed_lone_modifier();
                None
            }
        }
    }

    fn lone_modifier_tapped_by_flags_change(
        &self,
        flags_changed_keycode: u16,
        event_flags: u64,
        now_in_nanoseconds: u64,
    ) -> Option<LoneModifier> {
        let armed_lone_modifier = LoneModifier::of_discriminant(
            self.armed_lone_modifier
                .swap(NO_LONE_MODIFIER_IS_ARMED, Ordering::Relaxed),
        );
        let changed_lone_modifier = LoneModifier::of_flags_changed_keycode(flags_changed_keycode)?;

        if changed_lone_modifier.is_held_according_to_event_flags(event_flags) {
            if self.has_a_hotkey(changed_lone_modifier) {
                self.armed_at_nanoseconds
                    .store(now_in_nanoseconds, Ordering::Relaxed);
                self.armed_lone_modifier
                    .store(changed_lone_modifier as u8, Ordering::Relaxed);
            }
            return None;
        }

        let held_for_nanoseconds =
            now_in_nanoseconds.saturating_sub(self.armed_at_nanoseconds.load(Ordering::Relaxed));
        let was_tapped_alone = armed_lone_modifier == Some(changed_lone_modifier)
            && held_for_nanoseconds
                <= LONGEST_A_LONE_MODIFIER_MAY_BE_HELD_TO_COUNT_AS_A_TAP_IN_NANOSECONDS;
        was_tapped_alone.then_some(changed_lone_modifier)
    }

    pub(crate) fn forget_the_armed_lone_modifier(&self) {
        self.armed_lone_modifier
            .store(NO_LONE_MODIFIER_IS_ARMED, Ordering::Relaxed);
    }

    fn has_a_hotkey(&self, lone_modifier: LoneModifier) -> bool {
        self.bit_of_each_lone_modifier_with_a_hotkey
            .load(Ordering::Relaxed)
            & bit_of_the_lone_modifier(lone_modifier)
            != 0
    }
}

fn bit_of_the_lone_modifier(lone_modifier: LoneModifier) -> u16 {
    1 << lone_modifier as u8
}

#[cfg(test)]
mod tests {
    use super::{KeyboardTapEventSeenByTheLoneModifierTapDetector, LoneModifierTapDetector};
    use crate::hotkey::lone_modifier::LoneModifier;

    const EVERY_LONE_MODIFIER: [LoneModifier; 8] = [
        LoneModifier::LeftCommand,
        LoneModifier::RightCommand,
        LoneModifier::LeftAlt,
        LoneModifier::RightAlt,
        LoneModifier::LeftControl,
        LoneModifier::RightControl,
        LoneModifier::LeftShift,
        LoneModifier::RightShift,
    ];

    const RIGHT_COMMAND_KEYCODE: u16 = 54;
    const LEFT_COMMAND_KEYCODE: u16 = 55;
    const LEFT_SHIFT_KEYCODE: u16 = 56;
    const CAPS_LOCK_KEYCODE: u16 = 57;
    const FN_KEYCODE: u16 = 63;

    const COMMAND_EVENT_FLAG: u64 = 0x10_0000;
    const SHIFT_EVENT_FLAG: u64 = 0x2_0000;
    const RIGHT_COMMAND_DEVICE_DEPENDENT_FLAG: u64 = 0x10;
    const LEFT_COMMAND_DEVICE_DEPENDENT_FLAG: u64 = 0x8;
    const LEFT_SHIFT_DEVICE_DEPENDENT_FLAG: u64 = 0x2;
    const NO_MODIFIER_HELD: u64 = 0;

    const PRESSED_AT_NANOSECONDS: u64 = 5_000_000_000;
    const ONE_SECOND_IN_NANOSECONDS: u64 = 1_000_000_000;
    const A_QUARTER_SECOND_LATER: u64 = PRESSED_AT_NANOSECONDS + ONE_SECOND_IN_NANOSECONDS / 4;

    fn detector_arming_for(lone_modifiers: &[LoneModifier]) -> LoneModifierTapDetector {
        let detector = LoneModifierTapDetector::new();
        detector.arm_only_for_the_lone_modifiers_with_a_hotkey(lone_modifiers.iter().copied());
        detector
    }

    fn detector_arming_for_every_lone_modifier() -> LoneModifierTapDetector {
        detector_arming_for(&EVERY_LONE_MODIFIER)
    }

    fn flags_change(
        flags_changed_keycode: u16,
        event_flags: u64,
        now_in_nanoseconds: u64,
    ) -> KeyboardTapEventSeenByTheLoneModifierTapDetector {
        KeyboardTapEventSeenByTheLoneModifierTapDetector::FlagsChanged {
            flags_changed_keycode,
            event_flags,
            now_in_nanoseconds,
        }
    }

    fn press_right_command_at(
        detector: &LoneModifierTapDetector,
        now_in_nanoseconds: u64,
    ) -> Option<LoneModifier> {
        detector.lone_modifier_tapped_by(flags_change(
            RIGHT_COMMAND_KEYCODE,
            COMMAND_EVENT_FLAG | RIGHT_COMMAND_DEVICE_DEPENDENT_FLAG,
            now_in_nanoseconds,
        ))
    }

    fn release_right_command_at(
        detector: &LoneModifierTapDetector,
        now_in_nanoseconds: u64,
    ) -> Option<LoneModifier> {
        detector.lone_modifier_tapped_by(flags_change(
            RIGHT_COMMAND_KEYCODE,
            NO_MODIFIER_HELD,
            now_in_nanoseconds,
        ))
    }

    #[test]
    fn pressing_then_releasing_a_lone_modifier_within_a_second_is_a_tap_of_it() {
        let detector = detector_arming_for_every_lone_modifier();

        assert_eq!(
            press_right_command_at(&detector, PRESSED_AT_NANOSECONDS),
            None
        );
        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            Some(LoneModifier::RightCommand)
        );
    }

    #[test]
    fn releasing_exactly_one_second_after_pressing_is_still_a_tap() {
        let detector = detector_arming_for_every_lone_modifier();

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);

        assert_eq!(
            release_right_command_at(
                &detector,
                PRESSED_AT_NANOSECONDS + ONE_SECOND_IN_NANOSECONDS
            ),
            Some(LoneModifier::RightCommand)
        );
    }

    #[test]
    fn releasing_later_than_one_second_after_pressing_is_not_a_tap() {
        let detector = detector_arming_for_every_lone_modifier();

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);

        assert_eq!(
            release_right_command_at(
                &detector,
                PRESSED_AT_NANOSECONDS + ONE_SECOND_IN_NANOSECONDS + 1
            ),
            None
        );
    }

    #[test]
    fn a_release_the_detector_saw_no_press_for_is_not_a_tap() {
        let detector = detector_arming_for_every_lone_modifier();

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }

    #[test]
    fn a_tap_fires_once_and_leaves_nothing_armed() {
        let detector = detector_arming_for_every_lone_modifier();

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);
        release_right_command_at(&detector, A_QUARTER_SECOND_LATER);

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER + 1),
            None
        );
    }

    #[test]
    fn a_key_down_mouse_down_or_scroll_the_user_drives_between_press_and_release_cancels_the_tap() {
        let detector = detector_arming_for_every_lone_modifier();

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);
        assert_eq!(
            detector.lone_modifier_tapped_by(
                KeyboardTapEventSeenByTheLoneModifierTapDetector::AnyOtherEvent
            ),
            None
        );

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }

    #[test]
    fn momentum_scrolling_between_press_and_release_keeps_the_lone_modifier_armed() {
        let detector = detector_arming_for_every_lone_modifier();

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);
        assert_eq!(
            detector.lone_modifier_tapped_by(
                KeyboardTapEventSeenByTheLoneModifierTapDetector::MomentumScrollingTheUserNoLongerDrives
            ),
            None
        );

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            Some(LoneModifier::RightCommand)
        );
    }

    #[test]
    fn pressing_another_lone_modifier_between_press_and_release_cancels_the_tap() {
        let detector = detector_arming_for_every_lone_modifier();

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);
        detector.lone_modifier_tapped_by(flags_change(
            LEFT_SHIFT_KEYCODE,
            COMMAND_EVENT_FLAG
                | RIGHT_COMMAND_DEVICE_DEPENDENT_FLAG
                | SHIFT_EVENT_FLAG
                | LEFT_SHIFT_DEVICE_DEPENDENT_FLAG,
            PRESSED_AT_NANOSECONDS + 1,
        ));

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }

    #[test]
    fn caps_lock_or_fn_between_press_and_release_cancels_the_tap() {
        for keycode_of_a_key_that_is_no_lone_modifier in [CAPS_LOCK_KEYCODE, FN_KEYCODE] {
            let detector = detector_arming_for_every_lone_modifier();

            press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);
            assert_eq!(
                detector.lone_modifier_tapped_by(flags_change(
                    keycode_of_a_key_that_is_no_lone_modifier,
                    COMMAND_EVENT_FLAG | RIGHT_COMMAND_DEVICE_DEPENDENT_FLAG,
                    PRESSED_AT_NANOSECONDS + 1,
                )),
                None
            );

            assert_eq!(
                release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
                None,
                "{keycode_of_a_key_that_is_no_lone_modifier}"
            );
        }
    }

    #[test]
    fn a_tap_of_left_command_fires_left_command_and_never_arms_right_command() {
        let detector = detector_arming_for_every_lone_modifier();
        let press_left_command = flags_change(
            LEFT_COMMAND_KEYCODE,
            COMMAND_EVENT_FLAG | LEFT_COMMAND_DEVICE_DEPENDENT_FLAG,
            PRESSED_AT_NANOSECONDS,
        );

        detector.lone_modifier_tapped_by(press_left_command);
        assert_eq!(
            detector.lone_modifier_tapped_by(flags_change(
                LEFT_COMMAND_KEYCODE,
                NO_MODIFIER_HELD,
                A_QUARTER_SECOND_LATER
            )),
            Some(LoneModifier::LeftCommand)
        );

        detector.lone_modifier_tapped_by(press_left_command);
        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }

    #[test]
    fn a_lone_modifier_pressed_under_secure_input_is_never_armed() {
        let detector = detector_arming_for_every_lone_modifier();

        detector.lone_modifier_tapped_by(
            KeyboardTapEventSeenByTheLoneModifierTapDetector::FlagsChangedUnderSecureEventInput,
        );

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }

    #[test]
    fn a_lone_modifier_released_under_secure_input_is_no_tap_and_is_disarmed() {
        let detector = detector_arming_for_every_lone_modifier();

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);
        assert_eq!(
            detector.lone_modifier_tapped_by(
                KeyboardTapEventSeenByTheLoneModifierTapDetector::FlagsChangedUnderSecureEventInput
            ),
            None
        );

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }

    #[test]
    fn forgetting_the_armed_lone_modifier_between_press_and_release_cancels_the_tap() {
        let detector = detector_arming_for_every_lone_modifier();

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);
        detector.forget_the_armed_lone_modifier();

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }

    #[test]
    fn a_lone_modifier_without_a_hotkey_is_never_armed_so_its_tap_is_no_tap() {
        let detector = detector_arming_for(&[LoneModifier::RightCommand]);
        let left_command_flags = COMMAND_EVENT_FLAG | LEFT_COMMAND_DEVICE_DEPENDENT_FLAG;

        detector.lone_modifier_tapped_by(flags_change(
            LEFT_COMMAND_KEYCODE,
            left_command_flags,
            PRESSED_AT_NANOSECONDS,
        ));

        assert_eq!(
            detector.lone_modifier_tapped_by(flags_change(
                LEFT_COMMAND_KEYCODE,
                NO_MODIFIER_HELD,
                A_QUARTER_SECOND_LATER
            )),
            None
        );
    }

    #[test]
    fn pressing_a_lone_modifier_without_a_hotkey_between_press_and_release_still_cancels_the_tap() {
        let detector = detector_arming_for(&[LoneModifier::RightCommand]);

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);
        detector.lone_modifier_tapped_by(flags_change(
            LEFT_SHIFT_KEYCODE,
            COMMAND_EVENT_FLAG
                | RIGHT_COMMAND_DEVICE_DEPENDENT_FLAG
                | SHIFT_EVENT_FLAG
                | LEFT_SHIFT_DEVICE_DEPENDENT_FLAG,
            PRESSED_AT_NANOSECONDS + 1,
        ));

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }

    #[test]
    fn a_lone_modifier_whose_hotkey_went_away_is_no_longer_armed() {
        let detector = detector_arming_for(&[LoneModifier::RightCommand]);

        detector.arm_only_for_the_lone_modifiers_with_a_hotkey([]);
        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }

    #[test]
    fn a_detector_arms_for_no_lone_modifier_until_told_which_have_a_hotkey() {
        let detector = LoneModifierTapDetector::new();

        press_right_command_at(&detector, PRESSED_AT_NANOSECONDS);

        assert_eq!(
            release_right_command_at(&detector, A_QUARTER_SECOND_LATER),
            None
        );
    }
}
