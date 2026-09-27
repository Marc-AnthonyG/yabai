#![allow(deprecated)]

use crate::application::model::application_focused_window;
use crate::ffi::accessibility::{AXUIElementPerformAction, AXUIElementRef, kAXRaiseAction};
use crate::ffi::carbon_process::{
    CoreDockSendNotification, GetProcessPID, ProcessSerialNumber, psn_equals,
};
use crate::ffi::core_foundation::{CGPoint, k_com_apple_expose_front_awake};
use crate::ffi::core_graphics::{CGDisplayBounds, CGRectContainsPoint, CGWarpMouseCursorPosition};
use crate::ffi::skylight::{
    _SLPSGetFrontProcess, _SLPSSetFrontProcessWithOptions, SLPSPostEventRecordTo,
    SLSGetCurrentCursorLocation,
};
use crate::layout::insertion::clear_every_pending_insertion_point_other_than_the_window;
use crate::layout::tree::view_find_window_node;
use crate::mouse::drag::MouseDragState;
use crate::notifications::mouse::{
    MOUSE_EVENT_MASK, MOUSE_EVENT_MASK_FFM, mouse_handler_begin, mouse_handler_end,
};
use crate::space::manager::SpaceManager;
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{ProcessId, WindowId};
use crate::window::manager::{
    FfmMode, WindowManager, window_manager_find_application, window_manager_find_managed_window,
    window_manager_find_window,
};
use crate::window::model::{
    WindowRuleFlag, window_check_rule_flag, window_display_id, window_space,
};
use crate::window::opacity::window_manager_set_window_opacity;

#[allow(non_upper_case_globals)]
pub(crate) const kCPSUserGenerated: u32 = 0x200;
#[allow(non_upper_case_globals)]
pub(crate) const kCPSNoWindows: u32 = 0x400;

pub(crate) fn window_manager_set_focus_follows_mouse(
    window_manager: &mut WindowManager,
    mode: FfmMode,
) {
    mouse_handler_end();

    if mode == FfmMode::Disabled {
        mouse_handler_begin(MOUSE_EVENT_MASK);
    } else {
        mouse_handler_begin(MOUSE_EVENT_MASK_FFM);
    }

    window_manager.ffm_mode = mode;
}

pub(crate) fn window_manager_center_mouse(window_manager: &mut WindowManager, window_id: WindowId) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };

    if window_check_rule_flag(window, WindowRuleFlag::MFF) {
        if !window_check_rule_flag(window, WindowRuleFlag::MFF_VALUE) {
            return;
        }
    } else {
        if !window_manager.enable_mff {
            return;
        }
    }

    let window_frame = window.frame;

    let mut cursor = CGPoint::new(0.0, 0.0);
    unsafe { SLSGetCurrentCursorLocation(*CONNECTION.get().unwrap_or(&0), &mut cursor) };
    if CGRectContainsPoint(window_frame, cursor) {
        return;
    }

    let display_id = window_display_id(window_id);
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

pub(crate) fn window_manager_make_key_window(
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

pub(crate) fn window_manager_focus_window_without_raise(
    window_process_serial_number: &ProcessSerialNumber,
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    if psn_equals(
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
    window_manager_make_key_window(window_process_serial_number, window_id);
}

pub(crate) fn window_manager_focus_window_with_raise(
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
    window_manager_make_key_window(window_process_serial_number, window_id);
    if let Some(window_element) = unsafe { window_ref.as_ref() } {
        unsafe { AXUIElementPerformAction(window_element, kAXRaiseAction()) };
    }
}

pub(crate) fn window_manager_focus_window_with_raise_resolving_its_application(
    window_manager: &WindowManager,
    window_id: WindowId,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    let Some(application) = window.application.and_then(|application_process_id| {
        window_manager.application.find(&application_process_id)
    }) else {
        return;
    };

    window_manager_focus_window_with_raise(
        &application.process_serial_number,
        window.id,
        window.element_ref,
    );
}

pub(crate) fn window_manager_focused_application(
    window_manager: &mut WindowManager,
) -> Option<ProcessId> {
    let mut process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };
    unsafe { _SLPSGetFrontProcess(&mut process_serial_number) };

    let mut process_id: libc::pid_t = 0;
    unsafe { GetProcessPID(&process_serial_number, &mut process_id) };

    window_manager_find_application(window_manager, ProcessId(process_id))
}

pub(crate) fn window_manager_focused_window(
    window_manager: &mut WindowManager,
) -> Option<WindowId> {
    let Some(application_process_id) = window_manager_focused_application(window_manager) else {
        return None;
    };
    let Some(application) = window_manager.application.find(&application_process_id) else {
        return None;
    };

    let window_id = application_focused_window(application);
    window_manager_find_window(window_manager, window_id)
}

pub(crate) fn window_manager_toggle_window_expose(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    window_manager_focus_window_with_raise_resolving_its_application(window_manager, window_id);
    unsafe { CoreDockSendNotification(k_com_apple_expose_front_awake(), 0) };
}

pub(crate) fn window_did_receive_focus(
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

    let focused_window =
        window_manager_find_window(window_manager, window_manager.focused_window_id);
    if let Some(focused_window) = focused_window {
        if focused_window != window_id && window_space(focused_window) == window_space(window_id) {
            let normal_window_opacity = window_manager.normal_window_opacity;
            window_manager_set_window_opacity(
                window_manager,
                focused_window,
                normal_window_opacity,
            );
        }
    }

    let active_window_opacity = window_manager.active_window_opacity;
    window_manager_set_window_opacity(window_manager, window_id, active_window_opacity);

    if window_manager.focused_window_id != window_id {
        if mouse_drag_state.ffm_window_id != window_id {
            window_manager_center_mouse(window_manager, window_id);
        }

        window_manager.last_window_id = window_manager.focused_window_id;
    }

    window_manager.focused_window_id = window_id;
    let application_process_serial_number = window_manager
        .window
        .find(&window_id)
        .and_then(|window| window.application)
        .and_then(|application_process_id| window_manager.application.find(&application_process_id))
        .map(|application| application.process_serial_number);
    if let Some(application_process_serial_number) = application_process_serial_number {
        window_manager.focused_window_process_serial_number = application_process_serial_number;
    }
    mouse_drag_state.ffm_window_id = WindowId(0);

    let Some(view) = window_manager_find_managed_window(window_manager, window_id) else {
        return;
    };

    let Some(node_id) = view_find_window_node(space_manager, view, window_id) else {
        return;
    };
    let Some(view) = space_manager.view.find_mut(&view) else {
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
