pub(crate) fn max<T: PartialOrd>(first: T, second: T) -> T {
    if first > second { first } else { second }
}

pub(crate) fn add_and_clamp_to_zero(value: i32, delta: i32) -> i32 {
    if value + delta <= 0 { 0 } else { value + delta }
}

pub(crate) fn in_range_ii<T: PartialOrd>(value: T, low: T, high: T) -> bool {
    value >= low && value <= high
}

pub(crate) fn in_range_ie<T: PartialOrd>(value: T, low: T, high: T) -> bool {
    value >= low && value < high
}

pub(crate) fn in_range_ei<T: PartialOrd>(value: T, low: T, high: T) -> bool {
    value > low && value <= high
}

pub(crate) fn lerp(start: f64, interpolant: f32, end: f32) -> f64 {
    ((1.0f64 - interpolant as f64) * start) + (interpolant * end) as f64
}

pub(crate) const FAILURE_MESSAGE: &[u8] = b"\x07";

pub(crate) const MAXLEN: usize = 512;

pub(crate) const DIR_NORTH: i32 = 360;
pub(crate) const DIR_EAST: i32 = 90;
pub(crate) const DIR_SOUTH: i32 = 180;
pub(crate) const DIR_WEST: i32 = 270;

pub(crate) const STACK: i32 = 111;

pub(crate) const TYPE_ABS: i32 = 0x1;
pub(crate) const TYPE_REL: i32 = 0x2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResizeHandle(pub u8);

impl ResizeHandle {
    pub(crate) const TOP: ResizeHandle = ResizeHandle(0x01);
    pub(crate) const BOTTOM: ResizeHandle = ResizeHandle(0x02);
    pub(crate) const LEFT: ResizeHandle = ResizeHandle(0x04);
    pub(crate) const RIGHT: ResizeHandle = ResizeHandle(0x08);
    pub(crate) const ABS: ResizeHandle = ResizeHandle(0x10);
}

pub(crate) const LAYER_AUTO: i32 = 0;
pub(crate) const LAYER_BELOW: i32 = 3;
pub(crate) const LAYER_NORMAL: i32 = 4;
pub(crate) const LAYER_ABOVE: i32 = 5;
