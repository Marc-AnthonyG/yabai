#![allow(deprecated)]

use crate::display::bounds::query_dock_bounds;
use crate::ffi::CFStringOwned;
use crate::ffi::color_sync::{CGDisplayCreateUUIDFromDisplayID, CGDisplayGetDisplayIDFromUUID};
use crate::ffi::core_foundation::{
    CFRetainedAssumedSendAndSync, CFString, CFUUIDCreateFromString, CFUUIDCreateString, CGPoint,
    take_create_rule_result,
};
use crate::ffi::core_graphics::{CGGetActiveDisplayList, CGMainDisplayID};
use crate::ffi::skylight::{
    SLSCopyActiveMenuBarDisplayIdentifier, SLSCopyBestManagedDisplayForPoint,
    SLSCopyBestManagedDisplayForRect, SLSGetCurrentCursorLocation,
};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::DisplayId;

pub(crate) fn copy_uuid_of_display(display_id: DisplayId) -> Option<CFStringOwned> {
    let uuid_ref = unsafe { take_create_rule_result(CGDisplayCreateUUIDFromDisplayID(display_id.0)) };
    let Some(uuid_ref) = uuid_ref else {
        return None;
    };

    let uuid_string = CFUUIDCreateString(None, Some(&uuid_ref));

    uuid_string.map(CFRetainedAssumedSendAndSync)
}

pub(crate) fn query_display_with_uuid(uuid: &CFString) -> DisplayId {
    let uuid_ref = CFUUIDCreateFromString(None, Some(uuid));
    let Some(uuid_ref) = uuid_ref else {
        return DisplayId(0);
    };

    let display_id = unsafe { CGDisplayGetDisplayIDFromUUID(&*uuid_ref) };

    DisplayId(display_id)
}

pub(crate) fn query_main_display() -> DisplayId {
    DisplayId(CGMainDisplayID())
}

pub(crate) fn copy_uuid_of_display_showing_the_active_menu_bar() -> Option<CFStringOwned> {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    unsafe { take_create_rule_result(SLSCopyActiveMenuBarDisplayIdentifier(connection_id)) }
        .map(CFRetainedAssumedSendAndSync)
}

pub(crate) fn query_display_showing_the_active_menu_bar() -> DisplayId {
    let uuid = copy_uuid_of_display_showing_the_active_menu_bar();
    debug_assert!(uuid.is_some());

    let Some(uuid) = uuid else {
        return DisplayId(0);
    };

    query_display_with_uuid(uuid.as_ref())
}

pub(crate) fn copy_uuid_of_display_holding_the_dock() -> Option<CFStringOwned> {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let dock = query_dock_bounds();
    unsafe { take_create_rule_result(SLSCopyBestManagedDisplayForRect(connection_id, dock)) }
        .map(CFRetainedAssumedSendAndSync)
}

pub(crate) fn query_display_holding_the_dock() -> DisplayId {
    let Some(uuid) = copy_uuid_of_display_holding_the_dock() else {
        return DisplayId(0);
    };

    query_display_with_uuid(uuid.as_ref())
}

pub(crate) fn copy_uuid_of_display_at_point(point: CGPoint) -> Option<CFStringOwned> {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    unsafe { take_create_rule_result(SLSCopyBestManagedDisplayForPoint(connection_id, point)) }
        .map(CFRetainedAssumedSendAndSync)
}

pub(crate) fn query_display_at_point(point: CGPoint) -> DisplayId {
    let Some(uuid) = copy_uuid_of_display_at_point(point) else {
        return DisplayId(0);
    };

    query_display_with_uuid(uuid.as_ref())
}

pub(crate) fn query_display_under_the_cursor() -> DisplayId {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let mut cursor = CGPoint::ZERO;
    unsafe { SLSGetCurrentCursorLocation(connection_id, &mut cursor) };
    query_display_at_point(cursor)
}

pub(crate) fn query_count_of_displays_active_for_drawing() -> i32 {
    let mut count: u32 = 0;
    unsafe { CGGetActiveDisplayList(0, core::ptr::null_mut(), &mut count) };
    count as i32
}

pub(crate) fn query_displays_active_for_drawing() -> Vec<DisplayId> {
    let display_count = query_count_of_displays_active_for_drawing();
    let mut result: Vec<u32> = vec![0; display_count as usize];
    let mut count: u32 = 0;
    unsafe { CGGetActiveDisplayList(display_count as u32, result.as_mut_ptr(), &mut count) };
    result.truncate(count as usize);
    result.into_iter().map(DisplayId).collect()
}
