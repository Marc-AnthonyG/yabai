#![allow(deprecated)]

use crate::display::bounds::query_center_of_display;
use crate::display::focus::{
    focus_display_through_its_front_window_or_a_click_at_its_center,
    move_active_menu_bar_to_display,
};
use crate::display::identity::{
    query_display_showing_the_active_menu_bar, query_display_under_the_cursor,
};
use crate::display::manager::DisplayManager;
use crate::display::spaces::{
    is_display_animating_a_space_transition, query_current_space_of_display,
};
use crate::ffi::carbon_process::CoreDockSendNotification;
use crate::ffi::core_foundation::{
    show_all_windows_dock_notification_name, show_desktop_dock_notification_name,
};
use crate::ffi::core_graphics::{
    CGEventCreate, CGEventField, CGEventPost, CGEventSetDoubleValueField,
    CGEventSetIntegerValueField, CGPostMouseEvent, CGWarpMouseCursorPosition, kCGSessionEventTap,
};
use crate::mouse::drag::MouseDragState;
use crate::scripting_addition::client::focus_space_through_scripting_addition;
use crate::space::lookup::query_mission_control_index_of_space;
use crate::space::managed_space::query_display_holding_space;
use crate::space::manager::SpaceManager;
use crate::space::operations::{
    SpaceOperationOutcome, swap_spaces_across_displays_by_exchanging_their_windows,
};
use crate::state::mission_control_mode::{MissionControlMode, is_mission_control_active};
use crate::support::handles::{DisplayId, SpaceId, WindowId};
use crate::window::focus::query_focused_tracked_window;
use crate::window::manager::WindowManager;
use crate::window::model::query_display_holding_window;

pub(crate) fn focus_space_then_toggle_mission_control(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) {
    focus_space_through_the_scripting_addition_or_dock_swipes(
        space_id,
        window_manager,
        mission_control_mode,
    );
    unsafe { CoreDockSendNotification(show_all_windows_dock_notification_name(), 0) };
}

pub(crate) fn focus_space_then_toggle_show_desktop(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) {
    focus_space_through_the_scripting_addition_or_dock_swipes(
        space_id,
        window_manager,
        mission_control_mode,
    );
    unsafe { CoreDockSendNotification(show_desktop_dock_notification_name(), 0) };
}

pub(crate) fn query_current_space_of_the_focused_display(
    window_manager: &mut WindowManager,
) -> SpaceId {
    let mut display_id = DisplayId(0);
    let window = query_focused_tracked_window(window_manager);

    if let Some(window_id) = window {
        display_id = query_display_holding_window(window_id);
    }
    if display_id == DisplayId(0) {
        display_id = query_display_showing_the_active_menu_bar();
    }
    if display_id == DisplayId(0) {
        return SpaceId(0);
    }

    query_current_space_of_display(display_id)
}

pub(crate) fn query_current_space_of_display_holding_window_or_else_of_the_active_menu_bar_display(
    window_id: WindowId,
) -> SpaceId {
    let mut display_id = query_display_holding_window(window_id);
    if display_id == DisplayId(0) {
        display_id = query_display_showing_the_active_menu_bar();
    }
    if display_id == DisplayId(0) {
        return SpaceId(0);
    }

    query_current_space_of_display(display_id)
}

pub(crate) fn focus_space_with_synthesized_dock_swipes(
    new_display_id: DisplayId,
    new_space_id: SpaceId,
    window_manager: &mut WindowManager,
) -> bool {
    let current_index =
        query_mission_control_index_of_space(query_current_space_of_display(new_display_id));
    let new_index = query_mission_control_index_of_space(new_space_id);

    let count = (new_index - current_index).abs();
    if count == 0 {
        focus_display_through_its_front_window_or_a_click_at_its_center(
            new_display_id,
            new_space_id,
            window_manager,
        );
        return true;
    }

    let point = query_center_of_display(new_display_id);
    let current_display_id = query_display_under_the_cursor();

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
    CGEventSetIntegerValueField(
        Some(&event_dock_control),
        /* kCGSEventTypeField            */ CGEventField(55),
        /* kCGSEventDockControl       */ 30,
    );
    CGEventSetIntegerValueField(
        Some(&event_dock_control),
        /* kCGEventGestureHIDType        */ CGEventField(110),
        /* kIOHIDEventTypeDockSwipe   */ 23,
    );
    CGEventSetIntegerValueField(
        Some(&event_dock_control),
        /* kCGEventGestureSwipeMotion    */ CGEventField(123),
        /* kCGGestureMotionHorizontal */ 1,
    );
    CGEventSetDoubleValueField(
        Some(&event_dock_control),
        /* kCGEventGestureSwipeProgress  */ CGEventField(124),
        sign as f64,
    );
    CGEventSetDoubleValueField(
        Some(&event_dock_control),
        /* kCGEventGestureSwipeVelocityX */ CGEventField(129),
        sign as f64 * 9999.0f64,
    );

    for _ in 0..count {
        CGEventSetIntegerValueField(
            Some(&event_dock_control),
            /* kCGEventGesturePhase */ CGEventField(132),
            /* kCGSGesturePhaseBegan */ 1,
        );
        CGEventPost(kCGSessionEventTap, Some(&event_dock_control));
        CGEventSetIntegerValueField(
            Some(&event_dock_control),
            /* kCGEventGesturePhase */ CGEventField(132),
            /* kCGSGesturePhaseEnded */ 4,
        );
        CGEventPost(kCGSessionEventTap, Some(&event_dock_control));
    }
    drop(event_dock_control);

    if focus_display {
        move_active_menu_bar_to_display(new_display_id);
        if query_current_space_of_the_focused_display(window_manager) != new_space_id {
            unsafe { CGPostMouseEvent(point, 0, 1, 1) };
            unsafe { CGPostMouseEvent(point, 0, 1, 0) };
        }
    }

    true
}

pub(crate) fn focus_space_through_the_scripting_addition_or_dock_swipes(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOperationOutcome {
    let is_in_mission_control = is_mission_control_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOperationOutcome::MissionControlIsActive;
    }

    let current_space_id = query_current_space_of_the_focused_display(window_manager);
    if current_space_id == space_id {
        return SpaceOperationOutcome::SameSpace;
    }

    let current_display_id = query_display_holding_space(current_space_id);
    let new_display_id = query_display_holding_space(space_id);
    let focus_display = current_display_id != new_display_id;

    let is_animating = is_display_animating_a_space_transition(new_display_id);
    if is_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    if focus_space_through_scripting_addition(space_id) {
        if focus_display {
            focus_display_through_its_front_window_or_a_click_at_its_center(
                new_display_id,
                space_id,
                window_manager,
            );
        }
    } else {
        focus_space_with_synthesized_dock_swipes(new_display_id, space_id, window_manager);
    }

    SpaceOperationOutcome::Success
}

pub(crate) fn switch_to_space_bringing_it_to_the_current_display(
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mission_control_mode: &mut MissionControlMode,
    mouse_drag_state: &mut MouseDragState,
) -> SpaceOperationOutcome {
    let is_in_mission_control = is_mission_control_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOperationOutcome::MissionControlIsActive;
    }

    let current_space_id = query_current_space_of_the_focused_display(window_manager);
    if current_space_id == space_id {
        return SpaceOperationOutcome::SameSpace;
    }

    let current_display_id = query_display_holding_space(current_space_id);
    let display_id = query_display_holding_space(space_id);

    let is_source_animating = is_display_animating_a_space_transition(current_display_id);
    if is_source_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    let is_destination_animating = is_display_animating_a_space_transition(display_id);
    if is_destination_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    if current_display_id != display_id {
        swap_spaces_across_displays_by_exchanging_their_windows(
            current_display_id,
            current_space_id,
            display_id,
            space_id,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
        focus_display_through_its_front_window_or_a_click_at_its_center(
            current_display_id,
            current_space_id,
            window_manager,
        );
        return SpaceOperationOutcome::Success;
    }

    if focus_space_through_scripting_addition(space_id) {
        SpaceOperationOutcome::Success
    } else {
        SpaceOperationOutcome::ScriptingAdditionFailed
    }
}
