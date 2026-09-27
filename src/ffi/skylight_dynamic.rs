use core::ffi::{CStr, c_int, c_void};
use libc::mach_port_t;
use std::sync::OnceLock;

use crate::ffi::macho::macho_find_symbol;

pub type CGSGetConnectionPortByIdFn = unsafe extern "C" fn(c_int) -> mach_port_t;
pub type SLSPerformAsynchronousBridgedWindowManagementOperationFn =
    unsafe extern "C" fn(*mut c_void) -> i64;

const SKYLIGHT_IMAGE_PATH: &CStr =
    c"/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight";
const CGS_GET_CONNECTION_PORT_BY_ID_SYMBOL: &CStr = c"_CGSGetConnectionPortById";
const SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION_SYMBOL: &CStr =
    c"__ZL54SLSPerformAsynchronousBridgedWindowManagementOperationP47SLSAsynchronousBridgedWindowManagementOperation";

static CGS_GET_CONNECTION_PORT_BY_ID: OnceLock<Option<CGSGetConnectionPortByIdFn>> =
    OnceLock::new();
static SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION: OnceLock<
    Option<SLSPerformAsynchronousBridgedWindowManagementOperationFn>,
> = OnceLock::new();

pub fn resolve_dynamic_skylight_symbols() {
    let connection_port_address =
        unsafe { macho_find_symbol(SKYLIGHT_IMAGE_PATH, CGS_GET_CONNECTION_PORT_BY_ID_SYMBOL) };
    let bridged_operation_address = unsafe {
        macho_find_symbol(
            SKYLIGHT_IMAGE_PATH,
            SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION_SYMBOL,
        )
    };

    let _ = CGS_GET_CONNECTION_PORT_BY_ID.set(connection_port_address.map(|address| unsafe {
        core::mem::transmute::<*mut c_void, CGSGetConnectionPortByIdFn>(address)
    }));
    let _ = SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION.set(
        bridged_operation_address.map(|address| unsafe {
            core::mem::transmute::<
                *mut c_void,
                SLSPerformAsynchronousBridgedWindowManagementOperationFn,
            >(address)
        }),
    );
}

pub fn cgs_get_connection_port_by_id() -> Option<CGSGetConnectionPortByIdFn> {
    *CGS_GET_CONNECTION_PORT_BY_ID.get().unwrap_or(&None)
}

pub fn sls_perform_asynchronous_bridged_window_management_operation()
-> Option<SLSPerformAsynchronousBridgedWindowManagementOperationFn> {
    *SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION
        .get()
        .unwrap_or(&None)
}
