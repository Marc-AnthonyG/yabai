use core::ffi::c_void;
use std::sync::atomic::Ordering;

use crate::event::queue::{Event, event_loop_post};
use crate::ffi::carbon_core::read_os_timer;
use crate::state::process_wide::LAST_CMD_TAB_TIME;
use crate::support::handles::{SpaceId, WindowId};

pub(crate) unsafe extern "C-unwind" fn connection_handler(
    notification_type: u32,
    data: *mut c_void,
    data_length: usize,
    _context: *mut c_void,
    _connection_id: i32,
) {
    if notification_type == 1204 {
        event_loop_post(Event::MissionControlEnter);
    } else if notification_type == 1327 {
        if !data.is_null() && data_length >= size_of::<u64>() {
            let space_id: u64 = unsafe { data.cast::<u64>().read_unaligned() };
            event_loop_post(Event::SlsSpaceCreated(SpaceId(space_id)));
        }
    } else if notification_type == 1328 {
        if !data.is_null() && data_length >= size_of::<u64>() {
            let space_id: u64 = unsafe { data.cast::<u64>().read_unaligned() };
            event_loop_post(Event::SlsSpaceDestroyed(SpaceId(space_id)));
        }
    } else if notification_type == 808 {
        if !data.is_null() && data_length >= size_of::<u32>() {
            let window_id: u32 = unsafe { data.cast::<u32>().read_unaligned() };
            event_loop_post(Event::SlsWindowOrdered(WindowId(window_id)));
        }
    } else if notification_type == 804 {
        if !data.is_null() && data_length >= size_of::<u32>() {
            let window_id: u32 = unsafe { data.cast::<u32>().read_unaligned() };
            event_loop_post(Event::SlsWindowDestroyed(WindowId(window_id)));
        }
    } else if notification_type == 1202 {
        LAST_CMD_TAB_TIME.store(read_os_timer(), Ordering::Release);
    }
}
