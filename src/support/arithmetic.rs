pub(crate) fn is_within_range_including_both_bounds<T: PartialOrd>(
    value: T,
    low: T,
    high: T,
) -> bool {
    value >= low && value <= high
}
