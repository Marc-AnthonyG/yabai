pub(crate) const FIXED_STRING_BUFFER_LENGTH: usize = 512;

pub static BOOLEAN_NAMES: [&str; 2] = ["off", "on"];

pub fn are_both_strings_present_and_equal(first: Option<&str>, second: Option<&str>) -> bool {
    matches!((first, second), (Some(first), Some(second)) if first == second)
}
