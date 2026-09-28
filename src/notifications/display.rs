use core::ffi::c_void;

use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::core_graphics::{
    CGDirectDisplayID, CGDisplayChangeSummaryFlags, kCGDisplayAddFlag,
    kCGDisplayDesktopShapeChangedFlag, kCGDisplayMovedFlag, kCGDisplayRemoveFlag,
};
use crate::support::handles::DisplayId;

pub(crate) unsafe extern "C-unwind" fn handle_display_reconfiguration_callback(
    display_id: CGDirectDisplayID,
    flags: CGDisplayChangeSummaryFlags,
    _context: *mut c_void,
) {
    if flags.contains(kCGDisplayAddFlag) {
        post_event_to_event_loop(Event::DisplayAdded(DisplayId(display_id)));
    } else if flags.contains(kCGDisplayRemoveFlag) {
        post_event_to_event_loop(Event::DisplayRemoved(DisplayId(display_id)));
    } else if flags.contains(kCGDisplayMovedFlag) {
        post_event_to_event_loop(Event::DisplayMoved(DisplayId(display_id)));
    } else if flags.contains(kCGDisplayDesktopShapeChangedFlag) {
        post_event_to_event_loop(Event::DisplayResized(DisplayId(display_id)));
    }
}
