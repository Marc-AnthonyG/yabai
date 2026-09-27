pub(crate) const LAYER_AUTO: i32 = 0;
pub(crate) const LAYER_BELOW: i32 = 3;
pub(crate) const LAYER_NORMAL: i32 = 4;
pub(crate) const LAYER_ABOVE: i32 = 5;

pub static LAYER_STR: [Option<&str>; 6] = [
    Some("auto"),
    None,
    None,
    Some("below"),
    Some("normal"),
    Some("above"),
];
