pub(crate) const MAXLEN: usize = 512;

pub static BOOL_STR: [&str; 2] = ["off", "on"];

pub fn string_equals(first: Option<&str>, second: Option<&str>) -> bool {
    matches!((first, second), (Some(first), Some(second)) if first == second)
}

pub fn string_copy(string: &str) -> String {
    string.to_owned()
}
