pub(crate) fn greater_of_two_values<T: PartialOrd>(first: T, second: T) -> T {
    if first > second { first } else { second }
}

pub(crate) fn add_and_clamp_to_zero(value: i32, delta: i32) -> i32 {
    if value + delta <= 0 { 0 } else { value + delta }
}

pub(crate) fn is_within_range_including_both_bounds<T: PartialOrd>(
    value: T,
    low: T,
    high: T,
) -> bool {
    value >= low && value <= high
}

pub(crate) fn is_within_range_including_low_excluding_high<T: PartialOrd>(
    value: T,
    low: T,
    high: T,
) -> bool {
    value >= low && value < high
}

pub(crate) fn is_within_range_excluding_low_including_high<T: PartialOrd>(
    value: T,
    low: T,
    high: T,
) -> bool {
    value > low && value <= high
}

pub fn clamp_float_to_range(value: f32, minimum: f32, maximum: f32) -> f32 {
    if value < minimum {
        return minimum;
    }
    if value > maximum {
        return maximum;
    }
    value
}
