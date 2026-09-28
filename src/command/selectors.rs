use std::num::NonZeroU32;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub(crate) enum CardinalDirection {
    North,
    East,
    South,
    West,
}

impl CardinalDirection {
    fn of_word(word: &str) -> Option<CardinalDirection> {
        match word {
            "north" => Some(CardinalDirection::North),
            "east" => Some(CardinalDirection::East),
            "south" => Some(CardinalDirection::South),
            "west" => Some(CardinalDirection::West),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub(crate) enum DisplaySelector {
    InDirection(CardinalDirection),
    Previous,
    Next,
    First,
    Last,
    Recent,
    UnderTheMouse,
    ArrangementIndex(NonZeroU32),
    Label(String),
}

impl FromStr for DisplaySelector {
    type Err = String;

    fn from_str(text: &str) -> Result<DisplaySelector, String> {
        if let Some(direction) = CardinalDirection::of_word(text) {
            return Ok(DisplaySelector::InDirection(direction));
        }
        Ok(match text {
            "prev" => DisplaySelector::Previous,
            "next" => DisplaySelector::Next,
            "first" => DisplaySelector::First,
            "last" => DisplaySelector::Last,
            "recent" => DisplaySelector::Recent,
            "mouse" => DisplaySelector::UnderTheMouse,
            _ => match parse_index_counting_from_one_or_label(text)? {
                IndexCountingFromOneOrLabel::Index(index) => {
                    DisplaySelector::ArrangementIndex(index)
                }
                IndexCountingFromOneOrLabel::Label(label) => DisplaySelector::Label(label),
            },
        })
    }
}

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

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub(crate) enum WindowSelector {
    InDirection(CardinalDirection),
    Previous,
    Next,
    First,
    Last,
    Recent,
    UnderTheMouse,
    Largest,
    Smallest,
    Sibling,
    FirstNephew,
    SecondNephew,
    Uncle,
    FirstCousin,
    SecondCousin,
    InTheStack(StackPositionSelector),
    Id(u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub(crate) enum StackPositionSelector {
    Previous,
    Next,
    First,
    Last,
    Recent,
    OneBasedPosition(NonZeroU32),
}

const STACK_SELECTOR_PREFIX: &str = "stack.";

impl FromStr for WindowSelector {
    type Err = String;

    fn from_str(text: &str) -> Result<WindowSelector, String> {
        if let Some(direction) = CardinalDirection::of_word(text) {
            return Ok(WindowSelector::InDirection(direction));
        }
        if let Some(stack_position) = text.strip_prefix(STACK_SELECTOR_PREFIX) {
            return stack_position
                .parse()
                .map(WindowSelector::InTheStack)
                .map_err(|_| format!("'{text}' is not a window selector"));
        }
        Ok(match text {
            "prev" => WindowSelector::Previous,
            "next" => WindowSelector::Next,
            "first" => WindowSelector::First,
            "last" => WindowSelector::Last,
            "recent" => WindowSelector::Recent,
            "mouse" => WindowSelector::UnderTheMouse,
            "largest" => WindowSelector::Largest,
            "smallest" => WindowSelector::Smallest,
            "sibling" => WindowSelector::Sibling,
            "first-nephew" => WindowSelector::FirstNephew,
            "second-nephew" => WindowSelector::SecondNephew,
            "uncle" => WindowSelector::Uncle,
            "first-cousin" => WindowSelector::FirstCousin,
            "second-cousin" => WindowSelector::SecondCousin,
            _ => text
                .parse()
                .map(WindowSelector::Id)
                .map_err(|_| format!("'{text}' is not a window selector"))?,
        })
    }
}

impl FromStr for StackPositionSelector {
    type Err = String;

    fn from_str(text: &str) -> Result<StackPositionSelector, String> {
        Ok(match text {
            "prev" => StackPositionSelector::Previous,
            "next" => StackPositionSelector::Next,
            "first" => StackPositionSelector::First,
            "last" => StackPositionSelector::Last,
            "recent" => StackPositionSelector::Recent,
            _ => text
                .parse()
                .map(StackPositionSelector::OneBasedPosition)
                .map_err(|_| format!("'{text}' is not a position in a stack, counting from 1"))?,
        })
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub(crate) enum IndexOrLabelSelector {
    Index(u32),
    Label(String),
}

impl FromStr for IndexOrLabelSelector {
    type Err = String;

    fn from_str(text: &str) -> Result<IndexOrLabelSelector, String> {
        if text.is_empty() {
            return Err(String::from("a selector cannot be empty"));
        }
        Ok(match text.parse() {
            Ok(index) => IndexOrLabelSelector::Index(index),
            Err(_) => IndexOrLabelSelector::Label(text.to_owned()),
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

    use super::{
        CardinalDirection, DisplaySelector, IndexOrLabelSelector, SpaceSelector,
        StackPositionSelector, WindowSelector,
    };

    fn index(index: u32) -> NonZeroU32 {
        NonZeroU32::new(index).unwrap()
    }

    #[test]
    fn a_display_selector_is_a_direction_a_word_an_index_from_one_or_a_label() {
        assert_eq!(
            "west".parse(),
            Ok(DisplaySelector::InDirection(CardinalDirection::West))
        );
        assert_eq!("recent".parse(), Ok(DisplaySelector::Recent));
        assert_eq!("2".parse(), Ok(DisplaySelector::ArrangementIndex(index(2))));
        assert_eq!(
            "external".parse(),
            Ok(DisplaySelector::Label(String::from("external")))
        );
        assert!("0".parse::<DisplaySelector>().is_err());
        assert!("".parse::<DisplaySelector>().is_err());
    }

    #[test]
    fn a_space_selector_is_a_word_an_index_from_one_or_a_label() {
        assert_eq!("prev".parse(), Ok(SpaceSelector::Previous));
        assert_eq!("mouse".parse(), Ok(SpaceSelector::UnderTheMouse));
        assert_eq!(
            "3".parse(),
            Ok(SpaceSelector::MissionControlIndex(index(3)))
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

    #[test]
    fn a_space_selector_takes_no_direction() {
        assert_eq!(
            "north".parse(),
            Ok(SpaceSelector::Label(String::from("north")))
        );
    }

    #[test]
    fn a_window_selector_is_a_direction_a_word_a_tree_relative_or_an_id() {
        assert_eq!(
            "north".parse(),
            Ok(WindowSelector::InDirection(CardinalDirection::North))
        );
        assert_eq!("largest".parse(), Ok(WindowSelector::Largest));
        assert_eq!("first-nephew".parse(), Ok(WindowSelector::FirstNephew));
        assert_eq!("second-cousin".parse(), Ok(WindowSelector::SecondCousin));
        assert_eq!("12345".parse(), Ok(WindowSelector::Id(12345)));
    }

    #[test]
    fn a_window_selector_can_pick_a_window_of_the_stack() {
        assert_eq!(
            "stack.next".parse(),
            Ok(WindowSelector::InTheStack(StackPositionSelector::Next))
        );
        assert_eq!(
            "stack.3".parse(),
            Ok(WindowSelector::InTheStack(
                StackPositionSelector::OneBasedPosition(index(3))
            ))
        );
    }

    #[test]
    fn a_window_selector_refuses_labels_old_spellings_and_stack_position_zero() {
        for refused in [
            "stack.0",
            "stack.",
            "stack.top",
            "first_nephew",
            "code",
            "-1",
            "1.5",
            "",
        ] {
            assert!(refused.parse::<WindowSelector>().is_err(), "{refused:?}");
        }
    }

    #[test]
    fn a_rule_or_signal_selector_is_an_index_from_zero_or_a_label() {
        assert_eq!("0".parse(), Ok(IndexOrLabelSelector::Index(0)));
        assert_eq!(
            "screen_padding".parse(),
            Ok(IndexOrLabelSelector::Label(String::from("screen_padding")))
        );
        assert!("".parse::<IndexOrLabelSelector>().is_err());
    }
}
