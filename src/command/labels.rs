use crate::command::selectors::{DisplaySelector, SpaceSelector};

pub(crate) fn parse_display_label_that_no_display_selector_reads_otherwise(
    text: &str,
) -> Result<String, String> {
    match text.parse() {
        Ok(DisplaySelector::Label(label)) => Ok(label),
        _ => Err(refusal_of_a_label_that_is_read_as_a_selector(
            text, "display",
        )),
    }
}

pub(crate) fn parse_space_label_that_no_space_selector_reads_otherwise(
    text: &str,
) -> Result<String, String> {
    match text.parse() {
        Ok(SpaceSelector::Label(label)) => Ok(label),
        _ => Err(refusal_of_a_label_that_is_read_as_a_selector(text, "space")),
    }
}

fn refusal_of_a_label_that_is_read_as_a_selector(text: &str, kind_of_selector: &str) -> String {
    if text.is_empty() {
        return String::from("a label cannot be empty");
    }
    format!("'{text}' already selects a {kind_of_selector}, so it cannot be a label")
}

#[cfg(test)]
mod tests {
    use super::{
        parse_display_label_that_no_display_selector_reads_otherwise,
        parse_space_label_that_no_space_selector_reads_otherwise,
    };

    #[test]
    fn a_label_is_refused_when_its_selector_would_read_it_as_something_else() {
        for (label, is_a_display_label, is_a_space_label) in [
            ("code", true, true),
            ("1.5", true, true),
            ("0x1f", true, true),
            ("north", false, true),
            ("recent", false, false),
            ("2", false, false),
            ("0", false, false),
            ("", false, false),
        ] {
            assert_eq!(
                parse_display_label_that_no_display_selector_reads_otherwise(label).is_ok(),
                is_a_display_label,
                "{label:?}"
            );
            assert_eq!(
                parse_space_label_that_no_space_selector_reads_otherwise(label).is_ok(),
                is_a_space_label,
                "{label:?}"
            );
        }
    }
}
