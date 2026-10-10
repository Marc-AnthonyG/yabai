use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::hotkey::key::{EVERY_KEY_NAME_IN_SHORT, HotkeyKey};

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
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
}

impl Chord {
    pub(crate) fn is_pressed_by(
        self,
        ansi_virtual_keycode: u16,
        modifiers_held: HotkeyModifiers,
    ) -> bool {
        let Chord::KeyWithModifiers { modifiers, key } = self;
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
    if modifier_named(text).is_some() {
        return Err(format!(
            "'{text}' is a modifier without a key: write MODIFIER - KEY"
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
