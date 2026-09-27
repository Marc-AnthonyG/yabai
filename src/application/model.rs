#![allow(deprecated)]

use core::ptr::NonNull;
use std::sync::Arc;

use crate::ffi::accessibility::{
    AXObserverRef, AXUIElement, AXUIElementCopyAttributeValue, AXUIElementCreateApplication,
    AXUIElementRef, ax_window_id, kAXFocusedWindowAttribute, kAXWindowsAttribute,
};
use crate::ffi::carbon_process::{IsProcessVisible, ProcessSerialNumber, psn_equals};
use crate::ffi::core_foundation::{CFArray, CFRetained, CFType, take_create_rule_result};
use crate::ffi::skylight::{_SLPSGetFrontProcess, SLSGetConnectionIDForPSN};
use crate::process::model::Process;
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{ProcessId, WindowId};

pub(crate) struct Application {
    pub(crate) element_ref: AXUIElementRef,
    pub(crate) connection: i32,
    pub(crate) process_serial_number: ProcessSerialNumber,
    pub(crate) process_id: ProcessId,
    pub(crate) name: Arc<str>,
    pub(crate) observer_ref: AXObserverRef,
    pub(crate) notification: u8,
    pub(crate) is_observing: bool,
    pub(crate) is_hidden: bool,
    pub(crate) ax_retry: bool,
}

impl Drop for Application {
    fn drop(&mut self) {
        if let Some(element_ref) = NonNull::new(self.element_ref.cast_mut()) {
            drop(unsafe { CFRetained::from_raw(element_ref) });
        }
    }
}

pub(crate) fn application_focused_window(application: &Application) -> WindowId {
    let mut window_ref: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*application.element_ref,
            kAXFocusedWindowAttribute(),
            NonNull::from(&mut window_ref),
        )
    };
    let Some(window_ref) = (unsafe { take_create_rule_result(window_ref) }) else {
        return WindowId(0);
    };

    let window_id =
        ax_window_id(unsafe { &*((&*window_ref as *const CFType).cast::<AXUIElement>()) });
    drop(window_ref);

    WindowId(window_id)
}

pub(crate) fn application_is_frontmost(application: &Application) -> bool {
    let mut process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };
    unsafe { _SLPSGetFrontProcess(&mut process_serial_number) };
    psn_equals(&process_serial_number, &application.process_serial_number)
}

pub(crate) fn application_is_hidden(application: &Application) -> bool {
    (unsafe { IsProcessVisible(&application.process_serial_number) }) == 0
}

pub(crate) fn application_window_list(application: &Application) -> Option<CFRetained<CFArray>> {
    let mut window_list_ref: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*application.element_ref,
            kAXWindowsAttribute(),
            NonNull::from(&mut window_list_ref),
        )
    };
    unsafe { take_create_rule_result(window_list_ref.cast::<CFArray>()) }
}

pub(crate) fn application_create(process: &Arc<Process>) -> Application {
    let mut application = Application {
        element_ref: CFRetained::into_raw(unsafe {
            AXUIElementCreateApplication(process.process_id.0)
        })
        .as_ptr(),
        connection: 0,
        process_serial_number: process.process_serial_number,
        process_id: process.process_id,
        name: Arc::clone(&process.name),
        observer_ref: core::ptr::null_mut(),
        notification: 0,
        is_observing: false,
        is_hidden: false,
        ax_retry: false,
    };

    application.is_hidden = application_is_hidden(&application);
    unsafe {
        SLSGetConnectionIDForPSN(
            *CONNECTION.get().unwrap(),
            &mut application.process_serial_number,
            &mut application.connection,
        )
    };

    application
}

pub(crate) fn application_destroy(application: Application) {
    drop(application);
}
