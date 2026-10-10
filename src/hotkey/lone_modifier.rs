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
        &EVERY_LONE_MODIFIER_KEY[self as usize - 1]
    }
}
