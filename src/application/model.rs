#![allow(deprecated)]

use core::ptr::NonNull;
use std::sync::Arc;

use crate::ffi::accessibility::{
    AXObserverRef, AXUIElement, AXUIElementCopyAttributeValue, AXUIElementCreateApplication,
    AXUIElementRef, kAXFocusedWindowAttribute, kAXWindowsAttribute,
    read_window_id_of_accessibility_element,
};
use crate::ffi::carbon_process::{
    IsProcessVisible, ProcessSerialNumber, is_same_process_serial_number,
};
use crate::ffi::core_foundation::{CFArray, CFRetained, CFType, take_create_rule_result};
use crate::ffi::skylight::{_SLPSGetFrontProcess, SLSGetConnectionIDForPSN};
use crate::process::model::Process;
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
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

pub(crate) fn read_focused_window_of_application(application: &Application) -> WindowId {
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

    let window_id = read_window_id_of_accessibility_element(unsafe {
        &*((&*window_ref as *const CFType).cast::<AXUIElement>())
    });
    drop(window_ref);

    WindowId(window_id)
}

pub(crate) fn is_application_frontmost(application: &Application) -> bool {
    let mut process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };
    unsafe { _SLPSGetFrontProcess(&mut process_serial_number) };
    is_same_process_serial_number(&process_serial_number, &application.process_serial_number)
}

pub(crate) fn is_application_hidden(application: &Application) -> bool {
    (unsafe { IsProcessVisible(&application.process_serial_number) }) == 0
}

pub(crate) fn copy_accessibility_windows_of_application(
    application: &Application,
) -> Option<CFRetained<CFArray>> {
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

pub(crate) fn create_application_for_process(process: &Arc<Process>) -> Application {
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

    application.is_hidden = is_application_hidden(&application);
    unsafe {
        SLSGetConnectionIDForPSN(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            &mut application.process_serial_number,
            &mut application.connection,
        )
    };

    application
}

pub(crate) fn destroy_application_releasing_its_accessibility_element(application: Application) {
    drop(application);
}
