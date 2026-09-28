use core::ffi::{c_int, c_void};
use libc::pid_t;
use objc2_core_foundation::{
    CFArray, CFDictionary, CFString, CFType, CGAffineTransform, CGPoint, CGRect,
};
use objc2_core_graphics::{CGContext, CGError};

use crate::ffi::carbon_process::ProcessSerialNumber;

pub type SkylightConnectionNotificationCallback = unsafe extern "C-unwind" fn(
    notification_type: u32,
    data: *mut c_void,
    data_length: usize,
    context: *mut c_void,
    connection_id: c_int,
);

#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    pub fn SLSMainConnectionID() -> c_int;
    pub fn SLSNewConnection(zero: c_int, connection_id: *mut c_int) -> CGError;
    pub fn SLSReleaseConnection(connection_id: c_int) -> CGError;
    pub fn SLSRegisterConnectionNotifyProc(
        connection_id: c_int,
        handler: SkylightConnectionNotificationCallback,
        event: u32,
        context: *mut c_void,
    ) -> CGError;
    pub fn SLSGetWindowBounds(connection_id: c_int, window_id: u32, frame: *mut CGRect) -> CGError;
    pub fn SLSGetWindowLevel(connection_id: c_int, window_id: u32, level: *mut c_int) -> CGError;
    pub fn SLSGetWindowSubLevel(connection_id: c_int, window_id: u32) -> c_int;
    pub fn SLSGetWindowAlpha(connection_id: c_int, window_id: u32, alpha: *mut f32) -> CGError;
    pub fn SLSSetWindowAlpha(connection_id: c_int, window_id: u32, alpha: f32) -> CGError;
    pub fn SLSSetWindowResolution(connection_id: c_int, window_id: u32, resolution: f64)
    -> CGError;
    pub fn SLSCopyWindowProperty(
        connection_id: c_int,
        window_id: u32,
        property: *const CFString,
        value: *mut *mut CFType,
    ) -> CGError;
    pub fn SLSCopyManagedDisplayForWindow(connection_id: c_int, window_id: u32) -> *mut CFString;
    pub fn SLSCopyBestManagedDisplayForRect(connection_id: c_int, rect: CGRect) -> *mut CFString;
    pub fn SLSCopySpacesForWindows(
        connection_id: c_int,
        selector: c_int,
        window_list: *const CFArray,
    ) -> *mut CFArray;
    pub fn SLSDisableUpdate(connection_id: c_int) -> CGError;
    pub fn SLSReenableUpdate(connection_id: c_int) -> CGError;
    pub fn SLSNewWindowWithOpaqueShapeAndContext(
        connection_id: c_int,
        type_of_window: c_int,
        region: *const CFType,
        opaque_shape: *const CFType,
        options: c_int,
        tags: *mut u64,
        x: f32,
        y: f32,
        tag_size: c_int,
        window_id: *mut u32,
        context: *mut c_void,
    ) -> CGError;
    pub fn SLSReleaseWindow(connection_id: c_int, window_id: u32) -> CGError;
    pub fn SLSSetWindowShape(
        connection_id: c_int,
        window_id: u32,
        x_offset: f32,
        y_offset: f32,
        shape: *const CFType,
    ) -> CGError;
    pub fn SLSSetWindowOpacity(connection_id: c_int, window_id: u32, opaque: bool) -> CGError;
    pub fn SLSOrderWindow(
        connection_id: c_int,
        window_id: u32,
        mode: c_int,
        relative_window_id: u32,
    ) -> CGError;
    pub fn SLSWindowIsOrderedIn(connection_id: c_int, window_id: u32, value: *mut u8) -> CGError;
    pub fn SLSSetWindowLevel(connection_id: c_int, window_id: u32, level: c_int) -> CGError;
    pub fn SLSSetWindowSubLevel(connection_id: c_int, window_id: u32, sub_level: c_int) -> CGError;
    pub fn SLWindowContextCreate(
        connection_id: c_int,
        window_id: u32,
        options: *const CFDictionary,
    ) -> *mut CGContext;
    pub fn SLSCopyManagedDisplays(connection_id: c_int) -> *mut CFArray;
    pub fn SLSManagedDisplayGetCurrentSpace(connection_id: c_int, uuid: *const CFString) -> u64;
    pub fn SLSCopyActiveMenuBarDisplayIdentifier(connection_id: c_int) -> *mut CFString;
    pub fn SLSSetActiveMenuBarDisplayIdentifier(
        connection_id: c_int,
        uuid: *const CFString,
        repeat_uuid: *const CFString,
    ) -> CGError;
    pub fn SLSCopyBestManagedDisplayForPoint(connection_id: c_int, point: CGPoint)
    -> *mut CFString;
    pub fn SLSManagedDisplayIsAnimating(connection_id: c_int, uuid: *const CFString) -> bool;
    pub fn SLSSetMenuBarInsetAndAlpha(
        connection_id: c_int,
        unused1: f64,
        unused2: f64,
        alpha: f32,
    ) -> CGError;
    pub fn SLSGetMenuBarAutohideEnabled(connection_id: c_int, enabled: *mut c_int) -> CGError;
    #[cfg(target_arch = "x86_64")]
    pub fn SLSGetRevealedMenuBarBounds(
        rect: *mut CGRect,
        connection_id: c_int,
        space_id: u64,
    ) -> CGError;
    #[cfg(target_arch = "aarch64")]
    pub fn SLSGetDisplayMenubarHeight(display_id: u32, height: *mut u32) -> CGError;
    pub fn SLSGetDockRectWithReason(
        connection_id: c_int,
        rect: *mut CGRect,
        reason: *mut c_int,
    ) -> CGError;
    pub fn SLSCopyManagedDisplayForSpace(connection_id: c_int, space_id: u64) -> *mut CFString;
    pub fn SLSSpaceSetFrontPSN(
        connection_id: c_int,
        space_id: u64,
        process_serial_number: ProcessSerialNumber,
    ) -> CGError;
    pub fn SLSSpaceGetType(connection_id: c_int, space_id: u64) -> c_int;
    pub fn SLSSpaceCopyName(connection_id: c_int, space_id: u64) -> *mut CFString;
    pub fn SLSCopyWindowsWithOptionsAndTags(
        connection_id: c_int,
        owner: u32,
        spaces: *const CFArray,
        options: u32,
        set_tags: *mut u64,
        clear_tags: *mut u64,
    ) -> *mut CFArray;
    pub fn SLSGetSpaceManagementMode(connection_id: c_int) -> c_int;
    pub fn SLSCopyManagedDisplaySpaces(connection_id: c_int) -> *mut CFArray;
    pub fn SLSMoveWindowsToManagedSpace(
        connection_id: c_int,
        window_list: *const CFArray,
        space_id: u64,
    );
    pub fn SLSCopyAssociatedWindows(connection_id: c_int, window_id: u32) -> *mut CFArray;
    pub fn SLSWindowQueryWindows(
        connection_id: c_int,
        windows: *const CFArray,
        count: c_int,
    ) -> *mut CFType;
    pub fn SLSWindowQueryResultCopyWindows(window_query: *const CFType) -> *mut CFType;
    pub fn SLSWindowIteratorGetCount(iterator: *const CFType) -> c_int;
    pub fn SLSWindowIteratorAdvance(iterator: *const CFType) -> bool;
    pub fn SLSWindowIteratorGetParentID(iterator: *const CFType) -> u32;
    pub fn SLSWindowIteratorGetWindowID(iterator: *const CFType) -> u32;
    pub fn SLSWindowIteratorGetTags(iterator: *const CFType) -> u64;
    pub fn SLSWindowIteratorGetAttributes(iterator: *const CFType) -> u64;
    pub fn SLSWindowIteratorGetLevel(iterator: *const CFType) -> c_int;
    pub fn _SLPSGetFrontProcess(process_serial_number: *mut ProcessSerialNumber) -> i32;
    pub fn SLSGetWindowOwner(
        connection_id: c_int,
        window_id: u32,
        window_connection_id: *mut c_int,
    ) -> CGError;
    pub fn SLSGetConnectionPSN(
        connection_id: c_int,
        process_serial_number: *mut ProcessSerialNumber,
    ) -> CGError;
    pub fn SLSConnectionGetPID(connection_id: c_int, process_id: *mut pid_t) -> CGError;
    pub fn SLSGetConnectionIDForPSN(
        connection_id: c_int,
        process_serial_number: *mut ProcessSerialNumber,
        process_serial_number_connection_id: *mut c_int,
    ) -> CGError;
    pub fn _SLPSSetFrontProcessWithOptions(
        process_serial_number: *mut ProcessSerialNumber,
        window_id: u32,
        mode: u32,
    ) -> CGError;
    pub fn SLPSPostEventRecordTo(
        process_serial_number: *mut ProcessSerialNumber,
        bytes: *mut u8,
    ) -> CGError;
    pub fn SLSFindWindowAndOwner(
        connection_id: c_int,
        zero: c_int,
        one: c_int,
        zero_again: c_int,
        screen_point: *mut CGPoint,
        window_point: *mut CGPoint,
        window_id: *mut u32,
        window_connection_id: *mut c_int,
    ) -> i32;
    pub fn SLSGetCurrentCursorLocation(connection_id: c_int, point: *mut CGPoint) -> CGError;
    pub fn SLSWindowSetShadowProperties(window_id: u32, options: *const CFDictionary) -> CGError;
    pub fn SLSRequestNotificationsForWindows(
        connection_id: c_int,
        window_list: *mut u32,
        window_count: c_int,
    ) -> CGError;
    pub fn SLSTransactionCreate(connection_id: c_int) -> *mut CFType;
    pub fn SLSTransactionCommit(transaction: *const CFType, synchronous: c_int) -> CGError;
    pub fn SLSTransactionSetWindowTransform(
        transaction: *const CFType,
        window_id: u32,
        unknown: c_int,
        unknown2: c_int,
        transform: CGAffineTransform,
    ) -> CGError;
    pub fn SLSTransactionSetWindowAlpha(
        transaction: *const CFType,
        window_id: u32,
        alpha: f32,
    ) -> CGError;
    pub fn SLSHWCaptureWindowList(
        connection_id: c_int,
        window_list: *mut u32,
        window_count: c_int,
        options: u32,
    ) -> *mut CFArray;
    pub fn SLSSpaceSetCompatID(connection_id: c_int, space_id: u64, workspace: c_int) -> CGError;
    pub fn SLSSetWindowListWorkspace(
        connection_id: c_int,
        window_list: *mut u32,
        window_count: c_int,
        workspace: c_int,
    ) -> CGError;
}
