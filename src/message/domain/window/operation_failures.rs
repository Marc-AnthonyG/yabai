use crate::layout::tree::MOST_WINDOWS_A_NODE_CAN_HOLD;
use crate::support::handles::WindowId;
use crate::window::manager::WindowOperationOutcome;

pub(crate) fn fail_unless_the_window_was_minimized(
    outcome: WindowOperationOutcome,
    window_id: WindowId,
) -> Result<(), String> {
    match outcome {
        WindowOperationOutcome::CannotMinimize => Err(format!(
            "window with id '{}' does not support the minimize operation.",
            window_id.0
        )),
        WindowOperationOutcome::AlreadyMinimized => Err(format!(
            "window with id '{}' is already minimized.",
            window_id.0
        )),
        WindowOperationOutcome::MinimizeFailed => Err(format!(
            "could not minimize window with id '{}'.",
            window_id.0
        )),
        _ => Ok(()),
    }
}

pub(crate) fn fail_unless_the_window_was_deminimized(
    outcome: WindowOperationOutcome,
    window_id: WindowId,
) -> Result<(), String> {
    match outcome {
        WindowOperationOutcome::NotMinimized => Err(format!(
            "window with id '{}' is not minimized.",
            window_id.0
        )),
        WindowOperationOutcome::DeminimizeFailed => Err(format!(
            "could not deminimize window with id '{}'.",
            window_id.0
        )),
        _ => Ok(()),
    }
}

pub(crate) fn fail_unless_the_window_moved_in_the_tree(
    outcome: WindowOperationOutcome,
    operation: &str,
) -> Result<(), String> {
    match outcome {
        WindowOperationOutcome::InvalidSourceView => {
            Err(String::from("the acting window is not within a bsp space."))
        }
        WindowOperationOutcome::InvalidDestinationView => Err(String::from(
            "the selected window is not within a bsp space.",
        )),
        WindowOperationOutcome::InvalidSourceNode => {
            Err(String::from("the acting window is not managed."))
        }
        WindowOperationOutcome::InvalidDestinationNode => {
            Err(String::from("the selected window is not managed."))
        }
        WindowOperationOutcome::SameStack => Err(format!(
            "cannot {operation} a window with a window in the same stack."
        )),
        WindowOperationOutcome::SameWindow => {
            Err(format!("cannot {operation} a window with itself."))
        }
        WindowOperationOutcome::StackIsFull => Err(format!(
            "cannot stack window, max capacity of {MOST_WINDOWS_A_NODE_CAN_HOLD} reached."
        )),
        _ => Ok(()),
    }
}

pub(crate) fn fail_unless_the_window_was_resized(
    outcome: WindowOperationOutcome,
) -> Result<(), String> {
    match outcome {
        WindowOperationOutcome::InvalidSourceNode => Err(String::from(
            "cannot locate bsp node for the managed window.",
        )),
        WindowOperationOutcome::InvalidDestinationNode => {
            Err(String::from("cannot locate a bsp node fence."))
        }
        WindowOperationOutcome::InvalidOperation => Err(String::from(
            "cannot use absolute resizing on a managed window.",
        )),
        _ => Ok(()),
    }
}

pub(crate) fn fail_unless_the_split_ratio_changed(
    outcome: WindowOperationOutcome,
) -> Result<(), String> {
    match outcome {
        WindowOperationOutcome::InvalidSourceView => {
            Err(String::from("cannot adjust ratio of a non-managed window."))
        }
        WindowOperationOutcome::InvalidSourceNode => {
            Err(String::from("cannot adjust ratio of a root node."))
        }
        _ => Ok(()),
    }
}
