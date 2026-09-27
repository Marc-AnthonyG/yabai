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

pub fn clampf_range(value: f32, minimum: f32, maximum: f32) -> f32 {
    if value < minimum {
        return minimum;
    }
    if value > maximum {
        return maximum;
    }
    value
}
