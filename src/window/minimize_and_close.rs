#![allow(deprecated)]

use core::ptr::NonNull;

use crate::ffi::accessibility::{
    AXUIElement, AXUIElementCopyAttributeValue, AXUIElementPerformAction,
    AXUIElementSetAttributeValue, kAXCloseButtonAttribute, kAXErrorSuccess, kAXMinimizedAttribute,
    kAXPressAction,
};
use crate::ffi::core_foundation::{
    CFType, as_cftype, kCFBooleanFalse, kCFBooleanTrue, take_create_rule_result,
};
use crate::support::handles::WindowId;
use crate::window::manager::{WindowManager, WindowOperationOutcome};
use crate::window::model::{WindowFlag, can_window_be_minimized_through_accessibility};

pub(crate) fn minimize_window_through_accessibility(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> WindowOperationOutcome {
    let Some(window) = window_manager.window.get(&window_id) else {
        return WindowOperationOutcome::CannotMinimize;
    };

    if !can_window_be_minimized_through_accessibility(window) {
        return WindowOperationOutcome::CannotMinimize;
    }
    if window.flags.contains(WindowFlag::MINIMIZED) {
        return WindowOperationOutcome::AlreadyMinimized;
    }

    let result = unsafe {
        AXUIElementSetAttributeValue(
            &*window.element_ref,
            kAXMinimizedAttribute(),
            as_cftype(kCFBooleanTrue()),
        )
    };
    if result == kAXErrorSuccess {
        WindowOperationOutcome::Success
    } else {
        WindowOperationOutcome::MinimizeFailed
    }
}

pub(crate) fn deminimize_window_through_accessibility(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> WindowOperationOutcome {
    let Some(window) = window_manager.window.get(&window_id) else {
        return WindowOperationOutcome::NotMinimized;
    };

    if !window.flags.contains(WindowFlag::MINIMIZED) {
        return WindowOperationOutcome::NotMinimized;
    }

    let result = unsafe {
        AXUIElementSetAttributeValue(
            &*window.element_ref,
            kAXMinimizedAttribute(),
            as_cftype(kCFBooleanFalse()),
        )
    };
    if result == kAXErrorSuccess {
        WindowOperationOutcome::Success
    } else {
        WindowOperationOutcome::DeminimizeFailed
    }
}

pub(crate) fn close_window_by_pressing_its_close_button(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(window) = window_manager.window.get(&window_id) else {
        return false;
    };

    let mut button: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXCloseButtonAttribute(),
            NonNull::from(&mut button),
        )
    };
    let Some(button) = (unsafe { take_create_rule_result(button) }) else {
        return false;
    };

    unsafe {
        AXUIElementPerformAction(
            &*(&*button as *const CFType).cast::<AXUIElement>(),
            kAXPressAction(),
        )
    };
    drop(button);

    true
}
