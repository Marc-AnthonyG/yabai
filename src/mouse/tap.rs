use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, Ordering};

use crate::ffi::core_foundation::{CFMachPort, CFRunLoopSource, CGPoint, CGRect};
use crate::ffi::core_graphics::{CGEvent, CGRectContainsPoint};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct MouseModifier(pub u8);

impl MouseModifier {
    pub(crate) const ALT: MouseModifier = MouseModifier(0x02);
    pub(crate) const SHIFT: MouseModifier = MouseModifier(0x04);
    pub(crate) const COMMAND: MouseModifier = MouseModifier(0x08);
    pub(crate) const CONTROL: MouseModifier = MouseModifier(0x10);
    pub(crate) const FUNCTION: MouseModifier = MouseModifier(0x20);
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub(crate) enum MouseMode {
    #[default]
    None = 0,
    Move = 1,
    Resize = 2,
    Swap = 3,
    Stack = 4,
}

impl MouseMode {
    pub(crate) fn from_discriminant(discriminant: u8) -> MouseMode {
        match discriminant {
            1 => MouseMode::Move,
            2 => MouseMode::Resize,
            3 => MouseMode::Swap,
            4 => MouseMode::Stack,
            _ => MouseMode::None,
        }
    }
}

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
    pub swallowed_a_click_on_a_group_header: AtomicBool,
}

pub static GROUP_HEADER_FRAMES_WHOSE_CLICKS_THE_TAP_SWALLOWS: Mutex<Vec<CGRect>> =
    Mutex::new(Vec::new());

pub(crate) fn is_point_on_a_visible_group_header(point: CGPoint) -> bool {
    GROUP_HEADER_FRAMES_WHOSE_CLICKS_THE_TAP_SWALLOWS
        .lock()
        .is_ok_and(|header_frames| {
            header_frames
                .iter()
                .any(|header_frame| CGRectContainsPoint(*header_frame, point))
        })
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
            swallowed_a_click_on_a_group_header: AtomicBool::new(false),
        }
    }
}

pub static MOUSE_TAP_STATE: MouseTapState = MouseTapState::new();

pub(crate) fn set_default_mouse_modifier_and_actions() {
    MOUSE_TAP_STATE
        .modifier
        .store(MouseModifier::FUNCTION.0, Ordering::Relaxed);
    MOUSE_TAP_STATE
        .action1
        .store(MouseMode::Move as u8, Ordering::Relaxed);
    MOUSE_TAP_STATE
        .action2
        .store(MouseMode::Resize as u8, Ordering::Relaxed);
    MOUSE_TAP_STATE
        .drop_action
        .store(MouseMode::Swap as u8, Ordering::Relaxed);
}
