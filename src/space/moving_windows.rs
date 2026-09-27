use core::ffi::c_void;

use objc2::ffi::objc_msgSend;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject, Sel};

use crate::ffi::core_foundation::{CFRetained, cfarray_of_cfnumbers, kCFNumberSInt32Type};
use crate::ffi::skylight::{
    SLSMoveWindowsToManagedSpace, SLSSetWindowListWorkspace, SLSSpaceSetCompatID,
};
use crate::ffi::skylight_dynamic::sls_perform_asynchronous_bridged_window_management_operation;
use crate::scripting_addition::client::{
    scripting_addition_move_window_list_to_space, scripting_addition_move_window_to_space,
};
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{SpaceId, WindowId};
use crate::support::macos_version::workspace_use_macos_space_workaround;

type InitWithWindowsSpaceIdFn =
    unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, u64) -> *mut AnyObject;

pub(crate) fn space_manager_move_window_list_to_space(space_id: SpaceId, window_list: &[WindowId]) {
    let connection_id = *CONNECTION.get().unwrap();
    let mut window_id_list: Vec<u32> = window_list.iter().map(|window_id| window_id.0).collect();

    if let Some(perform_operation) = sls_perform_asynchronous_bridged_window_management_operation()
    {
        let window_list_ref = cfarray_of_cfnumbers(&window_id_list, kCFNumberSInt32Type);
        let Some(class) = AnyClass::get(c"SLSBridgedMoveWindowsToManagedSpaceOperation") else {
            return;
        };
        let selector = Sel::register(c"initWithWindows:spaceID:");
        let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
        let init_with_windows_space_id: InitWithWindowsSpaceIdFn = unsafe {
            core::mem::transmute::<*const c_void, InitWithWindowsSpaceIdFn>(
                objc_msgSend as *const c_void,
            )
        };
        let operation = unsafe {
            init_with_windows_space_id(
                allocated,
                selector,
                CFRetained::as_ptr(&window_list_ref)
                    .as_ptr()
                    .cast::<AnyObject>(),
                space_id.0,
            )
        };
        unsafe { perform_operation(operation.cast::<c_void>()) };
        let _: () = unsafe { msg_send![operation, release] };
        drop(window_list_ref);
    } else if !workspace_use_macos_space_workaround() {
        let window_list_ref = cfarray_of_cfnumbers(&window_id_list, kCFNumberSInt32Type);
        unsafe { SLSMoveWindowsToManagedSpace(connection_id, &*window_list_ref, space_id.0) };
        drop(window_list_ref);
    } else if !scripting_addition_move_window_list_to_space(space_id, window_list) {
        unsafe { SLSSpaceSetCompatID(connection_id, space_id.0, 0x79616265) };
        unsafe {
            SLSSetWindowListWorkspace(
                connection_id,
                window_id_list.as_mut_ptr(),
                window_id_list.len() as i32,
                0x79616265,
            )
        };
        unsafe { SLSSpaceSetCompatID(connection_id, space_id.0, 0x0) };
    }
}

pub(crate) fn space_manager_move_window_to_space(space_id: SpaceId, window_id: WindowId) {
    let connection_id = *CONNECTION.get().unwrap();
    let mut window_id_value: u32 = window_id.0;

    if let Some(perform_operation) = sls_perform_asynchronous_bridged_window_management_operation()
    {
        let window_list_ref = cfarray_of_cfnumbers(&[window_id_value], kCFNumberSInt32Type);
        let Some(class) = AnyClass::get(c"SLSBridgedMoveWindowsToManagedSpaceOperation") else {
            return;
        };
        let selector = Sel::register(c"initWithWindows:spaceID:");
        let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
        let init_with_windows_space_id: InitWithWindowsSpaceIdFn = unsafe {
            core::mem::transmute::<*const c_void, InitWithWindowsSpaceIdFn>(
                objc_msgSend as *const c_void,
            )
        };
        let operation = unsafe {
            init_with_windows_space_id(
                allocated,
                selector,
                CFRetained::as_ptr(&window_list_ref)
                    .as_ptr()
                    .cast::<AnyObject>(),
                space_id.0,
            )
        };
        unsafe { perform_operation(operation.cast::<c_void>()) };
        let _: () = unsafe { msg_send![operation, release] };
        drop(window_list_ref);
    } else if !workspace_use_macos_space_workaround() {
        let window_list_ref = cfarray_of_cfnumbers(&[window_id_value], kCFNumberSInt32Type);
        unsafe { SLSMoveWindowsToManagedSpace(connection_id, &*window_list_ref, space_id.0) };
        drop(window_list_ref);
    } else if !scripting_addition_move_window_to_space(space_id, window_id) {
        unsafe { SLSSpaceSetCompatID(connection_id, space_id.0, 0x79616265) };
        unsafe { SLSSetWindowListWorkspace(connection_id, &mut window_id_value, 1, 0x79616265) };
        unsafe { SLSSpaceSetCompatID(connection_id, space_id.0, 0x0) };
    }
}
