#![allow(deprecated)]

use crate::display::bounds::display_manager_dock_rect;
use crate::ffi::CFStringOwned;
use crate::ffi::color_sync::{CGDisplayCreateUUIDFromDisplayID, CGDisplayGetDisplayIDFromUUID};
use crate::ffi::core_foundation::{
    CFString, CFUUIDCreateFromString, CFUUIDCreateString, CGPoint, SendCFRetained,
    take_create_rule_result,
};
use crate::ffi::core_graphics::{CGGetActiveDisplayList, CGMainDisplayID};
use crate::ffi::skylight::{
    SLSCopyActiveMenuBarDisplayIdentifier, SLSCopyBestManagedDisplayForPoint,
    SLSCopyBestManagedDisplayForRect, SLSGetCurrentCursorLocation,
};
use crate::state::process_wide::CONNECTION;
use crate::support::handles::DisplayId;

pub(crate) fn display_uuid(display_id: DisplayId) -> Option<CFStringOwned> {
    let uuid_ref = unsafe { take_create_rule_result(CGDisplayCreateUUIDFromDisplayID(display_id.0)) };
    let Some(uuid_ref) = uuid_ref else {
        return None;
    };

    let uuid_string = CFUUIDCreateString(None, Some(&uuid_ref));

    uuid_string.map(SendCFRetained)
}

pub(crate) fn display_id(uuid: &CFString) -> DisplayId {
    let uuid_ref = CFUUIDCreateFromString(None, Some(uuid));
    let Some(uuid_ref) = uuid_ref else {
        return DisplayId(0);
    };

    let display_id = unsafe { CGDisplayGetDisplayIDFromUUID(&*uuid_ref) };

    DisplayId(display_id)
}

pub(crate) fn display_manager_main_display_id() -> DisplayId {
    DisplayId(CGMainDisplayID())
}

pub(crate) fn display_manager_active_display_uuid() -> Option<CFStringOwned> {
    let connection_id = *CONNECTION.get().unwrap();
    unsafe { take_create_rule_result(SLSCopyActiveMenuBarDisplayIdentifier(connection_id)) }
        .map(SendCFRetained)
}

pub(crate) fn display_manager_active_display_id() -> DisplayId {
    let uuid = display_manager_active_display_uuid();
    debug_assert!(uuid.is_some());

    let Some(uuid) = uuid else {
        return DisplayId(0);
    };

    display_id(uuid.as_ref())
}

pub(crate) fn display_manager_dock_display_uuid() -> Option<CFStringOwned> {
    let connection_id = *CONNECTION.get().unwrap();
    let dock = display_manager_dock_rect();
    unsafe { take_create_rule_result(SLSCopyBestManagedDisplayForRect(connection_id, dock)) }
        .map(SendCFRetained)
}

pub(crate) fn display_manager_dock_display_id() -> DisplayId {
    let Some(uuid) = display_manager_dock_display_uuid() else {
        return DisplayId(0);
    };

    display_id(uuid.as_ref())
}

pub(crate) fn display_manager_point_display_uuid(point: CGPoint) -> Option<CFStringOwned> {
    let connection_id = *CONNECTION.get().unwrap();
    unsafe { take_create_rule_result(SLSCopyBestManagedDisplayForPoint(connection_id, point)) }
        .map(SendCFRetained)
}

pub(crate) fn display_manager_point_display_id(point: CGPoint) -> DisplayId {
    let Some(uuid) = display_manager_point_display_uuid(point) else {
        return DisplayId(0);
    };

    display_id(uuid.as_ref())
}

pub(crate) fn display_manager_cursor_display_id() -> DisplayId {
    let connection_id = *CONNECTION.get().unwrap();
    let mut cursor = CGPoint::ZERO;
    unsafe { SLSGetCurrentCursorLocation(connection_id, &mut cursor) };
    display_manager_point_display_id(cursor)
}

pub(crate) fn display_manager_active_display_count() -> i32 {
    let mut count: u32 = 0;
    unsafe { CGGetActiveDisplayList(0, core::ptr::null_mut(), &mut count) };
    count as i32
}

pub(crate) fn display_manager_active_display_list() -> Vec<DisplayId> {
    let display_count = display_manager_active_display_count();
    let mut result: Vec<u32> = vec![0; display_count as usize];
    let mut count: u32 = 0;
    unsafe { CGGetActiveDisplayList(display_count as u32, result.as_mut_ptr(), &mut count) };
    result.truncate(count as usize);
    result.into_iter().map(DisplayId).collect()
}
