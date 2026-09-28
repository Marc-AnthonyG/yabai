#![allow(deprecated)]

use crate::display::spaces::query_current_space_of_display;
use crate::ffi::color_sync::CGDisplayGetDisplayIDFromUUID;
use crate::ffi::core_foundation::{
    CFUUIDCreateFromString, cfarray_count, create_cfarray_of_space_ids, take_create_rule_result,
};
use crate::ffi::skylight::{
    SLSCopyManagedDisplayForSpace, SLSCopyWindowsWithOptionsAndTags, SLSSpaceGetType,
    SLSWindowIteratorAdvance, SLSWindowIteratorGetAttributes, SLSWindowIteratorGetLevel,
    SLSWindowIteratorGetParentID, SLSWindowIteratorGetTags, SLSWindowIteratorGetWindowID,
    SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows,
};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{DisplayId, SpaceId, WindowId};
use crate::window::manager::{WindowManager, tracked_window_with_id};
use crate::window::model::{WindowFlag, query_every_space_holding_window};

pub(crate) fn query_display_holding_space(space_id: SpaceId) -> DisplayId {
    let uuid_string = unsafe {
        take_create_rule_result(SLSCopyManagedDisplayForSpace(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
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

pub(crate) fn query_windows_on_spaces_owned_by_connection(
    space_list: &[SpaceId],
    connection_id: i32,
    include_minimized: bool,
    window_manager: &mut WindowManager,
) -> Option<Vec<WindowId>> {
    let mut set_tags: u64 = 0;
    let mut clear_tags: u64 = 0;
    let options: u32 = if include_minimized { 0x7 } else { 0x2 };

    let space_identifier_list: Vec<u64> = space_list.iter().map(|space_id| space_id.0).collect();
    let space_list_ref = create_cfarray_of_space_ids(&space_identifier_list);
    let window_list_ref = unsafe {
        take_create_rule_result(SLSCopyWindowsWithOptionsAndTags(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
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
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
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
            let found_window_id = tracked_window_with_id(window_manager, window_id);
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
            let found_window_id = tracked_window_with_id(window_manager, window_id);
            let window = found_window_id
                .and_then(|found_window_id| window_manager.window.get(&found_window_id));
            if window.is_some_and(|window| !window.flags.contains(WindowFlag::MINIMIZED)) {
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

pub(crate) fn query_windows_on_space(
    space_id: SpaceId,
    include_minimized: bool,
    window_manager: &mut WindowManager,
) -> Option<Vec<WindowId>> {
    query_windows_on_spaces_owned_by_connection(&[space_id], 0, include_minimized, window_manager)
}

pub(crate) fn is_user_space(space_id: SpaceId) -> bool {
    unsafe { SLSSpaceGetType(*SKYLIGHT_CONNECTION_ID.get().unwrap(), space_id.0) == 0 }
}

pub(crate) fn is_native_fullscreen_space(space_id: SpaceId) -> bool {
    unsafe { SLSSpaceGetType(*SKYLIGHT_CONNECTION_ID.get().unwrap(), space_id.0) == 4 }
}

pub(crate) fn is_space_visible_on_its_display(space_id: SpaceId) -> bool {
    space_id == query_current_space_of_display(query_display_holding_space(space_id))
}

pub(crate) fn is_window_on_space(space_id: SpaceId, window_id: WindowId) -> bool {
    let space_list = query_every_space_holding_window(window_id);
    if space_list.is_empty() {
        return false;
    }
    let space_count = space_list.len() as i32;

    for index in 0..space_count {
        if space_id == space_list[index as usize] {
            return true;
        }
    }

    false
}
