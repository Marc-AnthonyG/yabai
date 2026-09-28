use crate::display::manager::DisplayManager;
use crate::ffi::skylight::{_SLPSSetFrontProcessWithOptions, SLSSpaceSetFrontPSN};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::space::managed_space::is_space_visible_on_its_display;
use crate::space::manager::SpaceManager;
use crate::space::moving_windows::move_window_to_space_by_whichever_mechanism_this_macos_supports;
use crate::space::tiling::{tile_window_on_space, untile_window_from_view_of_space};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{SpaceId, WindowId};
use crate::window::focus::{focus_and_raise_tracked_window, kCPSNoWindows};
use crate::window::focus_follows_mouse::schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles;
use crate::window::manager::{
    WindowManager, forget_managed_window, record_managed_window_on_space_updating_its_shadow,
    should_window_be_managed, space_managing_window,
};
use crate::window::model::query_space_holding_window;
use crate::window::screen_lookup::query_tracked_window_at_rank_on_space_skipping_window;
use crate::window::shadow::apply_shadow_removal_mode_to_window;

pub(crate) fn send_window_to_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    destination_space_id: SpaceId,
    moved_by_rule: bool,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let source_space_id = query_space_holding_window(window_id);
    if source_space_id == destination_space_id {
        return;
    }
    let window_leaves_the_screen = is_space_visible_on_its_display(source_space_id)
        && !is_space_visible_on_its_display(destination_space_id);

    if is_space_visible_on_its_display(source_space_id)
        && (moved_by_rule || window_manager.focused_window_id == window_id)
    {
        let next = query_tracked_window_at_rank_on_space_skipping_window(
            window_manager,
            source_space_id,
            1,
            window_id,
        );
        if let Some(next) = next {
            focus_and_raise_tracked_window(window_manager, next);
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

    let view = space_managing_window(window_manager, window_id);
    if let Some(space_id) = view {
        untile_window_from_view_of_space(
            space_manager,
            space_id,
            window_id,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        forget_managed_window(window_manager, window_id);
        apply_shadow_removal_mode_to_window(window_manager, window_id);
    }

    move_window_to_space_by_whichever_mechanism_this_macos_supports(
        destination_space_id,
        window_id,
    );
    if let Some(application) = window_manager
        .window
        .find(&window_id)
        .and_then(|window| window.application)
        .and_then(|application_process_id| window_manager.application.find(&application_process_id))
    {
        unsafe {
            SLSSpaceSetFrontPSN(
                *SKYLIGHT_CONNECTION_ID.get().unwrap(),
                destination_space_id.0,
                application.process_serial_number,
            )
        };
    }

    if should_window_be_managed(window_id, window_manager) {
        let view = tile_window_on_space(
            space_manager,
            window_id,
            destination_space_id,
            display_manager,
            window_manager,
        );
        record_managed_window_on_space_updating_its_shadow(
            window_manager,
            window_id,
            space_manager,
            view,
        );
    }

    if window_leaves_the_screen {
        schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles(window_manager);
    }
}
