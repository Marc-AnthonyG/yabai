#![allow(deprecated)]

use crate::display::bounds::display_center;
use crate::display::focus::{display_manager_focus_display, display_manager_set_active_display_id};
use crate::display::identity::{
    display_manager_active_display_id, display_manager_cursor_display_id,
};
use crate::display::manager::DisplayManager;
use crate::display::spaces::{display_manager_display_is_animating, display_space_id};
use crate::ffi::carbon_process::CoreDockSendNotification;
use crate::ffi::core_foundation::{k_com_apple_expose_awake, k_com_apple_showdesktop_awake};
use crate::ffi::core_graphics::{
    CGEventCreate, CGEventField, CGEventPost, CGEventSetDoubleValueField,
    CGEventSetIntegerValueField, CGPostMouseEvent, CGWarpMouseCursorPosition, kCGSessionEventTap,
};
use crate::mouse::drag::MouseDragState;
use crate::scripting_addition::client::scripting_addition_focus_space;
use crate::space::lookup::space_manager_mission_control_index;
use crate::space::managed_space::space_display_id;
use crate::space::manager::SpaceManager;
use crate::space::operations::{SpaceOpError, space_manager_swap_space_with_space_on_display};
use crate::state::mission_control_mode::{MissionControlMode, mission_control_is_active};
use crate::support::handles::{DisplayId, SpaceId};
use crate::window::focus::window_manager_focused_window;
use crate::window::manager::WindowManager;
use crate::window::model::window_display_id;

pub(crate) fn space_manager_toggle_mission_control(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) {
    space_manager_focus_space(space_id, window_manager, mission_control_mode);
    unsafe { CoreDockSendNotification(k_com_apple_expose_awake(), 0) };
}

pub(crate) fn space_manager_toggle_show_desktop(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) {
    space_manager_focus_space(space_id, window_manager, mission_control_mode);
    unsafe { CoreDockSendNotification(k_com_apple_showdesktop_awake(), 0) };
}

pub(crate) fn space_manager_active_space(window_manager: &mut WindowManager) -> SpaceId {
    let mut display_id = DisplayId(0);
    let window = window_manager_focused_window(window_manager);

    if let Some(window_id) = window {
        display_id = window_display_id(window_id);
    }
    if display_id == DisplayId(0) {
        display_id = display_manager_active_display_id();
    }
    if display_id == DisplayId(0) {
        return SpaceId(0);
    }

    display_space_id(display_id)
}

pub(crate) fn space_manager_focus_space_using_gesture(
    new_display_id: DisplayId,
    new_space_id: SpaceId,
    window_manager: &mut WindowManager,
) -> bool {
    let current_index = space_manager_mission_control_index(display_space_id(new_display_id));
    let new_index = space_manager_mission_control_index(new_space_id);

    let count = (new_index - current_index).abs();
    if count == 0 {
        display_manager_focus_display(new_display_id, new_space_id, window_manager);
        return true;
    }

    let point = display_center(new_display_id);
    let current_display_id = display_manager_cursor_display_id();

    let focus_display = current_display_id != new_display_id;
    if focus_display {
        CGWarpMouseCursorPosition(point);
    }

    //
    // NOTE(asmvik): MacOS does not have an API that allows for space activation.
    // However, we can synthesize a sequence of high velocity gestures to skip the
    // animation instead.
    //
    // :Attribution
    // https://github.com/jurplel/InstantSpaceSwitcher
    // https://github.com/thenickdude/wacom-driver-fix/blob/bdfda9a788934c88d09d31ea6a42664b9ba1471e/Readme.md
    // Technique first observed in practice, and reverse-engineered from, BetterTouchTool.
    //

    let Some(event_dock_control) = CGEventCreate(None) else {
        return false;
    };

    let sign: f32 = (if (new_index - current_index) > 0 {
        1.0f64
    } else {
        -1.0f64
    }) as f32;
    CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGSEventTypeField            */ CGEventField(55), /* kCGSEventDockControl       */ 30);
    CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGEventGestureHIDType        */ CGEventField(110), /* kIOHIDEventTypeDockSwipe   */ 23);
    CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGEventGestureSwipeMotion    */ CGEventField(123), /* kCGGestureMotionHorizontal */ 1);
    CGEventSetDoubleValueField(Some(&event_dock_control), /* kCGEventGestureSwipeProgress  */ CGEventField(124), sign as f64);
    CGEventSetDoubleValueField(Some(&event_dock_control), /* kCGEventGestureSwipeVelocityX */ CGEventField(129), sign as f64 * 9999.0f64);

    for _ in 0..count {
        CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGEventGesturePhase */ CGEventField(132), /* kCGSGesturePhaseBegan */ 1);
        CGEventPost(kCGSessionEventTap, Some(&event_dock_control));
        CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGEventGesturePhase */ CGEventField(132), /* kCGSGesturePhaseEnded */ 4);
        CGEventPost(kCGSessionEventTap, Some(&event_dock_control));
    }
    drop(event_dock_control);

    if focus_display {
        display_manager_set_active_display_id(new_display_id);
        if space_manager_active_space(window_manager) != new_space_id {
            unsafe { CGPostMouseEvent(point, 0, 1, 1) };
            unsafe { CGPostMouseEvent(point, 0, 1, 0) };
        }
    }

    true
}

pub(crate) fn space_manager_focus_space(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }

    let current_space_id = space_manager_active_space(window_manager);
    if current_space_id == space_id {
        return SpaceOpError::SameSpace;
    }

    let current_display_id = space_display_id(current_space_id);
    let new_display_id = space_display_id(space_id);
    let focus_display = current_display_id != new_display_id;

    let is_animating = display_manager_display_is_animating(new_display_id);
    if is_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    if scripting_addition_focus_space(space_id) {
        if focus_display {
            display_manager_focus_display(new_display_id, space_id, window_manager);
        }
    } else {
        space_manager_focus_space_using_gesture(new_display_id, space_id, window_manager);
    }

    SpaceOpError::Success
}

pub(crate) fn space_manager_switch_space(
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mission_control_mode: &mut MissionControlMode,
    mouse_drag_state: &mut MouseDragState,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }

    let current_space_id = space_manager_active_space(window_manager);
    if current_space_id == space_id {
        return SpaceOpError::SameSpace;
    }

    let current_display_id = space_display_id(current_space_id);
    let display_id = space_display_id(space_id);

    let is_source_animating = display_manager_display_is_animating(current_display_id);
    if is_source_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    let is_destination_animating = display_manager_display_is_animating(display_id);
    if is_destination_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    if current_display_id != display_id {
        space_manager_swap_space_with_space_on_display(
            current_display_id,
            current_space_id,
            display_id,
            space_id,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
        display_manager_focus_display(current_display_id, current_space_id, window_manager);
        return SpaceOpError::Success;
    }

    if scripting_addition_focus_space(space_id) {
        SpaceOpError::Success
    } else {
        SpaceOpError::ScriptingAddition
    }
}
