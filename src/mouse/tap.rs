#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::{NonNull, null_mut};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, Ordering};

use crate::event_loop::{Event, event_loop_post};
use crate::ffi::carbon_core::read_os_timer;
use crate::ffi::core_foundation::{
    CFMachPort, CFMachPortCreateRunLoopSource, CFMachPortInvalidate, CFRetained, CFRunLoopAddSource,
    CFRunLoopGetMain, CFRunLoopRemoveSource, CFRunLoopSource, SendCFRetained, kCFRunLoopCommonModes,
};
use crate::ffi::core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventGetFlags, CGEventGetIntegerValueField, CGEventMask,
    CGEventTapCreate, CGEventTapEnable, CGEventTapIsEnabled, CGEventTapPostEvent, CGEventTapProxy,
    CGEventType, kCGEventTapOptionDefault, kCGHIDEventTap, kCGHeadInsertEventTap,
};
use crate::globals::{LAST_GESTURE_TIME, PENDING_GESTURE};

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

pub(crate) const MOUSE_EVENT_MASK_FFM: u32 = (1 << CGEventType::MouseMoved.0)
    | (1 << CGEventType::LeftMouseDown.0)
    | (1 << CGEventType::LeftMouseUp.0)
    | (1 << CGEventType::LeftMouseDragged.0)
    | (1 << CGEventType::RightMouseDown.0)
    | (1 << CGEventType::RightMouseUp.0)
    | (1 << CGEventType::RightMouseDragged.0)
    | (1 << /* kCGSEventDockControl */ 30);

pub(crate) const MOUSE_EVENT_MASK: u32 = (1 << CGEventType::LeftMouseDown.0)
    | (1 << CGEventType::LeftMouseUp.0)
    | (1 << CGEventType::LeftMouseDragged.0)
    | (1 << CGEventType::RightMouseDown.0)
    | (1 << CGEventType::RightMouseUp.0)
    | (1 << CGEventType::RightMouseDragged.0)
    | (1 << /* kCGSEventDockControl */ 30);

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

pub(crate) fn mouse_mod_from_cgflags(core_graphics_event_flags: u32) -> MouseMod {
    let mut flags: u8 = 0;

    if (core_graphics_event_flags as u64 & CGEventFlags::MaskAlternate.0) == CGEventFlags::MaskAlternate.0 {
        flags |= MouseMod::ALT.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskShift.0) == CGEventFlags::MaskShift.0 {
        flags |= MouseMod::SHIFT.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskCommand.0) == CGEventFlags::MaskCommand.0 {
        flags |= MouseMod::CMD.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskControl.0) == CGEventFlags::MaskControl.0 {
        flags |= MouseMod::CTRL.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskSecondaryFn.0) == CGEventFlags::MaskSecondaryFn.0 {
        flags |= MouseMod::FN.0;
    }

    MouseMod(flags)
}

pub(crate) unsafe extern "C-unwind" fn mouse_handler(
    proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    context: *mut c_void,
) -> *mut CGEvent {
    let mouse_state = unsafe { &*(context as *const MouseTapState) };

    match event_type {
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
            let handle = mouse_state.handle.load(Ordering::Relaxed);
            if let Some(handle) = unsafe { handle.as_ref() } {
                CGEventTapEnable(handle, true);
            }
        }
        CGEventType::LeftMouseDown | CGEventType::RightMouseDown => {
            let event_modifier =
                mouse_mod_from_cgflags(CGEventGetFlags(Some(unsafe { event.as_ref() })).0 as u32);
            event_loop_post(Event::MouseDown {
                event: SendCFRetained(unsafe { CFRetained::retain(event) }),
                event_modifier,
            });

            if event_modifier.0 == mouse_state.modifier.load(Ordering::Relaxed) {
                mouse_state
                    .consume_mouse_click
                    .store(true, Ordering::Relaxed);
                let consumed_event = CFRetained::into_raw(unsafe { CFRetained::retain(event) });
                let previous_consumed_event = mouse_state
                    .consumed_event
                    .swap(consumed_event.as_ptr(), Ordering::Relaxed);
                if let Some(previous_consumed_event) = NonNull::new(previous_consumed_event) {
                    drop(unsafe { CFRetained::from_raw(previous_consumed_event) });
                }
                return null_mut();
            }
        }
        CGEventType::LeftMouseUp | CGEventType::RightMouseUp => {
            event_loop_post(Event::MouseUp {
                event: SendCFRetained(unsafe { CFRetained::retain(event) }),
            });

            if mouse_state.consume_mouse_click.load(Ordering::Relaxed) {
                let consumed_event = mouse_state
                    .consumed_event
                    .swap(null_mut(), Ordering::Relaxed);
                if !mouse_state.drag_detected.load(Ordering::Relaxed) {
                    unsafe { CGEventTapPostEvent(proxy, consumed_event.as_ref()) };
                    unsafe { CGEventTapPostEvent(proxy, Some(event.as_ref())) };
                }

                mouse_state.drag_detected.store(false, Ordering::Relaxed);
                mouse_state
                    .consume_mouse_click
                    .store(false, Ordering::Relaxed);
                if let Some(consumed_event) = NonNull::new(consumed_event) {
                    drop(unsafe { CFRetained::from_raw(consumed_event) });
                }
                return null_mut();
            }
        }
        CGEventType::LeftMouseDragged | CGEventType::RightMouseDragged => {
            mouse_state.drag_detected.store(true, Ordering::Relaxed);
            event_loop_post(Event::MouseDragged {
                event: SendCFRetained(unsafe { CFRetained::retain(event) }),
            });
        }
        CGEventType::MouseMoved => {
            let event_modifier =
                mouse_mod_from_cgflags(CGEventGetFlags(Some(unsafe { event.as_ref() })).0 as u32);
            if event_modifier.0 == mouse_state.modifier.load(Ordering::Relaxed) {
                return event.as_ptr();
            }

            event_loop_post(Event::MouseMoved {
                event: SendCFRetained(unsafe { CFRetained::retain(event) }),
                event_modifier,
            });
        }
        CGEventType(/* kCGSEventDockControl */ 30) => {
            let gesture_type = CGEventGetIntegerValueField(
                Some(unsafe { event.as_ref() }),
                CGEventField(/* kCGEventGestureHIDType */ 110),
            ) as i32;
            if gesture_type == /* kIOHIDEventTypeDockSwipe */ 23 {
                let motion = CGEventGetIntegerValueField(
                    Some(unsafe { event.as_ref() }),
                    CGEventField(/* kCGEventGestureSwipeMotion */ 123),
                ) as i32;
                if motion == /* kCGGestureMotionHorizontal */ 1 {
                    let phase = CGEventGetIntegerValueField(
                        Some(unsafe { event.as_ref() }),
                        CGEventField(/* kCGEventGesturePhase */ 132),
                    ) as i32;
                    if phase == /* kCGSGesturePhaseBegan */ 1 {
                        PENDING_GESTURE.store(true, Ordering::Release);
                    } else if phase == /* kCGSGesturePhaseEnded */ 4
                        || phase == /* kCGSGesturePhaseCancelled */ 8
                    {
                        PENDING_GESTURE.store(false, Ordering::Release);
                        LAST_GESTURE_TIME.store(read_os_timer(), Ordering::Release);
                    }
                }
            }
        }
        _ => {}
    }

    event.as_ptr()
}

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

pub(crate) fn mouse_handler_begin(mask: u32) -> bool {
    let mouse_state = &MOUSE_TAP_STATE;

    if !mouse_state.handle.load(Ordering::Relaxed).is_null() {
        return true;
    }

    let handle = unsafe {
        CGEventTapCreate(
            kCGHIDEventTap,
            kCGHeadInsertEventTap,
            kCGEventTapOptionDefault,
            mask as CGEventMask,
            Some(mouse_handler),
            mouse_state as *const MouseTapState as *mut c_void,
        )
    };
    let Some(handle) = handle else {
        return false;
    };
    let handle = CFRetained::into_raw(handle);
    mouse_state.handle.store(handle.as_ptr(), Ordering::Release);

    if !CGEventTapIsEnabled(unsafe { handle.as_ref() }) {
        CFMachPortInvalidate(unsafe { handle.as_ref() });
        drop(unsafe { CFRetained::from_raw(handle) });
        mouse_state.handle.store(null_mut(), Ordering::Release);
        return false;
    }

    let runloop_source = CFMachPortCreateRunLoopSource(None, Some(unsafe { handle.as_ref() }), 0)
        .map_or(null_mut(), |runloop_source| {
            CFRetained::into_raw(runloop_source).as_ptr()
        });
    mouse_state
        .runloop_source
        .store(runloop_source, Ordering::Relaxed);
    if let Some(main_run_loop) = CFRunLoopGetMain() {
        CFRunLoopAddSource(&main_run_loop, unsafe { runloop_source.as_ref() }, unsafe {
            kCFRunLoopCommonModes
        });
    }

    true
}

pub(crate) fn mouse_handler_end() {
    let mouse_state = &MOUSE_TAP_STATE;

    let Some(handle) = NonNull::new(mouse_state.handle.load(Ordering::Relaxed)) else {
        return;
    };

    CGEventTapEnable(unsafe { handle.as_ref() }, false);
    CFMachPortInvalidate(unsafe { handle.as_ref() });
    let runloop_source = mouse_state.runloop_source.load(Ordering::Relaxed);
    if let Some(main_run_loop) = CFRunLoopGetMain() {
        CFRunLoopRemoveSource(&main_run_loop, unsafe { runloop_source.as_ref() }, unsafe {
            kCFRunLoopCommonModes
        });
    }
    if let Some(runloop_source) = NonNull::new(runloop_source) {
        drop(unsafe { CFRetained::from_raw(runloop_source) });
    }
    drop(unsafe { CFRetained::from_raw(handle) });
    mouse_state.handle.store(null_mut(), Ordering::Release);
}
