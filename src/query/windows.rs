use crate::display::identity::display_manager_active_display_list;
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_list;
use crate::mouse::drag::MouseDragState;
use crate::serialise::untracked_window::window_nonax_serialize;
use crate::serialise::window::window_serialize;
use crate::space::managed_space::space_window_list_for_connection;
use crate::space::manager::SpaceManager;
use crate::support::handles::{DisplayId, SpaceId};
use crate::support::response::Response;
use crate::window::manager::{WindowManager, window_manager_find_window};

pub(crate) fn window_manager_query_windows_for_spaces(
    response: &mut Response,
    space_list: &[SpaceId],
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let window_list =
        space_window_list_for_connection(space_list, 0, true, window_manager).unwrap_or_default();

    response.write(format_args!("["));
    for index in 0..window_list.len() as i32 {
        let window = window_manager_find_window(window_manager, window_list[index as usize]);
        if let Some(window_id) = window {
            window_serialize(
                response,
                window_id,
                flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        } else {
            window_nonax_serialize(
                response,
                window_list[index as usize],
                flags,
                display_manager,
            );
        }
        if index < window_list.len() as i32 - 1 {
            response.write(format_args!(","));
        }
    }
    response.write(format_args!("]\n"));
}

pub(crate) fn window_manager_query_windows_for_display(
    response: &mut Response,
    display_id: DisplayId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let space_list = display_space_list(display_id).unwrap_or_default();
    window_manager_query_windows_for_spaces(
        response,
        &space_list,
        flags,
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    );
}

pub(crate) fn window_manager_query_windows_for_displays(
    response: &mut Response,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let display_list = display_manager_active_display_list();

    let mut space_list: Vec<SpaceId> = Vec::new();

    for index in 0..display_list.len() {
        let Some(list) = display_space_list(display_list[index]) else {
            continue;
        };

        //
        // NOTE(asmvik): display_space_list(..) uses a linear allocator,
        // and so we only need to track the beginning of the first list along
        // with the total number of spaces that have been allocated.
        //

        space_list.extend(list);
    }

    window_manager_query_windows_for_spaces(
        response,
        &space_list,
        flags,
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    );
}
