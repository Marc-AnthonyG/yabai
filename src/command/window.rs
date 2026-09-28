use clap::{Args, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::command::selectors::{DisplaySelector, SpaceSelector, WindowSelector};
use crate::command::values::{
    AbsoluteOrRelativeChange, GridPlacement, parse_finite_number, parse_opacity_from_zero_to_one,
};
use crate::support::direction::{
    DIRECTION_EAST, DIRECTION_NORTH, DIRECTION_SOUTH, DIRECTION_STACK_INSTEAD_OF_SPLIT,
    DIRECTION_WEST,
};
use crate::support::layer::WindowStackingSubLayer;
use crate::support::resize_handle::ResizeHandle;

#[derive(Args, Serialize, Deserialize, Debug)]
pub(crate) struct WindowCommand {
    /// Act on this window instead of the focused one
    #[arg(short, long, global = true, value_name = "WINDOW_SEL")]
    pub(crate) window: Option<WindowSelector>,
    #[command(subcommand)]
    pub(crate) action: WindowAction,
}

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum WindowAction {
    /// Focus a window [default: the acting window]
    Focus {
        #[arg(value_name = "WINDOW_SEL")]
        target: Option<WindowSelector>,
    },
    /// Close a window [default: the acting window]
    Close {
        #[arg(value_name = "WINDOW_SEL")]
        target: Option<WindowSelector>,
    },
    /// Minimize a window [default: the acting window]
    Minimize {
        #[arg(value_name = "WINDOW_SEL")]
        target: Option<WindowSelector>,
    },
    /// Deminimize a window
    Deminimize {
        #[arg(value_name = "WINDOW_SEL")]
        target: WindowSelector,
    },
    /// Send the acting window to the visible space of a display
    SendToDisplay {
        #[arg(value_name = "DISPLAY_SEL")]
        display: DisplaySelector,
    },
    /// Send the acting window to a space
    SendToSpace {
        #[arg(value_name = "SPACE_SEL")]
        space: SpaceSelector,
    },
    /// Swap the acting window with another tiled window
    Swap {
        #[arg(value_name = "WINDOW_SEL")]
        target: WindowSelector,
    },
    /// Split the node of another tiled window and put the acting window in it
    Warp {
        #[arg(value_name = "WINDOW_SEL")]
        target: WindowSelector,
    },
    /// Stack another window onto the node of the acting window
    Stack {
        #[arg(value_name = "WINDOW_SEL")]
        target: WindowSelector,
    },
    /// Mark where the next window splits the node of the acting window; the same direction again
    /// clears the mark
    Insert { direction: InsertionDirection },
    /// Place the floating acting window on a grid of its display
    Grid {
        #[arg(value_name = "ROWS:COLUMNS:X:Y:WIDTH:HEIGHT")]
        placement: GridPlacement,
    },
    /// Move the floating acting window to a position, or by an offset, in points
    #[command(allow_negative_numbers = true)]
    Move {
        change: AbsoluteOrRelativeChange,
        #[arg(value_parser = parse_finite_number)]
        x: f32,
        #[arg(value_parser = parse_finite_number)]
        y: f32,
    },
    /// Drag an edge or a corner of the acting window by an offset, or give it a size, in points
    #[command(allow_negative_numbers = true)]
    Resize {
        handle: ResizeHandleOrAbsoluteSize,
        #[arg(value_parser = parse_finite_number)]
        width: f32,
        #[arg(value_parser = parse_finite_number)]
        height: f32,
    },
    /// Set or change the split ratio of the node holding the acting window
    #[command(allow_negative_numbers = true)]
    Ratio {
        change: AbsoluteOrRelativeChange,
        #[arg(value_parser = parse_finite_number)]
        ratio: f32,
    },
    /// Toggle a property of the acting window
    Toggle { property: WindowToggleableProperty },
    /// Stack the acting window below, with or above normal windows
    SubLayer { layer: WindowStackingSubLayer },
    /// Set the opacity of the acting window; 0 gives it back the active and normal opacity
    Opacity {
        #[arg(value_parser = parse_opacity_from_zero_to_one)]
        opacity: f32,
    },
    /// Raise the acting window above another window [default: above every window]
    Raise {
        #[arg(value_name = "WINDOW_SEL")]
        above: Option<WindowSelector>,
    },
    /// Lower the acting window below another window [default: below every window]
    Lower {
        #[arg(value_name = "WINDOW_SEL")]
        below: Option<WindowSelector>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
pub(crate) enum InsertionDirection {
    North,
    East,
    South,
    West,
    /// Stack the next window onto the acting window instead of splitting
    Stack,
}

impl InsertionDirection {
    pub(crate) fn insert_direction_of_a_tree_node(self) -> i32 {
        match self {
            InsertionDirection::North => DIRECTION_NORTH,
            InsertionDirection::East => DIRECTION_EAST,
            InsertionDirection::South => DIRECTION_SOUTH,
            InsertionDirection::West => DIRECTION_WEST,
            InsertionDirection::Stack => DIRECTION_STACK_INSTEAD_OF_SPLIT,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
pub(crate) enum ResizeHandleOrAbsoluteSize {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    /// Give the window this width and height
    #[value(name = "to")]
    ToAbsoluteSize,
}

impl ResizeHandleOrAbsoluteSize {
    pub(crate) fn resize_handle(self) -> ResizeHandle {
        match self {
            ResizeHandleOrAbsoluteSize::Top => ResizeHandle::TOP,
            ResizeHandleOrAbsoluteSize::Bottom => ResizeHandle::BOTTOM,
            ResizeHandleOrAbsoluteSize::Left => ResizeHandle::LEFT,
            ResizeHandleOrAbsoluteSize::Right => ResizeHandle::RIGHT,
            ResizeHandleOrAbsoluteSize::TopLeft => {
                ResizeHandle(ResizeHandle::TOP.0 | ResizeHandle::LEFT.0)
            }
            ResizeHandleOrAbsoluteSize::TopRight => {
                ResizeHandle(ResizeHandle::TOP.0 | ResizeHandle::RIGHT.0)
            }
            ResizeHandleOrAbsoluteSize::BottomLeft => {
                ResizeHandle(ResizeHandle::BOTTOM.0 | ResizeHandle::LEFT.0)
            }
            ResizeHandleOrAbsoluteSize::BottomRight => {
                ResizeHandle(ResizeHandle::BOTTOM.0 | ResizeHandle::RIGHT.0)
            }
            ResizeHandleOrAbsoluteSize::ToAbsoluteSize => ResizeHandle::ABSOLUTE,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
pub(crate) enum WindowToggleableProperty {
    /// Float the window or tile it again
    Float,
    /// Show the window on every space
    Sticky,
    /// Picture in picture (needs the scripting addition)
    Pip,
    /// The shadow of the window (needs the scripting addition)
    Shadow,
    /// The split direction of the node holding the window
    Split,
    /// Let the window fill the node of its parent
    ZoomParent,
    /// Let the window fill the whole tree of its space
    ZoomFullscreen,
    /// Let the window fill its display without a native fullscreen space
    WindowedFullscreen,
    /// Move the window to its own native fullscreen space
    NativeFullscreen,
    /// Show every window of the application of the window
    Expose,
    /// Show the windows of the node holding the window as a group under a tabbed header, or give
    /// each its own tile again
    Group,
}

#[cfg(test)]
mod tests {
    use super::{
        InsertionDirection, ResizeHandleOrAbsoluteSize, WindowAction, WindowCommand,
        WindowToggleableProperty,
    };
    use crate::command::selectors::{
        CardinalDirection, SpaceSelector, StackPositionSelector, WindowSelector,
    };
    use crate::command::values::AbsoluteOrRelativeChange;
    use crate::command::{DaemonCommand, parse_daemon_command};

    fn parse_window_command(arguments: &[&str]) -> Result<WindowCommand, clap::Error> {
        match parse_daemon_command(&[&["window"], arguments].concat())? {
            DaemonCommand::Window(window_command) => Ok(window_command),
            _ => panic!("{arguments:?} should parse as a window command"),
        }
    }

    fn parse_window_action(arguments: &[&str]) -> WindowAction {
        parse_window_command(arguments).unwrap().action
    }

    #[test]
    fn the_acting_window_may_come_before_or_after_the_action() {
        for arguments in [
            ["-w", "123", "swap", "west"],
            ["swap", "west", "--window", "123"],
        ] {
            let window_command = parse_window_command(&arguments).unwrap();

            assert_eq!(window_command.window, Some(WindowSelector::Id(123)));
            assert!(matches!(
                window_command.action,
                WindowAction::Swap {
                    target: WindowSelector::InDirection(CardinalDirection::West)
                }
            ));
        }
    }

    #[test]
    fn window_actions_take_their_values_as_words_and_negative_numbers() {
        assert!(matches!(
            parse_window_action(&["focus", "stack.next"]),
            WindowAction::Focus {
                target: Some(WindowSelector::InTheStack(StackPositionSelector::Next))
            }
        ));
        assert!(matches!(
            parse_window_action(&["close"]),
            WindowAction::Close { target: None }
        ));
        assert!(matches!(
            parse_window_action(&["send-to-space", "3"]),
            WindowAction::SendToSpace {
                space: SpaceSelector::MissionControlIndex(_)
            }
        ));
        assert!(matches!(
            parse_window_action(&["move", "by", "-20", "30.5"]),
            WindowAction::Move {
                change: AbsoluteOrRelativeChange::By,
                x: -20.0,
                y: 30.5
            }
        ));
        assert!(matches!(
            parse_window_action(&["resize", "bottom-right", "-10", "0"]),
            WindowAction::Resize {
                handle: ResizeHandleOrAbsoluteSize::BottomRight,
                ..
            }
        ));
        assert!(matches!(
            parse_window_action(&["resize", "to", "800", "600"]),
            WindowAction::Resize {
                handle: ResizeHandleOrAbsoluteSize::ToAbsoluteSize,
                ..
            }
        ));
        assert!(matches!(
            parse_window_action(&["insert", "stack"]),
            WindowAction::Insert {
                direction: InsertionDirection::Stack
            }
        ));
        assert!(matches!(
            parse_window_action(&["toggle", "native-fullscreen"]),
            WindowAction::Toggle {
                property: WindowToggleableProperty::NativeFullscreen
            }
        ));
        assert!(matches!(
            parse_window_action(&["opacity", "1"]),
            WindowAction::Opacity { opacity: 1.0 }
        ));
    }

    #[test]
    fn window_commands_refuse_old_spellings_missing_targets_and_values_out_of_range() {
        for refused in [
            &["--focus", "west"][..],
            &["focus", "stack.0"],
            &["focus", "first_nephew"],
            &["swap"],
            &["deminimize"],
            &["insert", "up"],
            &["resize", "top_left", "1", "1"],
            &["move", "abs:1:2"],
            &["move", "to", "nan", "0"],
            &["opacity", "1.5"],
            &["grid", "0:4:0:0:1:1"],
            &["toggle", "terminal"],
            &["-w", "code", "focus"],
        ] {
            assert!(parse_window_command(refused).is_err(), "{refused:?}");
        }
    }
}
