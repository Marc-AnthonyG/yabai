#![allow(deprecated)]

use crate::application::read_focused_window_of_application;
use crate::ffi::accessibility::{AXUIElementPerformAction, AXUIElementRef, kAXRaiseAction};
use crate::ffi::carbon_process::{
    CoreDockSendNotification, GetProcessPID, ProcessSerialNumber, is_same_process_serial_number,
};
use crate::ffi::core_foundation::{CGPoint, show_front_windows_dock_notification_name};
use crate::ffi::core_graphics::{CGDisplayBounds, CGRectContainsPoint, CGWarpMouseCursorPosition};
use crate::ffi::skylight::{
    _SLPSGetFrontProcess, _SLPSSetFrontProcessWithOptions, SLPSPostEventRecordTo,
    SLSGetCurrentCursorLocation,
};
use crate::layout::group_header::refresh_the_group_headers_of_view;
use crate::layout::insertion::clear_every_pending_insertion_point_other_than_the_window;
use crate::layout::tree::leaf_holding_window;
use crate::mouse::drag::MouseDragState;
use crate::notifications::mouse::{
    MOUSE_EVENT_MASK_FOR_FOCUS_FOLLOWS_MOUSE, MOUSE_EVENT_MASK_WITHOUT_MOUSE_MOVED,
    start_mouse_event_tap, stop_mouse_event_tap,
};
use crate::space::manager::SpaceManager;
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{ProcessId, SpaceId, WindowId};
use crate::window::manager::{
    FocusFollowsMouseMode, WindowManager, space_managing_window,
    tracked_application_with_process_id, tracked_window_with_id,
};
use crate::window::model::{
    WindowRuleFlag, is_window_rule_flag_set, query_display_holding_window,
    query_space_holding_window,
};
use crate::window::opacity::set_window_opacity_unless_disabled_or_fixed_by_rule;

#[allow(non_upper_case_globals)]
pub(crate) const kCPSUserGenerated: u32 = 0x200;
#[allow(non_upper_case_globals)]
pub(crate) const kCPSNoWindows: u32 = 0x400;

pub(crate) fn set_focus_follows_mouse_mode(
    window_manager: &mut WindowManager,
    mode: FocusFollowsMouseMode,
) {
    stop_mouse_event_tap();

    if mode == FocusFollowsMouseMode::Disabled {
        start_mouse_event_tap(MOUSE_EVENT_MASK_WITHOUT_MOUSE_MOVED);
    } else {
        start_mouse_event_tap(MOUSE_EVENT_MASK_FOR_FOCUS_FOLLOWS_MOUSE);
    }

    window_manager.focus_follows_mouse_mode = mode;
}

pub(crate) fn warp_cursor_to_window_center_if_mouse_follows_focus(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) {
    let Some(window) = window_manager.window.get(&window_id) else {
        return;
    };

    if is_window_rule_flag_set(window, WindowRuleFlag::OVERRIDES_MOUSE_FOLLOWS_FOCUS) {
        if !is_window_rule_flag_set(window, WindowRuleFlag::MOUSE_FOLLOWS_FOCUS_OVERRIDE_IS_ON) {
            return;
        }
    } else {
        if !window_manager.enable_mff {
            return;
        }
    }

    let window_frame = window.frame;

    let mut cursor = CGPoint::new(0.0, 0.0);
    unsafe {
        SLSGetCurrentCursorLocation(*SKYLIGHT_CONNECTION_ID.get().unwrap_or(&0), &mut cursor)
    };
    if CGRectContainsPoint(window_frame, cursor) {
        return;
    }

    let display_id = query_display_holding_window(window_id);
    if display_id.0 == 0 {
        return;
    }

    let center = CGPoint::new(
        window_frame.origin.x + window_frame.size.width / 2.0,
        window_frame.origin.y + window_frame.size.height / 2.0,
    );

    let bounds = CGDisplayBounds(display_id.0);
    if !CGRectContainsPoint(bounds, center) {
        return;
    }

    CGWarpMouseCursorPosition(center);
}

pub(crate) fn make_window_key_with_synthesized_events(
    window_process_serial_number: &ProcessSerialNumber,
    window_id: WindowId,
) {
    //
    // :SynthesizedEvent
    //
    // NOTE(asmvik): These events will be picked up by an event-tap
    // registered at the "Annotated Session" location; specifying that an
    // event-tap is placed at the point where session events have been
    // annotated to flow to an application.
    //

    let mut window_process_serial_number = *window_process_serial_number;
    let mut event_bytes = [0u8; 0x100];

    event_bytes[..0xf8].fill(0);
    event_bytes[0x04] = 0xf8;
    event_bytes[0x3a] = 0x10;
    event_bytes[0x3c..0x40].copy_from_slice(&window_id.0.to_ne_bytes());
    event_bytes[0x20..0x30].fill(0xff);

    event_bytes[0x08] = 0x01;
    unsafe { SLPSPostEventRecordTo(&mut window_process_serial_number, event_bytes.as_mut_ptr()) };

    event_bytes[0x08] = 0x02;
    unsafe { SLPSPostEventRecordTo(&mut window_process_serial_number, event_bytes.as_mut_ptr()) };
}

pub(crate) fn focus_window_of_process_without_raising_it(
    window_process_serial_number: &ProcessSerialNumber,
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    if is_same_process_serial_number(
        window_process_serial_number,
        &window_manager.focused_window_process_serial_number,
    ) {
        let mut event_bytes = [0u8; 0x100];

        event_bytes[..0xf8].fill(0);
        event_bytes[0x04] = 0xf8;
        event_bytes[0x08] = 0x0d;

        event_bytes[0x8a] = 0x02;
        event_bytes[0x3c..0x40].copy_from_slice(&window_manager.focused_window_id.0.to_ne_bytes());
        unsafe {
            SLPSPostEventRecordTo(
                &mut window_manager.focused_window_process_serial_number,
                event_bytes.as_mut_ptr(),
            )
        };

        //
        // @hack
        // Artificially delay the activation by 40ms. This is necessary
        // because some applications appear to be confused if both of
        // the events appear instantaneously.
        //

        unsafe { libc::usleep(40000) };

        let mut target_process_serial_number = *window_process_serial_number;
        event_bytes[0x8a] = 0x01;
        event_bytes[0x3c..0x40].copy_from_slice(&window_id.0.to_ne_bytes());
        unsafe {
            SLPSPostEventRecordTo(&mut target_process_serial_number, event_bytes.as_mut_ptr())
        };
    }

    let mut target_process_serial_number = *window_process_serial_number;
    unsafe {
        _SLPSSetFrontProcessWithOptions(
            &mut target_process_serial_number,
            window_id.0,
            kCPSUserGenerated,
        )
    };
    make_window_key_with_synthesized_events(window_process_serial_number, window_id);
}

pub(crate) fn focus_and_raise_window_of_process(
    window_process_serial_number: &ProcessSerialNumber,
    window_id: WindowId,
    window_ref: AXUIElementRef,
) {
    let mut target_process_serial_number = *window_process_serial_number;
    unsafe {
        _SLPSSetFrontProcessWithOptions(
            &mut target_process_serial_number,
            window_id.0,
            kCPSUserGenerated,
        )
    };
    make_window_key_with_synthesized_events(window_process_serial_number, window_id);
    if let Some(window_element) = unsafe { window_ref.as_ref() } {
        unsafe { AXUIElementPerformAction(window_element, kAXRaiseAction()) };
    }
}

pub(crate) fn focus_and_raise_tracked_window(window_manager: &WindowManager, window_id: WindowId) {
    let Some(window) = window_manager.window.get(&window_id) else {
        return;
    };
    let Some(application) = window
        .application
        .and_then(|application_process_id| window_manager.application.get(&application_process_id))
    else {
        return;
    };

    focus_and_raise_window_of_process(
        &application.process_serial_number,
        window.id,
        window.element_ref,
    );
}

pub(crate) fn query_focused_tracked_application(
    window_manager: &mut WindowManager,
) -> Option<ProcessId> {
    let mut process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };
    unsafe { _SLPSGetFrontProcess(&mut process_serial_number) };

    let mut process_id: libc::pid_t = 0;
    unsafe { GetProcessPID(&process_serial_number, &mut process_id) };

    tracked_application_with_process_id(window_manager, ProcessId(process_id))
}

pub(crate) fn query_focused_tracked_window(window_manager: &mut WindowManager) -> Option<WindowId> {
    let Some(application_process_id) = query_focused_tracked_application(window_manager) else {
        return None;
    };
    let Some(application) = window_manager.application.get(&application_process_id) else {
        return None;
    };

    let window_id = read_focused_window_of_application(application);
    tracked_window_with_id(window_manager, window_id)
}

pub(crate) fn toggle_application_expose_for_window(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    focus_and_raise_tracked_window(window_manager, window_id);
    unsafe { CoreDockSendNotification(show_front_windows_dock_notification_name(), 0) };
}

pub(crate) fn respond_to_window_receiving_focus(
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    clear_every_pending_insertion_point_other_than_the_window(
        window_id,
        window_manager,
        space_manager,
        mouse_drag_state,
    );

    let focused_window = tracked_window_with_id(window_manager, window_manager.focused_window_id);
    if let Some(focused_window) = focused_window {
        if focused_window != window_id
            && query_space_holding_window(focused_window) == query_space_holding_window(window_id)
        {
            let normal_window_opacity = window_manager.normal_window_opacity;
            set_window_opacity_unless_disabled_or_fixed_by_rule(
                window_manager,
                focused_window,
                normal_window_opacity,
            );
        }
    }

    let active_window_opacity = window_manager.active_window_opacity;
    set_window_opacity_unless_disabled_or_fixed_by_rule(
        window_manager,
        window_id,
        active_window_opacity,
    );

    if window_manager.focused_window_id != window_id {
        if mouse_drag_state.ffm_window_id != window_id {
            warp_cursor_to_window_center_if_mouse_follows_focus(window_manager, window_id);
        }

        window_manager.last_window_id = window_manager.focused_window_id;
    }

    window_manager.focused_window_id = window_id;
    let application_process_serial_number = window_manager
        .window
        .get(&window_id)
        .and_then(|window| window.application)
        .and_then(|application_process_id| window_manager.application.get(&application_process_id))
        .map(|application| application.process_serial_number);
    if let Some(application_process_serial_number) = application_process_serial_number {
        window_manager.focused_window_process_serial_number = application_process_serial_number;
    }
    mouse_drag_state.ffm_window_id = WindowId(0);

    let Some(space_id) = space_managing_window(window_manager, window_id) else {
        return;
    };

    bring_window_to_the_front_of_its_stack(space_id, window_id, space_manager);
    refresh_the_group_headers_of_view(space_id, space_manager, window_manager);
}

fn bring_window_to_the_front_of_its_stack(
    space_id: SpaceId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let Some(node_id) = leaf_holding_window(space_manager, space_id, window_id) else {
        return;
    };
    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);
    if node.window_count <= 1 {
        return;
    }

    for index in 0..node.window_count {
        if node.window_order[index as usize] != window_id {
            continue;
        }

        node.window_order.copy_within(0..index as usize, 1);
        node.window_order[0] = window_id;

        break;
    }
}
