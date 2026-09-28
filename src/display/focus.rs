#![allow(deprecated)]

use core::ffi::c_int;
use core::ptr::NonNull;

use crate::display::bounds::query_center_of_display;
use crate::display::identity::copy_uuid_of_display;
use crate::display::spaces::{
    is_display_animating_a_space_transition, query_current_space_of_display,
};
use crate::ffi::accessibility::{
    AXUIElement, AXUIElementCopyAttributeValue, AXUIElementCopyElementAtPosition, kAXRoleAttribute,
    kAXWindowAttribute, kAXWindowRole, read_window_id_of_accessibility_element,
};
use crate::ffi::carbon_process::ProcessSerialNumber;
use crate::ffi::core_foundation::{
    CFEqual, CFRetained, CFType, CGPoint, as_cftype, take_create_rule_result,
};
use crate::ffi::core_graphics::{CGPostMouseEvent, CGWarpMouseCursorPosition};
use crate::ffi::skylight::{
    SLSGetConnectionPSN, SLSGetWindowOwner, SLSSetActiveMenuBarDisplayIdentifier,
};
use crate::scripting_addition::client::focus_space_through_scripting_addition;
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::managed_space::query_display_holding_space;
use crate::space::operations::SpaceOperationOutcome;
use crate::state::mission_control_mode::{MissionControlMode, is_mission_control_active};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{DisplayId, SpaceId, WindowId};
use crate::window::focus::{
    focus_and_raise_window_of_process, warp_cursor_to_window_center_if_mouse_follows_focus,
};
use crate::window::manager::WindowManager;
use crate::window::screen_lookup::query_tracked_window_at_rank_on_space_skipping_window;

pub(crate) fn copy_accessibility_window_element_at_point(
    point: CGPoint,
    window_manager: &mut WindowManager,
) -> Option<CFRetained<AXUIElement>> {
    let mut element_ref: *const AXUIElement = core::ptr::null();
    unsafe {
        AXUIElementCopyElementAtPosition(
            &*window_manager.system_element,
            point.x as f32,
            point.y as f32,
            NonNull::from(&mut element_ref),
        )
    };
    let element_ref = unsafe { take_create_rule_result(element_ref) }?;

    let mut role: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(&element_ref, kAXRoleAttribute(), NonNull::from(&mut role))
    };
    let role = unsafe { take_create_rule_result(role) }?;

    if CFEqual(Some(&role), Some(as_cftype(kAXWindowRole()))) {
        return Some(element_ref);
    }

    let mut window_ref: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &element_ref,
            kAXWindowAttribute(),
            NonNull::from(&mut window_ref),
        )
    };
    let window_ref = unsafe { take_create_rule_result(window_ref.cast::<AXUIElement>()) };
    drop(element_ref);
    drop(role);
    window_ref
}

pub(crate) fn focus_window_under_point(
    point: CGPoint,
    window_manager: &mut WindowManager,
) -> WindowId {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();

    let mut element_connection: c_int = 0;
    let mut element_process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };

    let Some(element_ref) = copy_accessibility_window_element_at_point(point, window_manager)
    else {
        return WindowId(0);
    };

    let element_id = read_window_id_of_accessibility_element(&element_ref);
    if element_id == 0 {
        return WindowId(0);
    }

    unsafe { SLSGetWindowOwner(connection_id, element_id, &mut element_connection) };
    unsafe { SLSGetConnectionPSN(element_connection, &mut element_process_serial_number) };
    focus_and_raise_window_of_process(
        &element_process_serial_number,
        WindowId(element_id),
        &*element_ref,
    );
    WindowId(element_id)
}

pub(crate) fn move_active_menu_bar_to_display(display_id: DisplayId) {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let Some(uuid) = copy_uuid_of_display(display_id) else {
        return;
    };
    unsafe { SLSSetActiveMenuBarDisplayIdentifier(connection_id, uuid.as_ref(), uuid.as_ref()) };
}

pub(crate) fn focus_display_through_its_front_window_or_a_click_at_its_center(
    display_id: DisplayId,
    space_id: SpaceId,
    window_manager: &mut WindowManager,
) {
    let window_id = query_tracked_window_at_rank_on_space_skipping_window(
        window_manager,
        space_id,
        1,
        WindowId(0),
    );
    if let Some(window_id) = window_id {
        let Some(window) = window_manager.window.get(&window_id) else {
            return;
        };
        let window_element_ref = window.element_ref;
        let Some(window_process_id) = window.application else {
            return;
        };
        let Some(application) = window_manager.application.get(&window_process_id) else {
            return;
        };
        let window_process_serial_number = application.process_serial_number;

        focus_and_raise_window_of_process(
            &window_process_serial_number,
            window_id,
            window_element_ref,
        );
        warp_cursor_to_window_center_if_mouse_follows_focus(window_manager, window_id);
        move_active_menu_bar_to_display(display_id);
    } else {
        let point = query_center_of_display(display_id);
        CGWarpMouseCursorPosition(point);
        move_active_menu_bar_to_display(display_id);

        if query_current_space_of_the_focused_display(window_manager)
            != query_current_space_of_display(display_id)
        {
            unsafe { CGPostMouseEvent(point, 0, 1, 1) };
            unsafe { CGPostMouseEvent(point, 0, 1, 0) };
        }
    }
}

pub(crate) fn focus_space_if_it_is_on_display(
    display_id: DisplayId,
    space_id: SpaceId,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOperationOutcome {
    let is_in_mission_control = is_mission_control_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOperationOutcome::MissionControlIsActive;
    }

    let is_animating = is_display_animating_a_space_transition(display_id);
    if is_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    let space_display_id = query_display_holding_space(space_id);
    if space_display_id != display_id {
        return SpaceOperationOutcome::NotOnTheSameDisplay;
    }

    if focus_space_through_scripting_addition(space_id) {
        SpaceOperationOutcome::Success
    } else {
        SpaceOperationOutcome::ScriptingAdditionFailed
    }
}
