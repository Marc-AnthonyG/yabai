#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum LoneModifier {
    LeftCommand = 1,
    RightCommand = 2,
    LeftAlt = 3,
    RightAlt = 4,
    LeftControl = 5,
    RightControl = 6,
    LeftShift = 7,
    RightShift = 8,
}

struct LoneModifierKey {
    lone_modifier: LoneModifier,
    name: &'static str,
    flags_changed_keycode: u16,
    device_dependent_flag_while_held: u64,
}

static EVERY_LONE_MODIFIER_KEY: [LoneModifierKey; 8] = [
    LoneModifierKey {
        lone_modifier: LoneModifier::LeftCommand,
        name: "lcmd",
        flags_changed_keycode: 55,
        device_dependent_flag_while_held: 0x8,
    },
    LoneModifierKey {
        lone_modifier: LoneModifier::RightCommand,
        name: "rcmd",
        flags_changed_keycode: 54,
        device_dependent_flag_while_held: 0x10,
    },
    LoneModifierKey {
        lone_modifier: LoneModifier::LeftAlt,
        name: "lalt",
        flags_changed_keycode: 58,
        device_dependent_flag_while_held: 0x20,
    },
    LoneModifierKey {
        lone_modifier: LoneModifier::RightAlt,
        name: "ralt",
        flags_changed_keycode: 61,
        device_dependent_flag_while_held: 0x40,
    },
    LoneModifierKey {
        lone_modifier: LoneModifier::LeftControl,
        name: "lctrl",
        flags_changed_keycode: 59,
        device_dependent_flag_while_held: 0x1,
    },
    LoneModifierKey {
        lone_modifier: LoneModifier::RightControl,
        name: "rctrl",
        flags_changed_keycode: 62,
        device_dependent_flag_while_held: 0x2000,
    },
    LoneModifierKey {
        lone_modifier: LoneModifier::LeftShift,
        name: "lshift",
        flags_changed_keycode: 56,
        device_dependent_flag_while_held: 0x2,
    },
    LoneModifierKey {
        lone_modifier: LoneModifier::RightShift,
        name: "rshift",
        flags_changed_keycode: 60,
        device_dependent_flag_while_held: 0x4,
    },
];

pub(crate) const EVERY_LONE_MODIFIER_NAME_IN_SHORT: &str =
    "lcmd, rcmd, lalt, ralt, lctrl, rctrl, lshift or rshift";

impl LoneModifier {
    pub(crate) fn named(name: &str) -> Option<LoneModifier> {
        EVERY_LONE_MODIFIER_KEY
            .iter()
            .find(|key| key.name == name)
            .map(|key| key.lone_modifier)
    }

    pub(crate) fn of_flags_changed_keycode(keycode: u16) -> Option<LoneModifier> {
        EVERY_LONE_MODIFIER_KEY
            .iter()
            .find(|key| key.flags_changed_keycode == keycode)
            .map(|key| key.lone_modifier)
    }

    pub(crate) fn of_discriminant(discriminant: u8) -> Option<LoneModifier> {
        EVERY_LONE_MODIFIER_KEY
            .iter()
            .find(|key| key.lone_modifier as u8 == discriminant)
            .map(|key| key.lone_modifier)
    }

    pub(crate) fn name(self) -> &'static str {
        self.key().name
    }

    pub(crate) fn is_held_according_to_event_flags(self, event_flags: u64) -> bool {
        event_flags & self.key().device_dependent_flag_while_held != 0
    }

    fn key(self) -> &'static LoneModifierKey {
        EVERY_LONE_MODIFIER_KEY
            .iter()
            .find(|key| key.lone_modifier == self)
            .expect("every lone modifier has a row in EVERY_LONE_MODIFIER_KEY")
    }
}

#[cfg(test)]
mod tests {
    use super::{EVERY_LONE_MODIFIER_KEY, EVERY_LONE_MODIFIER_NAME_IN_SHORT, LoneModifier};

    const EVERY_LONE_MODIFIER_WITH_ITS_NAME_KEYCODE_AND_DEVICE_DEPENDENT_FLAG: [(
        LoneModifier,
        &str,
        u16,
        u64,
    ); 8] = [
        (LoneModifier::LeftCommand, "lcmd", 55, 0x8),
        (LoneModifier::RightCommand, "rcmd", 54, 0x10),
        (LoneModifier::LeftAlt, "lalt", 58, 0x20),
        (LoneModifier::RightAlt, "ralt", 61, 0x40),
        (LoneModifier::LeftControl, "lctrl", 59, 0x1),
        (LoneModifier::RightControl, "rctrl", 62, 0x2000),
        (LoneModifier::LeftShift, "lshift", 56, 0x2),
        (LoneModifier::RightShift, "rshift", 60, 0x4),
    ];

    #[test]
    fn each_lone_modifier_name_maps_to_its_own_physical_key() {
        for (lone_modifier, name, flags_changed_keycode, device_dependent_flag_while_held) in
            EVERY_LONE_MODIFIER_WITH_ITS_NAME_KEYCODE_AND_DEVICE_DEPENDENT_FLAG
        {
            assert_eq!(LoneModifier::named(name), Some(lone_modifier));
            assert_eq!(lone_modifier.name(), name);
            assert_eq!(
                LoneModifier::of_flags_changed_keycode(flags_changed_keycode),
                Some(lone_modifier)
            );
            assert!(
                lone_modifier.is_held_according_to_event_flags(device_dependent_flag_while_held),
                "{name:?}"
            );
        }
    }

    #[test]
    fn a_lone_modifier_is_held_only_by_the_device_dependent_flag_of_its_own_side() {
        for (lone_modifier, name, ..) in
            EVERY_LONE_MODIFIER_WITH_ITS_NAME_KEYCODE_AND_DEVICE_DEPENDENT_FLAG
        {
            for (other_lone_modifier, _, _, flag_of_the_other_key) in
                EVERY_LONE_MODIFIER_WITH_ITS_NAME_KEYCODE_AND_DEVICE_DEPENDENT_FLAG
            {
                if other_lone_modifier != lone_modifier {
                    assert!(
                        !lone_modifier.is_held_according_to_event_flags(flag_of_the_other_key),
                        "{name:?} held by the flag of {other_lone_modifier:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn left_command_held_never_counts_as_right_command_held() {
        const COMMAND_EVENT_FLAG: u64 = 0x10_0000;
        const LEFT_COMMAND_DEVICE_DEPENDENT_FLAG: u64 = 0x8;

        assert!(
            !LoneModifier::RightCommand.is_held_according_to_event_flags(
                COMMAND_EVENT_FLAG | LEFT_COMMAND_DEVICE_DEPENDENT_FLAG
            )
        );
    }

    #[test]
    fn a_flags_change_of_a_key_that_is_not_a_side_specific_modifier_is_no_lone_modifier() {
        const CAPS_LOCK_KEYCODE: u16 = 57;
        const FN_KEYCODE: u16 = 63;
        const H_KEYCODE: u16 = 0x04;

        for keycode in [CAPS_LOCK_KEYCODE, FN_KEYCODE, H_KEYCODE] {
            assert_eq!(LoneModifier::of_flags_changed_keycode(keycode), None);
        }
        assert_eq!(LoneModifier::named("cmd"), None);
    }

    #[test]
    fn every_lone_modifier_reads_back_from_its_discriminant_and_zero_is_none() {
        for key in &EVERY_LONE_MODIFIER_KEY {
            assert_eq!(
                LoneModifier::of_discriminant(key.lone_modifier as u8),
                Some(key.lone_modifier)
            );
        }
        assert_eq!(LoneModifier::of_discriminant(0), None);
    }

    #[test]
    fn the_lone_modifier_list_refusals_print_names_every_lone_modifier() {
        let names_listed: Vec<&str> = EVERY_LONE_MODIFIER_NAME_IN_SHORT
            .split(", ")
            .flat_map(|listed| listed.split(" or "))
            .collect();
        let every_lone_modifier_name: Vec<&str> =
            EVERY_LONE_MODIFIER_KEY.iter().map(|key| key.name).collect();
        assert_eq!(names_listed, every_lone_modifier_name);
    }
}
