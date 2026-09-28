use crate::display::manager::DisplayManager;
use crate::layout::tree::{MOST_WINDOWS_A_NODE_CAN_HOLD, leaf_holding_window};
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::support::handles::WindowId;
use crate::window::manager::{WindowManager, tracked_window_with_id};

fn stack_holding_window_in_active_space_view(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<(
    [WindowId; MOST_WINDOWS_A_NODE_CAN_HOLD],
    [WindowId; MOST_WINDOWS_A_NODE_CAN_HOLD],
    i32,
)> {
    let space_id = find_or_create_view_for_space(
        space_manager,
        query_current_space_of_the_focused_display(window_manager),
        display_manager,
        window_manager,
    );
    if !space_manager.view.contains_key(&space_id) {
        return None;
    }

    let node_id = leaf_holding_window(space_manager, space_id, window_id)?;

    let node = space_manager.view.get(&space_id)?.find_node(node_id)?;
    Some((node.window_list, node.window_order, node.window_count))
}

pub(crate) fn previous_window_in_stack_holding_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = stack_holding_window_in_active_space_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    for index in 1..window_count {
        if window_list[index as usize] == window_id {
            return tracked_window_with_id(window_manager, window_list[(index - 1) as usize]);
        }
    }

    None
}

pub(crate) fn next_window_in_stack_holding_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = stack_holding_window_in_active_space_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    for index in 0..window_count - 1 {
        if window_list[index as usize] == window_id {
            return tracked_window_with_id(window_manager, window_list[(index + 1) as usize]);
        }
    }

    None
}

pub(crate) fn first_window_in_stack_holding_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = stack_holding_window_in_active_space_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 {
        tracked_window_with_id(window_manager, window_list[0])
    } else {
        None
    }
}

pub(crate) fn last_window_in_stack_holding_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = stack_holding_window_in_active_space_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 {
        tracked_window_with_id(window_manager, window_list[(window_count - 1) as usize])
    } else {
        None
    }
}

pub(crate) fn previously_focused_window_in_stack_holding_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (_window_list, window_order, window_count) = stack_holding_window_in_active_space_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 {
        tracked_window_with_id(window_manager, window_order[1])
    } else {
        None
    }
}

pub(crate) fn window_at_one_based_position_in_stack_holding_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    index: i32,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = stack_holding_window_in_active_space_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 && (1..=window_count).contains(&index) {
        tracked_window_with_id(window_manager, window_list[(index - 1) as usize])
    } else {
        None
    }
}
