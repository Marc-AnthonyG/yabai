use crate::display::identity::query_displays_active_for_drawing;
use crate::display::spaces::query_spaces_of_display;
use crate::ffi::core_foundation::{
    cfarray_count, create_cfarray_of_space_ids, take_create_rule_result,
};
use crate::ffi::skylight::{
    SLSCopyWindowsWithOptionsAndTags, SLSWindowIteratorAdvance, SLSWindowIteratorGetAttributes,
    SLSWindowIteratorGetLevel, SLSWindowIteratorGetParentID, SLSWindowIteratorGetTags,
    SLSWindowIteratorGetWindowID, SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows,
};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{SpaceId, WindowId};
use crate::window::model::query_space_holding_window;

pub(crate) fn query_space_of_first_window_owned_by_connection(connection: i32) -> SpaceId {
    let mut space_id = SpaceId(0);

    let display_list = query_displays_active_for_drawing();

    let mut space_list: Vec<u64> = Vec::new();

    for display_id in display_list {
        let Some(list) = query_spaces_of_display(display_id) else {
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

    let space_list_ref = create_cfarray_of_space_ids(&space_list);
    let window_list_ref = unsafe {
        SLSCopyWindowsWithOptionsAndTags(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
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
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
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
                    space_id = query_space_holding_window(WindowId(window_id));
                    break;
                }
            }
        }
    }

    drop(query);
    drop(iterator);
    space_id
}
