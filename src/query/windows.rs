use crate::display::identity::query_displays_active_for_drawing;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_spaces_of_display;
use crate::mouse::drag::MouseDragState;
use crate::serialise::untracked_window::snapshot_of_untracked_window;
use crate::serialise::window::{WindowSnapshot, snapshot_of_tracked_window};
use crate::space::managed_space::query_windows_on_spaces_owned_by_connection;
use crate::space::manager::SpaceManager;
use crate::support::handles::{DisplayId, SpaceId};
use crate::window::manager::{WindowManager, tracked_window_with_id};

pub(crate) fn snapshots_of_the_windows_on_spaces(
    space_list: &[SpaceId],
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &MouseDragState,
) -> Vec<WindowSnapshot> {
    let window_list =
        query_windows_on_spaces_owned_by_connection(space_list, 0, true, window_manager)
            .unwrap_or_default();

    window_list
        .into_iter()
        .filter_map(
            |window_id| match tracked_window_with_id(window_manager, window_id) {
                Some(tracked_window_id) => snapshot_of_tracked_window(
                    tracked_window_id,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                ),
                None => Some(snapshot_of_untracked_window(window_id, display_manager)),
            },
        )
        .collect()
}

pub(crate) fn snapshots_of_the_windows_on_display(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &MouseDragState,
) -> Vec<WindowSnapshot> {
    snapshots_of_the_windows_on_spaces(
        &query_spaces_of_display(display_id).unwrap_or_default(),
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    )
}

pub(crate) fn snapshots_of_the_windows_on_every_display(
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &MouseDragState,
) -> Vec<WindowSnapshot> {
    let space_list: Vec<SpaceId> = query_displays_active_for_drawing()
        .into_iter()
        .filter_map(query_spaces_of_display)
        .flatten()
        .collect();
    snapshots_of_the_windows_on_spaces(
        &space_list,
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    )
}
