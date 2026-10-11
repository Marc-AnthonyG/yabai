use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::hotkey::key::{EVERY_KEY_NAME_IN_SHORT, HotkeyKey};
use crate::hotkey::lone_modifier::{EVERY_LONE_MODIFIER_NAME_IN_SHORT, LoneModifier};

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) struct HotkeyModifiers: u8 {
        const ALT = 0x01;
        const SHIFT = 0x02;
        const CMD = 0x04;
        const CTRL = 0x08;
        const FN = 0x10;
    }
}

const MODIFIER_NAMES_IN_CANONICAL_ORDER: [(&str, HotkeyModifiers); 5] = [
    ("alt", HotkeyModifiers::ALT),
    ("shift", HotkeyModifiers::SHIFT),
    ("cmd", HotkeyModifiers::CMD),
    ("ctrl", HotkeyModifiers::CTRL),
    ("fn", HotkeyModifiers::FN),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) enum Chord {
    KeyWithModifiers {
        modifiers: HotkeyModifiers,
        key: HotkeyKey,
    },
    LoneModifierTap(LoneModifier),
}

impl Chord {
    pub(crate) fn is_pressed_by(
        self,
        ansi_virtual_keycode: u16,
        modifiers_held: HotkeyModifiers,
    ) -> bool {
        let Chord::KeyWithModifiers { modifiers, key } = self else {
            return false;
        };
        let modifiers_compared = if modifiers.contains(HotkeyModifiers::FN) {
            HotkeyModifiers::all()
        } else {
            HotkeyModifiers::all() - HotkeyModifiers::FN
        };
        key.ansi_virtual_keycode() == ansi_virtual_keycode
            && modifiers_held & modifiers_compared == modifiers
    }

    pub(crate) fn names_fn(self) -> bool {
        matches!(self, Chord::KeyWithModifiers { modifiers, .. } if modifiers.contains(HotkeyModifiers::FN))
    }
}

impl FromStr for Chord {
    type Err = String;

    fn from_str(text: &str) -> Result<Chord, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err(String::from("a chord cannot be empty"));
        }
        let Some((modifier_names, key_name)) = text.split_once('-') else {
            return chord_without_a_dash(text);
        };
        let modifiers = parse_modifiers_joined_by_plus(modifier_names, text)?;
        let key = parse_key(key_name.trim())?;
        Ok(Chord::KeyWithModifiers { modifiers, key })
    }
}

fn chord_without_a_dash(text: &str) -> Result<Chord, String> {
    if let Some(lone_modifier) = LoneModifier::named(text) {
        return Ok(Chord::LoneModifierTap(lone_modifier));
    }
    if modifier_named(text).is_some() {
        return Err(format!(
            "'{text}' is a modifier without a key: write MODIFIER - KEY, or tap one of \
             {EVERY_LONE_MODIFIER_NAME_IN_SHORT} on its own"
        ));
    }
    Ok(Chord::KeyWithModifiers {
        modifiers: HotkeyModifiers::empty(),
        key: parse_key(text)?,
    })
}

fn parse_modifiers_joined_by_plus(
    modifier_names: &str,
    chord: &str,
) -> Result<HotkeyModifiers, String> {
    let mut modifiers = HotkeyModifiers::empty();
    for modifier_name in modifier_names.split('+').map(str::trim) {
        if modifier_name.is_empty() {
            return Err(format!(
                "'{chord}' names no modifier before - or between two +"
            ));
        }
        let modifier = modifier_named(modifier_name).ok_or_else(|| {
            format!(
                "'{modifier_name}' is not a modifier; a modifier is alt, shift, cmd, ctrl or fn"
            )
        })?;
        if modifiers.contains(modifier) {
            return Err(format!("'{chord}' names {modifier_name} twice"));
        }
        modifiers |= modifier;
    }
    Ok(modifiers)
}

fn modifier_named(name: &str) -> Option<HotkeyModifiers> {
    MODIFIER_NAMES_IN_CANONICAL_ORDER
        .iter()
        .find(|(modifier_name, _)| *modifier_name == name)
        .map(|(_, modifier)| *modifier)
}

fn parse_key(key_name: &str) -> Result<HotkeyKey, String> {
    if key_name.is_empty() {
        return Err(String::from("a chord needs a key after its modifiers"));
    }
    HotkeyKey::named(key_name)
        .ok_or_else(|| format!("'{key_name}' is not a key; a key is {EVERY_KEY_NAME_IN_SHORT}"))
}

impl fmt::Display for Chord {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Chord::LoneModifierTap(lone_modifier) => formatter.write_str(lone_modifier.name()),
            Chord::KeyWithModifiers { modifiers, key } if modifiers.is_empty() => {
                formatter.write_str(key.name())
            }
            Chord::KeyWithModifiers { modifiers, key } => {
                let modifier_names: Vec<&str> = MODIFIER_NAMES_IN_CANONICAL_ORDER
                    .iter()
                    .filter(|(_, modifier)| modifiers.contains(*modifier))
                    .map(|(modifier_name, _)| *modifier_name)
                    .collect();
                write!(formatter, "{} - {}", modifier_names.join(" + "), key.name())
            }
        }
    }
}

impl TryFrom<String> for Chord {
    type Error = String;

    fn try_from(text: String) -> Result<Chord, String> {
        text.parse()
    }
}

impl From<Chord> for String {
    fn from(chord: Chord) -> String {
        chord.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{Chord, HotkeyModifiers};
    use crate::hotkey::key::HotkeyKey;
    use crate::hotkey::lone_modifier::LoneModifier;

    const KEYCODE_OF_H: u16 = 0x04;
    const KEYCODE_OF_J: u16 = 0x26;
    const KEYCODE_OF_F2: u16 = 0x78;

    fn chord(text: &str) -> Chord {
        text.parse()
            .unwrap_or_else(|refusal| panic!("'{text}' should parse as a chord: {refusal}"))
    }

    fn refusal_of(text: &str) -> String {
        text.parse::<Chord>()
            .expect_err("the chord should be refused")
    }

    #[test]
    fn chord_with_modifiers_in_any_order_and_spacing_prints_back_in_canonical_order() {
        for spelling in [
            "alt + cmd - h",
            "cmd+alt-h",
            "cmd + alt - h",
            "alt+cmd -h",
            "  cmd +alt-  h  ",
        ] {
            assert_eq!(chord(spelling), chord("alt + cmd - h"), "{spelling:?}");
            assert_eq!(chord(spelling).to_string(), "alt + cmd - h", "{spelling:?}");
        }
    }

    #[test]
    fn every_modifier_prints_in_the_order_alt_shift_cmd_ctrl_fn() {
        assert_eq!(
            chord("fn+ctrl+cmd+shift+alt-a").to_string(),
            "alt + shift + cmd + ctrl + fn - a"
        );
    }

    #[test]
    fn a_key_without_modifiers_is_a_chord_that_prints_as_the_bare_key_name() {
        assert_eq!(
            chord("space"),
            Chord::KeyWithModifiers {
                modifiers: HotkeyModifiers::empty(),
                key: HotkeyKey::named("space").unwrap(),
            }
        );
        assert_eq!(chord(" f2 ").to_string(), "f2");
    }

    #[test]
    fn a_side_specific_modifier_on_its_own_is_a_lone_modifier_tap_that_prints_back_alike() {
        assert_eq!(
            chord("rcmd"),
            Chord::LoneModifierTap(LoneModifier::RightCommand)
        );
        for name in [
            "lcmd", "rcmd", "lalt", "ralt", "lctrl", "rctrl", "lshift", "rshift",
        ] {
            assert_eq!(chord(name).to_string(), name);
        }
    }

    #[test]
    fn an_unknown_key_is_refused_naming_it() {
        assert!(refusal_of("alt - hh").starts_with("'hh' is not a key;"));
        assert!(refusal_of("hh").starts_with("'hh' is not a key;"));
    }

    #[test]
    fn a_modifier_named_twice_is_refused() {
        assert_eq!(
            refusal_of("alt + alt - h"),
            "'alt + alt - h' names alt twice"
        );
        assert_eq!(
            refusal_of("cmd+alt+cmd-h"),
            "'cmd+alt+cmd-h' names cmd twice"
        );
    }

    #[test]
    fn a_side_specific_modifier_inside_a_key_chord_is_refused() {
        assert!(refusal_of("rcmd - h").starts_with("'rcmd' is not a modifier;"));
        assert!(refusal_of("alt + lshift - h").starts_with("'lshift' is not a modifier;"));
    }

    #[test]
    fn a_chord_with_nothing_after_its_dash_is_refused_for_lacking_a_key() {
        assert_eq!(
            refusal_of("alt -"),
            "a chord needs a key after its modifiers"
        );
        assert_eq!(
            refusal_of("alt + cmd - "),
            "a chord needs a key after its modifiers"
        );
    }

    #[test]
    fn a_side_agnostic_modifier_on_its_own_is_refused_rather_than_taken_as_a_lone_modifier() {
        for name in ["cmd", "alt", "ctrl", "shift", "fn"] {
            assert!(
                refusal_of(name).starts_with(&format!("'{name}' is a modifier without a key")),
                "{name:?}"
            );
        }
    }

    #[test]
    fn a_plus_or_a_dash_with_no_modifier_beside_it_is_refused() {
        for spelling in ["- h", "alt + - h", "+ alt - h", "alt ++ cmd - h"] {
            assert!(
                refusal_of(spelling).contains("names no modifier"),
                "{spelling:?}"
            );
        }
    }

    #[test]
    fn an_empty_chord_is_refused() {
        assert_eq!(refusal_of(""), "a chord cannot be empty");
        assert_eq!(refusal_of("   "), "a chord cannot be empty");
    }

    #[test]
    fn a_chord_crosses_json_as_its_canonical_spelling_and_is_checked_when_read_back() {
        assert_eq!(
            serde_json::to_string(&chord("cmd+alt-h")).unwrap(),
            "\"alt + cmd - h\""
        );
        assert_eq!(
            serde_json::from_str::<Chord>("\"cmd+alt-h\"").unwrap(),
            chord("alt + cmd - h")
        );
        assert!(serde_json::from_str::<Chord>("\"alt - hh\"").is_err());
    }

    #[test]
    fn a_chord_fires_only_while_exactly_its_modifiers_are_held() {
        let alt_h = chord("alt - h");

        assert!(alt_h.is_pressed_by(KEYCODE_OF_H, HotkeyModifiers::ALT));
        assert!(!alt_h.is_pressed_by(KEYCODE_OF_H, HotkeyModifiers::ALT | HotkeyModifiers::SHIFT));
        assert!(!alt_h.is_pressed_by(KEYCODE_OF_H, HotkeyModifiers::empty()));
        assert!(!alt_h.is_pressed_by(KEYCODE_OF_H, HotkeyModifiers::CMD));
        assert!(!alt_h.is_pressed_by(KEYCODE_OF_J, HotkeyModifiers::ALT));
    }

    #[test]
    fn a_chord_that_does_not_name_fn_fires_whether_or_not_fn_is_held() {
        let alt_f2 = chord("alt - f2");

        assert!(alt_f2.is_pressed_by(KEYCODE_OF_F2, HotkeyModifiers::ALT | HotkeyModifiers::FN));
        assert!(alt_f2.is_pressed_by(KEYCODE_OF_F2, HotkeyModifiers::ALT));
    }

    #[test]
    fn a_chord_that_names_fn_fires_only_while_fn_is_held() {
        let fn_alt_f2 = chord("fn + alt - f2");

        assert!(fn_alt_f2.is_pressed_by(KEYCODE_OF_F2, HotkeyModifiers::ALT | HotkeyModifiers::FN));
        assert!(!fn_alt_f2.is_pressed_by(KEYCODE_OF_F2, HotkeyModifiers::ALT));
        assert!(fn_alt_f2.names_fn());
        assert!(!chord("alt - f2").names_fn());
    }

    #[test]
    fn a_lone_modifier_tap_is_never_pressed_by_a_key_down() {
        const FLAGS_CHANGED_KEYCODE_OF_RIGHT_COMMAND: u16 = 54;
        let right_command_tap = chord("rcmd");

        assert!(
            !right_command_tap
                .is_pressed_by(FLAGS_CHANGED_KEYCODE_OF_RIGHT_COMMAND, HotkeyModifiers::CMD)
        );
        assert!(!right_command_tap.names_fn());
    }
}
