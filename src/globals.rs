use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicU64};

use crate::ffi::core_foundation::{CFMachPort, CFRunLoopSource};
use crate::ffi::core_graphics::CGEvent;

pub static CV_HOST_CLOCK_FREQUENCY: OnceLock<f64> = OnceLock::new();
pub static LAYER_NORMAL_WINDOW_LEVEL: OnceLock<i32> = OnceLock::new();
pub static LAYER_BELOW_WINDOW_LEVEL: OnceLock<i32> = OnceLock::new();
pub static LAYER_ABOVE_WINDOW_LEVEL: OnceLock<i32> = OnceLock::new();

pub static SA_SOCKET_FILE: OnceLock<String> = OnceLock::new();
pub static SOCKET_FILE: OnceLock<String> = OnceLock::new();
pub static CONFIG_FILE: OnceLock<String> = OnceLock::new();
pub static LOCK_FILE: OnceLock<String> = OnceLock::new();

pub static BOOTSTRAP_PORT: OnceLock<libc::mach_port_t> = OnceLock::new();
pub static CONNECTION: OnceLock<i32> = OnceLock::new();
pub static VERBOSE: AtomicBool = AtomicBool::new(false);
pub static PROCESS_ID: OnceLock<i32> = OnceLock::new();

pub static PENDING_WINDOW_FOCUS: AtomicBool = AtomicBool::new(false);
pub static PENDING_GESTURE: AtomicBool = AtomicBool::new(false);
pub static LAST_GESTURE_TIME: AtomicU64 = AtomicU64::new(0);
pub static LAST_CMD_TAB_TIME: AtomicU64 = AtomicU64::new(0);

pub struct MouseTapState {
    pub handle: AtomicPtr<CFMachPort>,
    pub runloop_source: AtomicPtr<CFRunLoopSource>,
    pub consume_mouse_click: AtomicBool,
    pub drag_detected: AtomicBool,
    pub consumed_event: AtomicPtr<CGEvent>,
    pub modifier: AtomicU8,
    pub action1: AtomicU8,
    pub action2: AtomicU8,
    pub drop_action: AtomicU8,
}

impl MouseTapState {
    const fn new() -> MouseTapState {
        MouseTapState {
            handle: AtomicPtr::new(std::ptr::null_mut()),
            runloop_source: AtomicPtr::new(std::ptr::null_mut()),
            consume_mouse_click: AtomicBool::new(false),
            drag_detected: AtomicBool::new(false),
            consumed_event: AtomicPtr::new(std::ptr::null_mut()),
            modifier: AtomicU8::new(0),
            action1: AtomicU8::new(0),
            action2: AtomicU8::new(0),
            drop_action: AtomicU8::new(0),
        }
    }
}

pub static MOUSE_TAP_STATE: MouseTapState = MouseTapState::new();
