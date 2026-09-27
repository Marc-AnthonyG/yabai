use crate::display::identity::display_manager_active_display_list;
use crate::display::spaces::display_space_list;
use crate::ffi::core_foundation::{
    cfarray_count, cfarray_of_cfnumbers, kCFNumberSInt64Type, take_create_rule_result,
};
use crate::ffi::skylight::{
    SLSCopyWindowsWithOptionsAndTags, SLSWindowIteratorAdvance, SLSWindowIteratorGetAttributes,
    SLSWindowIteratorGetLevel, SLSWindowIteratorGetParentID, SLSWindowIteratorGetTags,
    SLSWindowIteratorGetWindowID, SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows,
};
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{SpaceId, WindowId};
use crate::window::model::window_space;

pub(crate) fn process_manager_active_space_for_psn(connection: i32) -> SpaceId {
    let mut space_id = SpaceId(0);

    let display_list = display_manager_active_display_list();

    let mut space_list: Vec<u64> = Vec::new();

    for display_id in display_list {
        let Some(list) = display_space_list(display_id) else {
            continue;
        };

        //
        // NOTE(asmvik): display_space_list(..) uses a linear allocator,
        // and so we only need to track the beginning of the first list along
        // with the total number of windows that have been allocated.
        //

        space_list.extend(list.iter().map(|space_id| space_id.0));
    }

    let mut set_tags: u64 = 0;
    let mut clear_tags: u64 = 0;
    let options: u32 = 0x2;

    let space_list_ref = cfarray_of_cfnumbers(&space_list, kCFNumberSInt64Type);
    let window_list_ref = unsafe {
        SLSCopyWindowsWithOptionsAndTags(
            *CONNECTION.get().unwrap(),
            connection as u32,
            &*space_list_ref,
            options,
            &mut set_tags,
            &mut clear_tags,
        )
    };
    let Some(window_list_ref) = (unsafe { take_create_rule_result(window_list_ref) }) else {
        return space_id;
    };

    let count = cfarray_count(&window_list_ref) as i32;
    if count == 0 {
        return space_id;
    }

    let query = unsafe {
        take_create_rule_result(SLSWindowQueryWindows(
            *CONNECTION.get().unwrap(),
            &*window_list_ref,
            count,
        ))
    };
    let Some(query) = query else {
        return space_id;
    };

    let iterator = unsafe { take_create_rule_result(SLSWindowQueryResultCopyWindows(&*query)) };
    let Some(iterator) = iterator else {
        return space_id;
    };

    while unsafe { SLSWindowIteratorAdvance(&*iterator) } {
        let tags = unsafe { SLSWindowIteratorGetTags(&*iterator) };
        let attributes = unsafe { SLSWindowIteratorGetAttributes(&*iterator) };
        let parent_window_id = unsafe { SLSWindowIteratorGetParentID(&*iterator) };
        let window_id = unsafe { SLSWindowIteratorGetWindowID(&*iterator) };
        let level = unsafe { SLSWindowIteratorGetLevel(&*iterator) };

        if parent_window_id == 0 {
            if level == 0 || level == 3 || level == 8 {
                if ((attributes & 0x2) != 0 || (tags & 0x400000000000000) != 0)
                    && ((tags & 0x1) != 0 || ((tags & 0x2) != 0 && (tags & 0x80000000) != 0))
                {
                    space_id = window_space(WindowId(window_id));
                    break;
                }
            }
        }
    }

    drop(query);
    drop(iterator);
    space_id
}
