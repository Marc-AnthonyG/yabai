#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct HotkeyKey {
    index_in_the_key_table: usize,
}

const KEY_NAMES_AND_THEIR_ANSI_VIRTUAL_KEYCODES: [(&str, u16); 81] = [
    ("a", 0x00),
    ("b", 0x0B),
    ("c", 0x08),
    ("d", 0x02),
    ("e", 0x0E),
    ("f", 0x03),
    ("g", 0x05),
    ("h", 0x04),
    ("i", 0x22),
    ("j", 0x26),
    ("k", 0x28),
    ("l", 0x25),
    ("m", 0x2E),
    ("n", 0x2D),
    ("o", 0x1F),
    ("p", 0x23),
    ("q", 0x0C),
    ("r", 0x0F),
    ("s", 0x01),
    ("t", 0x11),
    ("u", 0x20),
    ("v", 0x09),
    ("w", 0x0D),
    ("x", 0x07),
    ("y", 0x10),
    ("z", 0x06),
    ("0", 0x1D),
    ("1", 0x12),
    ("2", 0x13),
    ("3", 0x14),
    ("4", 0x15),
    ("5", 0x17),
    ("6", 0x16),
    ("7", 0x1A),
    ("8", 0x1C),
    ("9", 0x19),
    ("return", 0x24),
    ("tab", 0x30),
    ("space", 0x31),
    ("escape", 0x35),
    ("backspace", 0x33),
    ("delete", 0x75),
    ("f1", 0x7A),
    ("f2", 0x78),
    ("f3", 0x63),
    ("f4", 0x76),
    ("f5", 0x60),
    ("f6", 0x61),
    ("f7", 0x62),
    ("f8", 0x64),
    ("f9", 0x65),
    ("f10", 0x6D),
    ("f11", 0x67),
    ("f12", 0x6F),
    ("f13", 0x69),
    ("f14", 0x6B),
    ("f15", 0x71),
    ("f16", 0x6A),
    ("f17", 0x40),
    ("f18", 0x4F),
    ("f19", 0x50),
    ("f20", 0x5A),
    ("left", 0x7B),
    ("right", 0x7C),
    ("up", 0x7E),
    ("down", 0x7D),
    ("home", 0x73),
    ("end", 0x77),
    ("pageup", 0x74),
    ("pagedown", 0x79),
    ("minus", 0x1B),
    ("equal", 0x18),
    ("leftbracket", 0x21),
    ("rightbracket", 0x1E),
    ("semicolon", 0x29),
    ("quote", 0x27),
    ("comma", 0x2B),
    ("period", 0x2F),
    ("slash", 0x2C),
    ("backslash", 0x2A),
    ("grave", 0x32),
];

pub(crate) const EVERY_KEY_NAME_IN_SHORT: &str = "a-z, 0-9, return, tab, space, escape, \
     backspace, delete, f1-f20, left, right, up, down, home, end, pageup, pagedown, minus, \
     equal, leftbracket, rightbracket, semicolon, quote, comma, period, slash, backslash or grave";

impl HotkeyKey {
    pub(crate) fn named(name: &str) -> Option<HotkeyKey> {
        KEY_NAMES_AND_THEIR_ANSI_VIRTUAL_KEYCODES
            .iter()
            .position(|(key_name, _)| *key_name == name)
            .map(|index_in_the_key_table| HotkeyKey {
                index_in_the_key_table,
            })
    }

    pub(crate) fn name(self) -> &'static str {
        KEY_NAMES_AND_THEIR_ANSI_VIRTUAL_KEYCODES[self.index_in_the_key_table].0
    }

    pub(crate) fn ansi_virtual_keycode(self) -> u16 {
        KEY_NAMES_AND_THEIR_ANSI_VIRTUAL_KEYCODES[self.index_in_the_key_table].1
    }
}
