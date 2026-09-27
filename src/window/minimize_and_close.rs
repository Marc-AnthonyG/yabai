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
use crate::handles::WindowId;
use crate::window::manager::{WindowManager, WindowOpError};
use crate::window::model::{WindowFlag, window_can_minimize, window_check_flag};

pub(crate) fn window_manager_minimize_window(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> WindowOpError {
    let Some(window) = window_manager.window.find(&window_id) else {
        return WindowOpError::CantMinimize;
    };

    if !window_can_minimize(window) {
        return WindowOpError::CantMinimize;
    }
    if window_check_flag(window, WindowFlag::MINIMIZE) {
        return WindowOpError::AlreadyMinimized;
    }

    let result = unsafe {
        AXUIElementSetAttributeValue(
            &*window.element_ref,
            kAXMinimizedAttribute(),
            as_cftype(kCFBooleanTrue()),
        )
    };
    if result == kAXErrorSuccess {
        WindowOpError::Success
    } else {
        WindowOpError::MinimizeFailed
    }
}

pub(crate) fn window_manager_deminimize_window(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> WindowOpError {
    let Some(window) = window_manager.window.find(&window_id) else {
        return WindowOpError::NotMinimized;
    };

    if !window_check_flag(window, WindowFlag::MINIMIZE) {
        return WindowOpError::NotMinimized;
    }

    let result = unsafe {
        AXUIElementSetAttributeValue(
            &*window.element_ref,
            kAXMinimizedAttribute(),
            as_cftype(kCFBooleanFalse()),
        )
    };
    if result == kAXErrorSuccess {
        WindowOpError::Success
    } else {
        WindowOpError::DeminimizeFailed
    }
}

pub(crate) fn window_manager_close_window(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(window) = window_manager.window.find(&window_id) else {
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
