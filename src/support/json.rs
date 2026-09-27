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
