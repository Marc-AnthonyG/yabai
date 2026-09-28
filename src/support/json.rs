pub fn json_literal_for_optional_boolean(value: i32) -> &'static str {
    if value == 0 {
        return "null";
    }
    if value == 1 {
        return "true";
    }

    "false"
}

pub fn json_literal_for_boolean(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub fn escape_string_for_json_when_it_needs_escaping(string: &str) -> Option<String> {
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
    use super::{
        escape_string_for_json_when_it_needs_escaping, json_literal_for_boolean,
        json_literal_for_optional_boolean,
    };

    #[test]
    fn escape_string_for_json_when_it_needs_escaping_returns_nothing_when_there_is_nothing_to_escape()
     {
        for unescaped in ["Safari", "", "/", "\u{7f}", "caf\u{e9} \u{2014} \u{1f600}"] {
            assert_eq!(
                escape_string_for_json_when_it_needs_escaping(unescaped),
                None,
                "escaping {unescaped:?}"
            );
        }
    }

    #[test]
    fn escape_string_for_json_when_it_needs_escaping_escapes_quotes_and_backslashes_with_a_backslash()
     {
        assert_eq!(
            escape_string_for_json_when_it_needs_escaping("say \"hi\""),
            Some("say \\\"hi\\\"".to_string())
        );
        assert_eq!(
            escape_string_for_json_when_it_needs_escaping("C:\\path"),
            Some("C:\\\\path".to_string())
        );
    }

    #[test]
    fn escape_string_for_json_when_it_needs_escaping_uses_the_short_escapes_for_backspace_form_feed_newline_return_and_tab()
     {
        assert_eq!(
            escape_string_for_json_when_it_needs_escaping("\u{8}\u{c}\n\r\t"),
            Some("\\b\\f\\n\\r\\t".to_string())
        );
    }

    #[test]
    fn escape_string_for_json_when_it_needs_escaping_writes_other_control_characters_as_four_lowercase_hex_digits()
     {
        let expected_escapes = [
            ("\u{1}", "\\u0001"),
            ("\u{1b}[0m", "\\u001b[0m"),
            ("\u{1f}", "\\u001f"),
            ("a\u{b}b", "a\\u000bb"),
        ];

        for (unescaped, expected_escape) in expected_escapes {
            assert_eq!(
                escape_string_for_json_when_it_needs_escaping(unescaped),
                Some(expected_escape.to_string()),
                "escaping {unescaped:?}"
            );
        }
    }

    #[test]
    fn escape_string_for_json_when_it_needs_escaping_passes_non_ascii_text_through_beside_escaped_characters()
     {
        assert_eq!(
            escape_string_for_json_when_it_needs_escaping("tab\there \"q\" \\ \u{2} caf\u{e9}"),
            Some("tab\\there \\\"q\\\" \\\\ \\u0002 caf\u{e9}".to_string())
        );
    }

    #[test]
    fn json_literal_for_optional_boolean_reads_zero_as_null_one_as_true_and_anything_else_as_false()
    {
        assert_eq!(json_literal_for_optional_boolean(0), "null");
        assert_eq!(json_literal_for_optional_boolean(1), "true");
        assert_eq!(json_literal_for_optional_boolean(2), "false");
        assert_eq!(json_literal_for_optional_boolean(-1), "false");
    }

    #[test]
    fn json_literal_for_boolean_spells_true_and_false() {
        assert_eq!(json_literal_for_boolean(true), "true");
        assert_eq!(json_literal_for_boolean(false), "false");
    }
}
