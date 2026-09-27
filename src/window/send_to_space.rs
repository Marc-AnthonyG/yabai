use crate::display::manager::DisplayManager;
use crate::ffi::skylight::{_SLPSSetFrontProcessWithOptions, SLSSpaceSetFrontPSN};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::space::managed_space::space_is_visible;
use crate::space::manager::SpaceManager;
use crate::space::moving_windows::space_manager_move_window_to_space;
use crate::space::tiling::{space_manager_tile_window_on_space, space_manager_untile_window};
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{SpaceId, WindowId};
use crate::window::focus::{
    kCPSNoWindows, window_manager_focus_window_with_raise_resolving_its_application,
};
use crate::window::manager::{
    WindowManager, window_manager_add_managed_window, window_manager_find_managed_window,
    window_manager_remove_managed_window, window_manager_should_manage_window,
};
use crate::window::model::window_space;
use crate::window::screen_lookup::window_manager_find_window_on_space_by_rank_filtering_window;
use crate::window::shadow::window_manager_purify_window;

pub(crate) fn window_manager_send_window_to_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    destination_space_id: SpaceId,
    moved_by_rule: bool,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let source_space_id = window_space(window_id);
    if source_space_id == destination_space_id {
        return;
    }

    if space_is_visible(source_space_id)
        && (moved_by_rule || window_manager.focused_window_id == window_id)
    {
        let next = window_manager_find_window_on_space_by_rank_filtering_window(
            window_manager,
            source_space_id,
            1,
            window_id,
        );
        if let Some(next) = next {
            window_manager_focus_window_with_raise_resolving_its_application(window_manager, next);
        } else {
            unsafe {
                _SLPSSetFrontProcessWithOptions(
                    &mut process_manager.finder_process_serial_number,
                    0,
                    kCPSNoWindows,
                )
            };
        }
    }

    let view = window_manager_find_managed_window(window_manager, window_id);
    if let Some(space_id) = view {
        space_manager_untile_window(
            space_manager,
            space_id,
            window_id,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        window_manager_remove_managed_window(window_manager, window_id);
        window_manager_purify_window(window_manager, window_id);
    }

    space_manager_move_window_to_space(destination_space_id, window_id);
    if let Some(application) = window_manager
        .window
        .find(&window_id)
        .and_then(|window| window.application)
        .and_then(|application_process_id| window_manager.application.find(&application_process_id))
    {
        unsafe {
            SLSSpaceSetFrontPSN(
                *CONNECTION.get().unwrap(),
                destination_space_id.0,
                application.process_serial_number,
            )
        };
    }

    if window_manager_should_manage_window(window_id, window_manager) {
        let view = space_manager_tile_window_on_space(
            space_manager,
            window_id,
            destination_space_id,
            display_manager,
            window_manager,
        );
        window_manager_add_managed_window(window_manager, window_id, space_manager, view);
    }
}
