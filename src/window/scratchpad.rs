use crate::display::manager::DisplayManager;
use crate::ffi::skylight::{_SLPSSetFrontProcessWithOptions, SLSWindowIsOrderedIn};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::scripting_addition::client::{
    order_in_windows_not_yet_ordered_in_through_scripting_addition,
    order_window_relative_to_other_window_through_scripting_addition,
};
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::manager::SpaceManager;
use crate::space::moving_windows::move_window_to_space_by_whichever_mechanism_this_macos_supports;
use crate::state::mission_control_mode::MissionControlMode;
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::WindowId;
use crate::window::discovery::{
    query_application_windows_on_every_space,
    retry_tracking_windows_of_applications_with_unresolved_windows,
};
use crate::window::floating_and_sticky::set_whether_window_floats;
use crate::window::focus::{focus_and_raise_tracked_window, kCPSNoWindows};
use crate::window::manager::WindowManager;
use crate::window::model::{is_window_on_more_than_one_space, query_space_holding_window};
use crate::window::screen_lookup::query_tracked_window_at_rank_on_space_skipping_window;

pub(crate) struct Scratchpad {
    pub(crate) label: String,
    pub(crate) window_id: WindowId,
}

pub(crate) fn scratchpad_window_with_label(
    window_manager: &mut WindowManager,
    label: &[u8],
) -> Option<WindowId> {
    for index in 0..window_manager.scratchpad_window.len() {
        if window_manager.scratchpad_window[index].label.as_bytes() == label {
            return Some(window_manager.scratchpad_window[index].window_id);
        }
    }

    None
}

pub(crate) fn toggle_scratchpad_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    forced_mode: i32,
    process_manager: &mut ProcessManager,
) -> bool {
    let space_id = query_current_space_of_the_focused_display(window_manager);
    if space_id.0 == 0 {
        return false;
    }

    // TODO(asmvik): Both functions use the same underlying API and could be combined in a single function to reduce redundant work.
    let visible_space = query_space_holding_window(window_id) == space_id
        || is_window_on_more_than_one_space(window_id);

    let mut ordered_in: u8 = 0;
    unsafe {
        SLSWindowIsOrderedIn(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            window_id.0,
            &mut ordered_in,
        )
    };

    let mode = match forced_mode {
        1 | 2 | 3 => forced_mode,
        _ => {
            if visible_space && ordered_in != 0 {
                1
            } else if visible_space && ordered_in == 0 {
                2
            } else {
                3
            }
        }
    };

    if mode == 1 {
        let next = query_tracked_window_at_rank_on_space_skipping_window(
            window_manager,
            space_id,
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
        order_window_relative_to_other_window_through_scripting_addition(window_id, 0, WindowId(0));
    } else if mode == 2 {
        order_window_relative_to_other_window_through_scripting_addition(window_id, 1, WindowId(0));
        focus_and_raise_tracked_window(window_manager, window_id);
    } else {
        move_window_to_space_by_whichever_mechanism_this_macos_supports(space_id, window_id);
        order_window_relative_to_other_window_through_scripting_addition(window_id, 1, WindowId(0));
        focus_and_raise_tracked_window(window_manager, window_id);
    }

    true
}

pub(crate) fn assign_window_to_scratchpad_making_it_float(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    label: String,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> bool {
    let existing_window = scratchpad_window_with_label(window_manager, label.as_bytes());
    if existing_window.is_some() {
        return false;
    }

    remove_window_from_its_scratchpad(
        window_manager,
        window_id,
        false,
        process_manager,
        display_manager,
        space_manager,
        mouse_drag_state,
    );
    let window_scratchpad = label.clone();
    window_manager
        .scratchpad_window
        .push(Scratchpad { label, window_id });
    if let Some(window) = window_manager.window.get_mut(&window_id) {
        window.scratchpad = Some(window_scratchpad);
    }
    set_whether_window_floats(
        space_manager,
        window_manager,
        window_id,
        true,
        false,
        display_manager,
        mouse_drag_state,
    );

    true
}

pub(crate) fn remove_window_from_its_scratchpad(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    unfloat: bool,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> bool {
    for index in 0..window_manager.scratchpad_window.len() {
        if window_manager.scratchpad_window[index].window_id == window_id {
            if let Some(window) = window_manager.window.get_mut(&window_id) {
                window.scratchpad = None;
            }

            window_manager.scratchpad_window.swap_remove(index);

            if unfloat {
                toggle_scratchpad_window(window_manager, window_id, 3, process_manager);
                set_whether_window_floats(
                    space_manager,
                    window_manager,
                    window_id,
                    false,
                    false,
                    display_manager,
                    mouse_drag_state,
                );
            }

            return true;
        }
    }

    false
}

pub(crate) fn recover_hidden_scratchpad_windows_by_ordering_every_window_in(
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let Some(window_list) = query_application_windows_on_every_space(None, window_manager) else {
        return;
    };

    if order_in_windows_not_yet_ordered_in_through_scripting_addition(&window_list) {
        retry_tracking_windows_of_applications_with_unresolved_windows(
            space_manager,
            process_manager,
            display_manager,
            window_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    }
}
