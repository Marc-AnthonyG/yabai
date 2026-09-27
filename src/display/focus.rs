#![allow(deprecated)]

use core::ffi::c_int;
use core::ptr::NonNull;

use crate::display::bounds::display_center;
use crate::display::identity::display_uuid;
use crate::display::spaces::{display_manager_display_is_animating, display_space_id};
use crate::ffi::accessibility::{
    AXUIElement, AXUIElementCopyAttributeValue, AXUIElementCopyElementAtPosition, ax_window_id,
    kAXRoleAttribute, kAXWindowAttribute, kAXWindowRole,
};
use crate::ffi::carbon_process::ProcessSerialNumber;
use crate::ffi::core_foundation::{
    CFEqual, CFRetained, CFType, CGPoint, as_cftype, take_create_rule_result,
};
use crate::ffi::core_graphics::{CGPostMouseEvent, CGWarpMouseCursorPosition};
use crate::ffi::skylight::{
    SLSGetConnectionPSN, SLSGetWindowOwner, SLSSetActiveMenuBarDisplayIdentifier,
};
use crate::globals::CONNECTION;
use crate::handles::{DisplayId, SpaceId, WindowId};
use crate::mission_control::{MissionControlMode, mission_control_is_active};
use crate::scripting_addition::client::scripting_addition_focus_space;
use crate::space::focus::space_manager_active_space;
use crate::space::managed_space::space_display_id;
use crate::space::operations::SpaceOpError;
use crate::window::focus::{window_manager_center_mouse, window_manager_focus_window_with_raise};
use crate::window::manager::WindowManager;
use crate::window::screen_lookup::window_manager_find_window_on_space_by_rank_filtering_window;

pub(crate) fn display_manager_find_element_at_point(
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
        AXUIElementCopyAttributeValue(
            &element_ref,
            kAXRoleAttribute(),
            NonNull::from(&mut role),
        )
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

pub(crate) fn display_manager_focus_display_with_window_at_point(
    point: CGPoint,
    window_manager: &mut WindowManager,
) -> WindowId {
    let connection_id = *CONNECTION.get().unwrap();

    let mut element_connection: c_int = 0;
    let mut element_process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };

    let Some(element_ref) = display_manager_find_element_at_point(point, window_manager) else {
        return WindowId(0);
    };

    let element_id = ax_window_id(&element_ref);
    if element_id == 0 {
        return WindowId(0);
    }

    unsafe { SLSGetWindowOwner(connection_id, element_id, &mut element_connection) };
    unsafe { SLSGetConnectionPSN(element_connection, &mut element_process_serial_number) };
    window_manager_focus_window_with_raise(
        &element_process_serial_number,
        WindowId(element_id),
        &*element_ref,
    );
    WindowId(element_id)
}

pub(crate) fn display_manager_set_active_display_id(display_id: DisplayId) {
    let connection_id = *CONNECTION.get().unwrap();
    let Some(uuid) = display_uuid(display_id) else {
        return;
    };
    unsafe { SLSSetActiveMenuBarDisplayIdentifier(connection_id, uuid.as_ref(), uuid.as_ref()) };
}

pub(crate) fn display_manager_focus_display(
    display_id: DisplayId,
    space_id: SpaceId,
    window_manager: &mut WindowManager,
) {
    let window_id = window_manager_find_window_on_space_by_rank_filtering_window(
        window_manager,
        space_id,
        1,
        WindowId(0),
    );
    if let Some(window_id) = window_id {
        let Some(window) = window_manager.window.find(&window_id) else {
            return;
        };
        let window_element_ref = window.element_ref;
        let Some(window_process_id) = window.application else {
            return;
        };
        let Some(application) = window_manager.application.find(&window_process_id) else {
            return;
        };
        let window_process_serial_number = application.process_serial_number;

        window_manager_focus_window_with_raise(
            &window_process_serial_number,
            window_id,
            window_element_ref,
        );
        window_manager_center_mouse(window_manager, window_id);
        display_manager_set_active_display_id(display_id);
    } else {
        let point = display_center(display_id);
        CGWarpMouseCursorPosition(point);
        display_manager_set_active_display_id(display_id);

        if space_manager_active_space(window_manager) != display_space_id(display_id) {
            unsafe { CGPostMouseEvent(point, 0, 1, 1) };
            unsafe { CGPostMouseEvent(point, 0, 1, 0) };
        }
    }
}

pub(crate) fn display_manager_focus_space(
    display_id: DisplayId,
    space_id: SpaceId,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }

    let is_animating = display_manager_display_is_animating(display_id);
    if is_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    let space_display_id = space_display_id(space_id);
    if space_display_id != display_id {
        return SpaceOpError::SameDisplay;
    }

    if scripting_addition_focus_space(space_id) {
        SpaceOpError::Success
    } else {
        SpaceOpError::ScriptingAddition
    }
}
