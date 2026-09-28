use clap::{Args, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::command::labels::parse_space_label_that_no_space_selector_reads_otherwise;
use crate::command::selectors::{DisplaySelector, SpaceSelector};
use crate::command::values::AbsoluteOrRelativeChange;
use crate::layout::settings::ViewLayout;
use crate::layout::tree::WindowNodeSplit;

#[derive(Args, Serialize, Deserialize, Debug)]
pub(crate) struct SpaceCommand {
    /// Act on this space instead of the focused one
    #[arg(short, long, global = true, value_name = "SPACE_SEL")]
    pub(crate) space: Option<SpaceSelector>,
    #[command(subcommand)]
    pub(crate) action: SpaceAction,
}

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum SpaceAction {
    /// Focus a space where it is; relative selectors start from the acting space
    Focus {
        #[arg(value_name = "SPACE_SEL")]
        target: SpaceSelector,
    },
    /// Bring a space to the focused display and focus it; relative selectors start from the
    /// acting space
    Switch {
        #[arg(value_name = "SPACE_SEL")]
        target: SpaceSelector,
    },
    /// Create a space on a display [default: the display of the acting space]
    Create {
        #[arg(value_name = "DISPLAY_SEL")]
        display: Option<DisplaySelector>,
    },
    /// Destroy the acting space
    Destroy,
    /// Move the acting space to the position of another space of its display
    Move {
        #[arg(value_name = "SPACE_SEL")]
        target: SpaceSelector,
    },
    /// Swap the acting space with another space
    Swap {
        #[arg(value_name = "SPACE_SEL")]
        target: SpaceSelector,
    },
    /// Send the acting space to another display
    SendToDisplay {
        #[arg(value_name = "DISPLAY_SEL")]
        display: DisplaySelector,
    },
    /// Reset every split of the acting space to the global split ratio [default: along both
    /// axes]
    Equalize { axis: Option<TreeAxis> },
    /// Give every window of the acting space an equal share [default: along both axes]
    Balance { axis: Option<TreeAxis> },
    /// Flip the tree of the acting space along an axis
    Mirror { axis: TreeAxis },
    /// Rotate the tree of the acting space clockwise
    Rotate { degrees: TreeRotationInDegrees },
    /// Set or change the padding of the acting space, in points
    #[command(allow_negative_numbers = true)]
    Padding {
        change: AbsoluteOrRelativeChange,
        top: i32,
        bottom: i32,
        left: i32,
        right: i32,
    },
    /// Set or change the gap between the windows of the acting space, in points
    #[command(allow_negative_numbers = true)]
    Gap {
        change: AbsoluteOrRelativeChange,
        gap: i32,
    },
    /// Toggle the padding or the gap of the acting space, or focus it and toggle mission control
    /// or show desktop
    Toggle { setting: SpaceToggleableSetting },
    /// Re-tile the acting space; unlike `config set --space`, a later global layout replaces it
    Layout { layout: ViewLayout },
    /// Label the acting space; without LABEL, remove its label
    Label {
        /// Any text a space selector would not read as something else
        #[arg(value_parser = parse_space_label_that_no_space_selector_reads_otherwise)]
        label: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
pub(crate) enum TreeAxis {
    XAxis,
    YAxis,
}

impl TreeAxis {
    pub(crate) fn split_of_the_tree_along_this_axis(self) -> WindowNodeSplit {
        match self {
            TreeAxis::XAxis => WindowNodeSplit::Horizontal,
            TreeAxis::YAxis => WindowNodeSplit::Vertical,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
pub(crate) enum TreeRotationInDegrees {
    #[value(name = "90")]
    Quarter,
    #[value(name = "180")]
    Half,
    #[value(name = "270")]
    ThreeQuarters,
}

impl TreeRotationInDegrees {
    pub(crate) fn degrees(self) -> i32 {
        match self {
            TreeRotationInDegrees::Quarter => 90,
            TreeRotationInDegrees::Half => 180,
            TreeRotationInDegrees::ThreeQuarters => 270,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
pub(crate) enum SpaceToggleableSetting {
    Padding,
    Gap,
    MissionControl,
    ShowDesktop,
}

#[cfg(test)]
mod tests {
    use super::{
        SpaceAction, SpaceCommand, SpaceToggleableSetting, TreeAxis, TreeRotationInDegrees,
    };
    use crate::command::selectors::SpaceSelector;
    use crate::command::values::AbsoluteOrRelativeChange;
    use crate::command::{DaemonCommand, parse_daemon_command};
    use crate::layout::settings::ViewLayout;

    fn parse_space_command(arguments: &[&str]) -> Result<SpaceCommand, clap::Error> {
        match parse_daemon_command(&[&["space"], arguments].concat())? {
            DaemonCommand::Space(space_command) => Ok(space_command),
            _ => panic!("{arguments:?} should parse as a space command"),
        }
    }

    fn parse_space_action(arguments: &[&str]) -> SpaceAction {
        parse_space_command(arguments).unwrap().action
    }

    #[test]
    fn the_acting_space_may_come_before_or_after_the_action() {
        for arguments in [
            ["-s", "code", "layout", "bsp"],
            ["layout", "bsp", "--space", "code"],
        ] {
            let space_command = parse_space_command(&arguments).unwrap();

            assert_eq!(
                space_command.space,
                Some(SpaceSelector::Label(String::from("code")))
            );
            assert!(matches!(
                space_command.action,
                SpaceAction::Layout {
                    layout: ViewLayout::BinarySpacePartitioning
                }
            ));
        }
    }

    #[test]
    fn space_actions_take_their_values_as_words_and_negative_numbers() {
        assert!(matches!(
            parse_space_action(&["padding", "by", "-10", "0", "5", "0"]),
            SpaceAction::Padding {
                change: AbsoluteOrRelativeChange::By,
                top: -10,
                bottom: 0,
                left: 5,
                right: 0
            }
        ));
        assert!(matches!(
            parse_space_action(&["gap", "to", "8"]),
            SpaceAction::Gap {
                change: AbsoluteOrRelativeChange::To,
                gap: 8
            }
        ));
        assert!(matches!(
            parse_space_action(&["balance"]),
            SpaceAction::Balance { axis: None }
        ));
        assert!(matches!(
            parse_space_action(&["mirror", "y-axis"]),
            SpaceAction::Mirror {
                axis: TreeAxis::YAxis
            }
        ));
        assert!(matches!(
            parse_space_action(&["rotate", "270"]),
            SpaceAction::Rotate {
                degrees: TreeRotationInDegrees::ThreeQuarters
            }
        ));
        assert!(matches!(
            parse_space_action(&["toggle", "show-desktop"]),
            SpaceAction::Toggle {
                setting: SpaceToggleableSetting::ShowDesktop
            }
        ));
        assert!(matches!(
            parse_space_action(&["label"]),
            SpaceAction::Label { label: None }
        ));
    }

    #[test]
    fn space_commands_refuse_old_spellings_missing_values_and_selector_words_as_labels() {
        for refused in [
            &["focus"][..],
            &["--focus", "2"],
            &["destroy", "3"],
            &["mirror"],
            &["rotate", "45"],
            &["padding", "abs:1:2:3:4"],
            &["gap", "rel:5"],
            &["layout", "default"],
            &["label", "next"],
            &["label", "3"],
            &["-s", "0", "destroy"],
        ] {
            assert!(parse_space_command(refused).is_err(), "{refused:?}");
        }
    }
}
