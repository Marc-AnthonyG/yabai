use crate::display::manager::DisplayManager;
use crate::layout::tree::{NODE_MAX_WINDOW_COUNT, view_find_window_node};
use crate::space::focus::space_manager_active_space;
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::support::arithmetic::in_range_ii;
use crate::support::handles::WindowId;
use crate::window::manager::{WindowManager, window_manager_find_window};

fn window_node_stack_of_window_in_active_view(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<(
    [WindowId; NODE_MAX_WINDOW_COUNT],
    [WindowId; NODE_MAX_WINDOW_COUNT],
    i32,
)> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return None;
    };

    let node = space_manager.view.find(&space_id)?.find_node(node_id)?;
    Some((node.window_list, node.window_order, node.window_count))
}

pub(crate) fn window_manager_find_prev_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    for index in 1..window_count {
        if window_list[index as usize] == window_id {
            return window_manager_find_window(window_manager, window_list[(index - 1) as usize]);
        }
    }

    None
}

pub(crate) fn window_manager_find_next_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    for index in 0..window_count - 1 {
        if window_list[index as usize] == window_id {
            return window_manager_find_window(window_manager, window_list[(index + 1) as usize]);
        }
    }

    None
}

pub(crate) fn window_manager_find_first_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 {
        window_manager_find_window(window_manager, window_list[0])
    } else {
        None
    }
}

pub(crate) fn window_manager_find_last_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 {
        window_manager_find_window(window_manager, window_list[(window_count - 1) as usize])
    } else {
        None
    }
}

pub(crate) fn window_manager_find_recent_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (_window_list, window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 {
        window_manager_find_window(window_manager, window_order[1])
    } else {
        None
    }
}

pub(crate) fn window_manager_find_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    index: i32,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 && in_range_ii(index, 1, window_count) {
        window_manager_find_window(window_manager, window_list[(index - 1) as usize])
    } else {
        None
    }
}
