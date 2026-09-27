use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, Ordering};

use crate::ffi::core_foundation::{CFMachPort, CFRunLoopSource};
use crate::ffi::core_graphics::CGEvent;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct MouseMod(pub u8);

impl MouseMod {
    pub(crate) const ALT: MouseMod = MouseMod(0x02);
    pub(crate) const SHIFT: MouseMod = MouseMod(0x04);
    pub(crate) const CMD: MouseMod = MouseMod(0x08);
    pub(crate) const CTRL: MouseMod = MouseMod(0x10);
    pub(crate) const FN: MouseMod = MouseMod(0x20);
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

pub(crate) static MOUSE_MOD_STR: [Option<&str>; 33] = [
    None,
    Some("none"),
    Some("alt"),
    None,
    Some("shift"),
    None,
    None,
    None,
    Some("cmd"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("ctrl"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("fn"),
];

pub(crate) static MOUSE_MODE_STR: [&str; 5] = ["none", "move", "resize", "swap", "stack"];

pub(crate) fn mouse_state_init() {
    MOUSE_TAP_STATE
        .modifier
        .store(MouseMod::FN.0, Ordering::Relaxed);
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
