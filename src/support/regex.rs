const _: () = assert!(std::mem::size_of::<libc::regex_t>() == 32);

pub struct PosixRegex {
    regex: Box<libc::regex_t>,
}

impl PosixRegex {
    pub fn compile(pattern: &std::ffi::CStr) -> Option<PosixRegex> {
        let mut regex: Box<libc::regex_t> = Box::new(unsafe { std::mem::zeroed() });
        let status = unsafe { libc::regcomp(regex.as_mut(), pattern.as_ptr(), libc::REG_EXTENDED) };
        if status == 0 {
            Some(PosixRegex { regex })
        } else {
            None
        }
    }

    pub fn matches(&self, subject: &std::ffi::CStr) -> bool {
        let status = unsafe {
            libc::regexec(self.regex.as_ref(), subject.as_ptr(), 0, std::ptr::null_mut(), 0)
        };
        status == 0
    }
}

impl Drop for PosixRegex {
    fn drop(&mut self) {
        unsafe { libc::regfree(self.regex.as_mut()) };
    }
}

#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RegexMatch {
    Undefined = 0,
    Yes = 1,
    No = 2,
}

pub fn match_subject_against_optional_regex(
    regex: Option<&PosixRegex>,
    subject: &std::ffi::CStr,
) -> RegexMatch {
    match regex {
        None => RegexMatch::Undefined,
        Some(regex) => {
            if regex.matches(subject) {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::{PosixRegex, RegexMatch, match_subject_against_optional_regex};

    fn compile_or_panic(pattern: &CStr) -> PosixRegex {
        PosixRegex::compile(pattern)
            .unwrap_or_else(|| panic!("{pattern:?} should compile as an extended regex"))
    }

    fn pattern_matches_subject(pattern: &CStr, subject: &CStr) -> bool {
        compile_or_panic(pattern).matches(subject)
    }

    #[test]
    fn match_subject_against_optional_regex_without_a_regex_is_undefined() {
        assert!(match_subject_against_optional_regex(None, c"Safari") == RegexMatch::Undefined);
    }

    #[test]
    fn match_subject_against_optional_regex_is_yes_when_the_subject_matches() {
        let regex = compile_or_panic(c"^Safari$");

        assert!(match_subject_against_optional_regex(Some(&regex), c"Safari") == RegexMatch::Yes);
    }

    #[test]
    fn match_subject_against_optional_regex_is_no_when_the_subject_does_not_match() {
        let regex = compile_or_panic(c"^Safari$");

        assert!(
            match_subject_against_optional_regex(Some(&regex), c"Safari Technology Preview")
                == RegexMatch::No
        );
    }

    #[test]
    fn match_subject_against_optional_regex_results_keep_the_c_values() {
        assert_eq!(RegexMatch::Undefined as i32, 0);
        assert_eq!(RegexMatch::Yes as i32, 1);
        assert_eq!(RegexMatch::No as i32, 2);
    }

    #[test]
    fn an_unanchored_pattern_matches_anywhere_in_the_subject() {
        assert!(pattern_matches_subject(
            c"Safari",
            c"Safari Technology Preview"
        ));
        assert!(pattern_matches_subject(
            c"Preview",
            c"Safari Technology Preview"
        ));
    }

    #[test]
    fn matching_is_case_sensitive() {
        assert!(!pattern_matches_subject(c"safari", c"Safari"));
    }

    #[test]
    fn backslash_d_is_a_literal_d_and_not_a_digit_class_as_with_regcomp() {
        assert!(!pattern_matches_subject(c"\\d", c"5"));
        assert!(pattern_matches_subject(c"\\d", c"d"));
    }

    #[test]
    fn backslash_w_is_a_literal_w_and_not_a_word_class_as_with_regcomp() {
        assert!(pattern_matches_subject(c"\\w+", c"w"));
        assert!(!pattern_matches_subject(c"\\w+", c"abc"));
    }

    #[test]
    fn posix_bracket_classes_and_ranges_match_digits() {
        assert!(pattern_matches_subject(c"[[:digit:]]+", c"abc123"));
        assert!(!pattern_matches_subject(c"[0-9]+", c"abc"));
    }

    #[test]
    fn extended_syntax_supports_alternation_groups_and_intervals_without_escaping() {
        assert!(pattern_matches_subject(c"^(Finder|Safari)$", c"Finder"));
        assert!(!pattern_matches_subject(c"^(Finder|Safari)$", c"Terminal"));
        assert!(pattern_matches_subject(
            c"System Settings|System Preferences",
            c"System Preferences"
        ));
        assert!(pattern_matches_subject(c"a{2}", c"caab"));
        assert!(!pattern_matches_subject(c"a{2}", c"cab"));
    }

    #[test]
    fn an_escaped_dot_matches_only_a_dot() {
        assert!(pattern_matches_subject(c"\\.", c"a.b"));
        assert!(!pattern_matches_subject(c"\\.", c"ab"));
    }

    #[test]
    fn an_anchored_empty_pattern_matches_an_empty_subject() {
        assert!(pattern_matches_subject(c"^$", c""));
    }

    #[test]
    fn compile_rejects_the_patterns_regcomp_rejects() {
        for pattern in [c"(", c"[", c"a**", c"a+?", c"(?i)safari", c"a|*b", c""] {
            assert!(
                PosixRegex::compile(pattern).is_none(),
                "{pattern:?} should not compile as an extended regex"
            );
        }
    }
}
