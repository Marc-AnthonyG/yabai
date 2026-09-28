use crate::display::identity::query_displays_active_for_drawing;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_spaces_of_display;
use crate::mouse::drag::MouseDragState;
use crate::serialise::untracked_window::write_untracked_window_as_json_object;
use crate::serialise::window::write_tracked_window_as_json_object;
use crate::space::managed_space::query_windows_on_spaces_owned_by_connection;
use crate::space::manager::SpaceManager;
use crate::support::handles::{DisplayId, SpaceId};
use crate::support::response::Response;
use crate::window::manager::{WindowManager, tracked_window_with_id};

pub(crate) fn write_windows_on_spaces_as_json_array(
    response: &mut Response,
    space_list: &[SpaceId],
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let window_list =
        query_windows_on_spaces_owned_by_connection(space_list, 0, true, window_manager)
            .unwrap_or_default();

    response.write(format_args!("["));
    for index in 0..window_list.len() as i32 {
        let window = tracked_window_with_id(window_manager, window_list[index as usize]);
        if let Some(window_id) = window {
            write_tracked_window_as_json_object(
                response,
                window_id,
                flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        } else {
            write_untracked_window_as_json_object(
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

pub(crate) fn write_windows_on_display_as_json_array(
    response: &mut Response,
    display_id: DisplayId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let space_list = query_spaces_of_display(display_id).unwrap_or_default();
    write_windows_on_spaces_as_json_array(
        response,
        &space_list,
        flags,
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    );
}

pub(crate) fn write_windows_on_every_display_as_json_array(
    response: &mut Response,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let display_list = query_displays_active_for_drawing();

    let mut space_list: Vec<SpaceId> = Vec::new();

    for index in 0..display_list.len() {
        let Some(list) = query_spaces_of_display(display_list[index]) else {
            continue;
        };

        //
        // NOTE(asmvik): display_space_list(..) uses a linear allocator,
        // and so we only need to track the beginning of the first list along
        // with the total number of spaces that have been allocated.
        //

        space_list.extend(list);
    }

    write_windows_on_spaces_as_json_array(
        response,
        &space_list,
        flags,
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    );
}
