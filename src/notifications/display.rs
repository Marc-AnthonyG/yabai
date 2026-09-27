use core::ffi::c_void;

use crate::event::queue::{Event, event_loop_post};
use crate::ffi::core_graphics::{
    CGDirectDisplayID, CGDisplayChangeSummaryFlags, kCGDisplayAddFlag,
    kCGDisplayDesktopShapeChangedFlag, kCGDisplayMovedFlag, kCGDisplayRemoveFlag,
};
use crate::support::handles::DisplayId;

pub(crate) unsafe extern "C-unwind" fn display_handler(
    display_id: CGDirectDisplayID,
    flags: CGDisplayChangeSummaryFlags,
    _context: *mut c_void,
) {
    if flags.contains(kCGDisplayAddFlag) {
        event_loop_post(Event::DisplayAdded(DisplayId(display_id)));
    } else if flags.contains(kCGDisplayRemoveFlag) {
        event_loop_post(Event::DisplayRemoved(DisplayId(display_id)));
    } else if flags.contains(kCGDisplayMovedFlag) {
        event_loop_post(Event::DisplayMoved(DisplayId(display_id)));
    } else if flags.contains(kCGDisplayDesktopShapeChangedFlag) {
        event_loop_post(Event::DisplayResized(DisplayId(display_id)));
    }
}
