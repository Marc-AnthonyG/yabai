use regex::Regex;

pub fn is_subject_rejected_by_optional_pattern(
    pattern: Option<&Regex>,
    pattern_is_negated: bool,
    subject: &str,
) -> bool {
    pattern.is_some_and(|pattern| pattern.is_match(subject) == pattern_is_negated)
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    use super::is_subject_rejected_by_optional_pattern;

    fn compile_or_panic(pattern: &str) -> Regex {
        Regex::new(pattern).unwrap_or_else(|error| panic!("{pattern:?} should compile: {error}"))
    }

    fn assert_pattern_matches_exactly(pattern: &str, matching: &[&str], not_matching: &[&str]) {
        let regex = compile_or_panic(pattern);
        for subject in matching {
            assert!(
                regex.is_match(subject),
                "{pattern:?} should match {subject:?}"
            );
        }
        for subject in not_matching {
            assert!(
                !regex.is_match(subject),
                "{pattern:?} should not match {subject:?}"
            );
        }
    }

    #[test]
    fn a_missing_pattern_rejects_nothing_even_when_negated() {
        assert!(!is_subject_rejected_by_optional_pattern(
            None, false, "Safari"
        ));
        assert!(!is_subject_rejected_by_optional_pattern(
            None, true, "Safari"
        ));
    }

    #[test]
    fn a_pattern_rejects_the_subjects_it_does_not_match() {
        let regex = compile_or_panic("^Safari$");

        assert!(!is_subject_rejected_by_optional_pattern(
            Some(&regex),
            false,
            "Safari"
        ));
        assert!(is_subject_rejected_by_optional_pattern(
            Some(&regex),
            false,
            "Safari Technology Preview"
        ));
    }

    #[test]
    fn a_negated_pattern_rejects_the_subjects_it_matches() {
        let regex = compile_or_panic("^Safari$");

        assert!(is_subject_rejected_by_optional_pattern(
            Some(&regex),
            true,
            "Safari"
        ));
        assert!(!is_subject_rejected_by_optional_pattern(
            Some(&regex),
            true,
            "Safari Technology Preview"
        ));
    }

    #[test]
    fn the_floating_application_rule_matches_only_the_whole_listed_names() {
        assert_pattern_matches_exactly(
            "^(Harvest|Stickies|Calculator|Software Update|Dictionary|System Preferences|System Settings|zoom.us|App Store|Activity Monitor|Raycast)$",
            &[
                "Harvest",
                "Calculator",
                "Software Update",
                "System Settings",
                "zoom.us",
                "App Store",
                "Activity Monitor",
                "Raycast",
            ],
            &["Calculator Pro", "Harvest Moon", "Settings", "Safari"],
        );
    }

    #[test]
    fn the_finder_rule_matches_only_the_finder() {
        assert_pattern_matches_exactly("^Finder$", &["Finder"], &["Finder Helper", "The Finder"]);
    }

    #[test]
    fn the_finder_dialog_title_rule_matches_copy_connect_move_info_and_preferences() {
        assert_pattern_matches_exactly(
            "(Co(py|nnect)|Move|Info|Pref)",
            &[
                "Copy",
                "Connect to Server",
                "Move",
                "Macintosh HD Info",
                "Finder Preferences",
            ],
            &["Trash", "Downloads", "Co"],
        );
    }

    #[test]
    fn the_about_this_mac_rule_matches_its_title() {
        assert_pattern_matches_exactly(
            "About This Mac",
            &["About This Mac"],
            &["About", "This Mac"],
        );
    }

    #[test]
    fn the_jetbrains_application_rule_matches_every_listed_ide() {
        assert_pattern_matches_exactly(
            "^(WebStorm|PyCharm|IntelliJ.*|GoLand|DataGrip|RustRover)$",
            &[
                "WebStorm",
                "PyCharm",
                "IntelliJ IDEA",
                "IntelliJ IDEA Ultimate",
                "GoLand",
                "DataGrip",
                "RustRover",
            ],
            &["CLion", "PyCharm CE", "Android Studio"],
        );
    }

    #[test]
    fn the_jetbrains_dialog_title_rule_accepts_an_escaped_slash_and_matches_the_dialogs() {
        assert_pattern_matches_exactly(
            r"(Run\/Debug.*|Settings|Rename|Conflicts|Copy|Merge Revisions.*|Delete|Move|Keyboard Shortcut|Notifications|Data Sources and Drivers|Database|Database Query|Database Search|Change Signature.*|File Cache Conflict|Extract .*|Modify)",
            &[
                "Run/Debug Configurations",
                "Settings",
                "Rename",
                "Merge Revisions for yabai",
                "Keyboard Shortcut",
                "Data Sources and Drivers",
                "Change Signature",
                "File Cache Conflict",
                "Extract Method",
                "Modify",
            ],
            &["yabai – main.rs", "Project", "Run"],
        );
    }
}
