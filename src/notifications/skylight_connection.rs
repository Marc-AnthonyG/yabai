use core::ffi::c_void;
use std::sync::atomic::Ordering;

use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::carbon_core::read_system_clock_in_nanoseconds;
use crate::state::process_wide::LAST_COMMAND_TAB_TIME;
use crate::support::handles::{SpaceId, WindowId};

pub(crate) unsafe extern "C-unwind" fn handle_skylight_connection_notification_callback(
    notification_type: u32,
    data: *mut c_void,
    data_length: usize,
    _context: *mut c_void,
    _connection_id: i32,
) {
    if notification_type == 1204 {
        post_event_to_event_loop(Event::MissionControlEnter);
    } else if notification_type == 1327 {
        if !data.is_null() && data_length >= size_of::<u64>() {
            let space_id: u64 = unsafe { data.cast::<u64>().read_unaligned() };
            post_event_to_event_loop(Event::SkylightSpaceCreated(SpaceId(space_id)));
        }
    } else if notification_type == 1328 {
        if !data.is_null() && data_length >= size_of::<u64>() {
            let space_id: u64 = unsafe { data.cast::<u64>().read_unaligned() };
            post_event_to_event_loop(Event::SkylightSpaceDestroyed(SpaceId(space_id)));
        }
    } else if notification_type == 808 {
        if !data.is_null() && data_length >= size_of::<u32>() {
            let window_id: u32 = unsafe { data.cast::<u32>().read_unaligned() };
            post_event_to_event_loop(Event::SkylightWindowOrdered(WindowId(window_id)));
        }
    } else if notification_type == 804 {
        if !data.is_null() && data_length >= size_of::<u32>() {
            let window_id: u32 = unsafe { data.cast::<u32>().read_unaligned() };
            post_event_to_event_loop(Event::SkylightWindowDestroyed(WindowId(window_id)));
        }
    } else if notification_type == 1202 {
        LAST_COMMAND_TAB_TIME.store(read_system_clock_in_nanoseconds(), Ordering::Release);
    }
}
