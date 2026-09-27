pub fn json_optional_bool(value: i32) -> &'static str {
    if value == 0 {
        return "null";
    }
    if value == 1 {
        return "true";
    }

    "false"
}

pub fn json_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub fn ts_string_escape(string: &str) -> Option<String> {
    let mut number_of_replacements = 0;

    for cursor in string.chars() {
        if (cursor == '"')
            || (cursor == '\\')
            || (cursor == '\u{8}')
            || (cursor == '\u{c}')
            || (cursor == '\n')
            || (cursor == '\r')
            || (cursor == '\t')
        {
            number_of_replacements += 1;
        } else if cursor <= '\u{1f}' {
            number_of_replacements += 5;
        }
    }

    if number_of_replacements == 0 {
        return None;
    }

    let size_in_bytes = string.len() + number_of_replacements;
    let mut destination = String::with_capacity(size_in_bytes);

    for cursor in string.chars() {
        if cursor == '"' {
            destination.push_str("\\\"");
        } else if cursor == '\\' {
            destination.push_str("\\\\");
        } else if cursor == '\u{8}' {
            destination.push_str("\\b");
        } else if cursor == '\u{c}' {
            destination.push_str("\\f");
        } else if cursor == '\n' {
            destination.push_str("\\n");
        } else if cursor == '\r' {
            destination.push_str("\\r");
        } else if cursor == '\t' {
            destination.push_str("\\t");
        } else if cursor <= '\u{1f}' {
            destination.push_str(&format!("\\u{:04x}", cursor as u32));
        } else {
            destination.push(cursor);
        }
    }

    Some(destination)
}

#[cfg(test)]
mod tests {
    use super::{json_bool, json_optional_bool, ts_string_escape};

    #[test]
    fn ts_string_escape_returns_nothing_when_there_is_nothing_to_escape() {
        for unescaped in ["Safari", "", "/", "\u{7f}", "caf\u{e9} \u{2014} \u{1f600}"] {
            assert_eq!(ts_string_escape(unescaped), None, "escaping {unescaped:?}");
        }
    }

    #[test]
    fn ts_string_escape_escapes_quotes_and_backslashes_with_a_backslash() {
        assert_eq!(
            ts_string_escape("say \"hi\""),
            Some("say \\\"hi\\\"".to_string())
        );
        assert_eq!(ts_string_escape("C:\\path"), Some("C:\\\\path".to_string()));
    }

    #[test]
    fn ts_string_escape_uses_the_short_escapes_for_backspace_form_feed_newline_return_and_tab() {
        assert_eq!(
            ts_string_escape("\u{8}\u{c}\n\r\t"),
            Some("\\b\\f\\n\\r\\t".to_string())
        );
    }

    #[test]
    fn ts_string_escape_writes_other_control_characters_as_four_lowercase_hex_digits() {
        let expected_escapes = [
            ("\u{1}", "\\u0001"),
            ("\u{1b}[0m", "\\u001b[0m"),
            ("\u{1f}", "\\u001f"),
            ("a\u{b}b", "a\\u000bb"),
        ];

        for (unescaped, expected_escape) in expected_escapes {
            assert_eq!(
                ts_string_escape(unescaped),
                Some(expected_escape.to_string()),
                "escaping {unescaped:?}"
            );
        }
    }

    #[test]
    fn ts_string_escape_passes_non_ascii_text_through_beside_escaped_characters() {
        assert_eq!(
            ts_string_escape("tab\there \"q\" \\ \u{2} caf\u{e9}"),
            Some("tab\\there \\\"q\\\" \\\\ \\u0002 caf\u{e9}".to_string())
        );
    }

    #[test]
    fn json_optional_bool_reads_zero_as_null_one_as_true_and_anything_else_as_false() {
        assert_eq!(json_optional_bool(0), "null");
        assert_eq!(json_optional_bool(1), "true");
        assert_eq!(json_optional_bool(2), "false");
        assert_eq!(json_optional_bool(-1), "false");
    }

    #[test]
    fn json_bool_spells_true_and_false() {
        assert_eq!(json_bool(true), "true");
        assert_eq!(json_bool(false), "false");
    }
}
