use std::num::NonZeroU32;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub(crate) enum SpaceSelector {
    Previous,
    Next,
    First,
    Last,
    Recent,
    UnderTheMouse,
    MissionControlIndex(NonZeroU32),
    Label(String),
}

impl FromStr for SpaceSelector {
    type Err = String;

    fn from_str(text: &str) -> Result<SpaceSelector, String> {
        Ok(match text {
            "prev" => SpaceSelector::Previous,
            "next" => SpaceSelector::Next,
            "first" => SpaceSelector::First,
            "last" => SpaceSelector::Last,
            "recent" => SpaceSelector::Recent,
            "mouse" => SpaceSelector::UnderTheMouse,
            _ => match parse_index_counting_from_one_or_label(text)? {
                IndexCountingFromOneOrLabel::Index(index) => {
                    SpaceSelector::MissionControlIndex(index)
                }
                IndexCountingFromOneOrLabel::Label(label) => SpaceSelector::Label(label),
            },
        })
    }
}

enum IndexCountingFromOneOrLabel {
    Index(NonZeroU32),
    Label(String),
}

fn parse_index_counting_from_one_or_label(
    text: &str,
) -> Result<IndexCountingFromOneOrLabel, String> {
    if text.is_empty() {
        return Err(String::from("a selector cannot be empty"));
    }
    match text.parse::<u32>() {
        Ok(index) => NonZeroU32::new(index)
            .map(IndexCountingFromOneOrLabel::Index)
            .ok_or_else(|| String::from("indexes count from 1")),
        Err(_) => Ok(IndexCountingFromOneOrLabel::Label(text.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::SpaceSelector;

    #[test]
    fn a_space_selector_is_a_word_an_index_from_one_or_a_label() {
        assert_eq!("prev".parse(), Ok(SpaceSelector::Previous));
        assert_eq!("mouse".parse(), Ok(SpaceSelector::UnderTheMouse));
        assert_eq!(
            "3".parse(),
            Ok(SpaceSelector::MissionControlIndex(
                NonZeroU32::new(3).unwrap()
            ))
        );
        assert_eq!(
            "code".parse(),
            Ok(SpaceSelector::Label(String::from("code")))
        );
        assert_eq!("1.5".parse(), Ok(SpaceSelector::Label(String::from("1.5"))));
        assert_eq!(
            "0x1f".parse(),
            Ok(SpaceSelector::Label(String::from("0x1f")))
        );
    }

    #[test]
    fn a_space_selector_refuses_index_zero_and_nothing() {
        assert!("0".parse::<SpaceSelector>().is_err());
        assert!("".parse::<SpaceSelector>().is_err());
    }
}
