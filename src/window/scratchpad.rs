use crate::display::manager::DisplayManager;
use crate::ffi::skylight::{_SLPSSetFrontProcessWithOptions, SLSWindowIsOrderedIn};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::scripting_addition::client::{
    scripting_addition_order_window, scripting_addition_order_window_in,
};
use crate::space::focus::space_manager_active_space;
use crate::space::manager::SpaceManager;
use crate::space::moving_windows::space_manager_move_window_to_space;
use crate::state::mission_control_mode::MissionControlMode;
use crate::state::process_wide::CONNECTION;
use crate::support::handles::WindowId;
use crate::window::discovery::{
    space_manager_refresh_application_windows, window_manager_existing_application_window_list,
};
use crate::window::floating_and_sticky::window_manager_make_window_floating;
use crate::window::focus::{
    kCPSNoWindows, window_manager_focus_window_with_raise_resolving_its_application,
};
use crate::window::manager::WindowManager;
use crate::window::model::{window_is_sticky, window_space};
use crate::window::screen_lookup::window_manager_find_window_on_space_by_rank_filtering_window;

pub(crate) struct Scratchpad {
    pub(crate) label: String,
    pub(crate) window_id: WindowId,
}

pub(crate) fn window_manager_find_scratchpad_window(
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

pub(crate) fn window_manager_toggle_scratchpad_window_by_label(
    window_manager: &mut WindowManager,
    label: &[u8],
    process_manager: &mut ProcessManager,
) -> bool {
    let window = window_manager_find_scratchpad_window(window_manager, label);
    match window {
        Some(window_id) => {
            window_manager_toggle_scratchpad_window(window_manager, window_id, 0, process_manager)
        }
        None => false,
    }
}

pub(crate) fn window_manager_toggle_scratchpad_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    forced_mode: i32,
    process_manager: &mut ProcessManager,
) -> bool {
    let space_id = space_manager_active_space(window_manager);
    if space_id.0 == 0 {
        return false;
    }

    // TODO(asmvik): Both functions use the same underlying API and could be combined in a single function to reduce redundant work.
    let visible_space = window_space(window_id) == space_id || window_is_sticky(window_id);

    let mut ordered_in: u8 = 0;
    unsafe { SLSWindowIsOrderedIn(*CONNECTION.get().unwrap(), window_id.0, &mut ordered_in) };

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
        let next = window_manager_find_window_on_space_by_rank_filtering_window(
            window_manager,
            space_id,
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
        scripting_addition_order_window(window_id, 0, WindowId(0));
    } else if mode == 2 {
        scripting_addition_order_window(window_id, 1, WindowId(0));
        window_manager_focus_window_with_raise_resolving_its_application(window_manager, window_id);
    } else {
        space_manager_move_window_to_space(space_id, window_id);
        scripting_addition_order_window(window_id, 1, WindowId(0));
        window_manager_focus_window_with_raise_resolving_its_application(window_manager, window_id);
    }

    true
}

pub(crate) fn window_manager_set_scratchpad_for_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    label: String,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> bool {
    let existing_window = window_manager_find_scratchpad_window(window_manager, label.as_bytes());
    if existing_window.is_some() {
        return false;
    }

    window_manager_remove_scratchpad_for_window(
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
    if let Some(window) = window_manager.window.find_mut(&window_id) {
        window.scratchpad = Some(window_scratchpad);
    }
    window_manager_make_window_floating(
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

pub(crate) fn window_manager_remove_scratchpad_for_window(
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
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                window.scratchpad = None;
            }

            window_manager.scratchpad_window.swap_remove(index);

            if unfloat {
                window_manager_toggle_scratchpad_window(
                    window_manager,
                    window_id,
                    3,
                    process_manager,
                );
                window_manager_make_window_floating(
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

pub(crate) fn window_manager_scratchpad_recover_windows(
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let Some(window_list) = window_manager_existing_application_window_list(None, window_manager)
    else {
        return;
    };

    if scripting_addition_order_window_in(&window_list) {
        space_manager_refresh_application_windows(
            space_manager,
            process_manager,
            display_manager,
            window_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    }
}
