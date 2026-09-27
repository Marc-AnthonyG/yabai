#![allow(deprecated)]

use crate::display::display_space_id;
use crate::ffi::color_sync::CGDisplayGetDisplayIDFromUUID;
use crate::ffi::core_foundation::{
    CFUUIDCreateFromString, cfarray_count, cfarray_of_cfnumbers, kCFNumberSInt64Type,
    take_create_rule_result,
};
use crate::ffi::skylight::{
    SLSCopyManagedDisplayForSpace, SLSCopyWindowsWithOptionsAndTags, SLSSpaceGetType,
    SLSWindowIteratorAdvance, SLSWindowIteratorGetAttributes, SLSWindowIteratorGetLevel,
    SLSWindowIteratorGetParentID, SLSWindowIteratorGetTags, SLSWindowIteratorGetWindowID,
    SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows,
};
use crate::globals::CONNECTION;
use crate::handles::{DisplayId, SpaceId, WindowId};
use crate::window::{WindowFlag, window_check_flag};
use crate::window_manager::{WindowManager, window_manager_find_window};

pub(crate) fn space_display_id(space_id: SpaceId) -> DisplayId {
    let uuid_string = unsafe {
        take_create_rule_result(SLSCopyManagedDisplayForSpace(
            *CONNECTION.get().unwrap(),
            space_id.0,
        ))
    };
    let Some(uuid_string) = uuid_string else {
        return DisplayId(0);
    };

    let uuid = CFUUIDCreateFromString(None, Some(&*uuid_string));
    let Some(uuid) = uuid else {
        return DisplayId(0);
    };

    DisplayId(unsafe { CGDisplayGetDisplayIDFromUUID(&*uuid) })
}

pub(crate) fn space_window_list_for_connection(
    space_list: &[SpaceId],
    connection_id: i32,
    include_minimized: bool,
    window_manager: &mut WindowManager,
) -> Option<Vec<WindowId>> {
    let mut set_tags: u64 = 0;
    let mut clear_tags: u64 = 0;
    let options: u32 = if include_minimized { 0x7 } else { 0x2 };

    let space_identifier_list: Vec<u64> = space_list.iter().map(|space_id| space_id.0).collect();
    let space_list_ref = cfarray_of_cfnumbers(&space_identifier_list, kCFNumberSInt64Type);
    let window_list_ref = unsafe {
        take_create_rule_result(SLSCopyWindowsWithOptionsAndTags(
            *CONNECTION.get().unwrap(),
            connection_id as u32,
            &*space_list_ref,
            options,
            &mut set_tags,
            &mut clear_tags,
        ))
    };
    let Some(window_list_ref) = window_list_ref else {
        return None;
    };

    let count = cfarray_count(&window_list_ref) as i32;
    if count == 0 {
        return None;
    }

    let query = unsafe {
        take_create_rule_result(SLSWindowQueryWindows(
            *CONNECTION.get().unwrap(),
            &*window_list_ref,
            count,
        ))
    };
    let Some(query) = query else {
        return None;
    };

    let iterator = unsafe { take_create_rule_result(SLSWindowQueryResultCopyWindows(&*query)) };
    let Some(iterator) = iterator else {
        return None;
    };

    let mut window_list: Vec<WindowId> = Vec::with_capacity(count as usize);

    while unsafe { SLSWindowIteratorAdvance(&*iterator) } {
        let tags = unsafe { SLSWindowIteratorGetTags(&*iterator) };
        let attributes = unsafe { SLSWindowIteratorGetAttributes(&*iterator) };
        let parent_window_id = unsafe { SLSWindowIteratorGetParentID(&*iterator) };
        let window_id = WindowId(unsafe { SLSWindowIteratorGetWindowID(&*iterator) });
        let level = unsafe { SLSWindowIteratorGetLevel(&*iterator) };

        if include_minimized {
            let found_window_id = window_manager_find_window(window_manager, window_id);
            if found_window_id.is_some() {
                window_list.push(window_id);
            } else if parent_window_id == 0 {
                if level == 0 || level == 3 || level == 8 {
                    if ((attributes & 0x2) != 0 || (tags & 0x400000000000000) != 0)
                        && ((tags & 0x1) != 0 || ((tags & 0x2) != 0 && (tags & 0x80000000) != 0))
                    {
                        window_list.push(window_id);
                    } else if (attributes == 0x0 || attributes == 0x1)
                        && ((tags & 0x1000000000000000) != 0 || (tags & 0x300000000000000) != 0)
                        && ((tags & 0x1) != 0 || ((tags & 0x2) != 0 && (tags & 0x80000000) != 0))
                    {
                        window_list.push(window_id);
                    }
                }
            }
        } else {
            let found_window_id = window_manager_find_window(window_manager, window_id);
            let window = found_window_id
                .and_then(|found_window_id| window_manager.window.find(&found_window_id));
            if window.is_some_and(|window| !window_check_flag(window, WindowFlag::MINIMIZE)) {
                window_list.push(window_id);
            } else if parent_window_id == 0 {
                if level == 0 || level == 3 || level == 8 {
                    if ((attributes & 0x2) != 0 || (tags & 0x400000000000000) != 0)
                        && ((tags & 0x1) != 0 || ((tags & 0x2) != 0 && (tags & 0x80000000) != 0))
                    {
                        window_list.push(window_id);
                    }
                }
            }
        }
    }

    drop(query);
    drop(iterator);
    Some(window_list)
}

pub(crate) fn space_window_list(
    space_id: SpaceId,
    include_minimized: bool,
    window_manager: &mut WindowManager,
) -> Option<Vec<WindowId>> {
    space_window_list_for_connection(&[space_id], 0, include_minimized, window_manager)
}

pub(crate) fn space_is_user(space_id: SpaceId) -> bool {
    unsafe { SLSSpaceGetType(*CONNECTION.get().unwrap(), space_id.0) == 0 }
}

pub(crate) fn space_is_fullscreen(space_id: SpaceId) -> bool {
    unsafe { SLSSpaceGetType(*CONNECTION.get().unwrap(), space_id.0) == 4 }
}

pub(crate) fn space_is_visible(space_id: SpaceId) -> bool {
    space_id == display_space_id(space_display_id(space_id))
}
