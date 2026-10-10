use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use crate::hotkey::lone_modifier::LoneModifier;

const LONGEST_A_LONE_MODIFIER_MAY_BE_HELD_TO_COUNT_AS_A_TAP_IN_NANOSECONDS: u64 = 1_000_000_000;
const NO_LONE_MODIFIER_IS_ARMED: u8 = 0;

pub(crate) struct LoneModifierTapDetector {
    armed_lone_modifier: AtomicU8,
    armed_at_nanoseconds: AtomicU64,
}

impl LoneModifierTapDetector {
    pub(crate) const fn new() -> LoneModifierTapDetector {
        LoneModifierTapDetector {
            armed_lone_modifier: AtomicU8::new(NO_LONE_MODIFIER_IS_ARMED),
            armed_at_nanoseconds: AtomicU64::new(0),
        }
    }

    pub(crate) fn lone_modifier_tapped_by_flags_change(
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
            self.armed_at_nanoseconds
                .store(now_in_nanoseconds, Ordering::Relaxed);
            self.armed_lone_modifier
                .store(changed_lone_modifier as u8, Ordering::Relaxed);
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
}
