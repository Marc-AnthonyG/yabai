#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::{NonNull, null_mut};
use std::sync::atomic::Ordering;

use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::carbon_core::read_system_clock_in_nanoseconds;
use crate::ffi::core_foundation::{
    CFMachPortCreateRunLoopSource, CFMachPortInvalidate, CFRetained, CFRetainedAssumedSendAndSync,
    CFRunLoopAddSource, CFRunLoopGetMain, CFRunLoopRemoveSource, kCFRunLoopCommonModes,
};
use crate::ffi::core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventGetFlags, CGEventGetIntegerValueField, CGEventMask,
    CGEventTapCreate, CGEventTapEnable, CGEventTapIsEnabled, CGEventTapPostEvent, CGEventTapProxy,
    CGEventType, kCGEventTapOptionDefault, kCGHIDEventTap, kCGHeadInsertEventTap,
};
use crate::mouse::tap::{MOUSE_TAP_STATE, MouseModifier, MouseTapState};
use crate::state::process_wide::{
    DOCK_SWIPE_GESTURE_IS_IN_PROGRESS, LAST_DOCK_SWIPE_GESTURE_END_TIME,
};

pub(crate) const MOUSE_EVENT_MASK_FOR_FOCUS_FOLLOWS_MOUSE: u32 = (1 << CGEventType::MouseMoved.0)
    | (1 << CGEventType::LeftMouseDown.0)
    | (1 << CGEventType::LeftMouseUp.0)
    | (1 << CGEventType::LeftMouseDragged.0)
    | (1 << CGEventType::RightMouseDown.0)
    | (1 << CGEventType::RightMouseUp.0)
    | (1 << CGEventType::RightMouseDragged.0)
    | (1 << /* kCGSEventDockControl */ 30);

pub(crate) const MOUSE_EVENT_MASK_WITHOUT_MOUSE_MOVED: u32 = (1 << CGEventType::LeftMouseDown.0)
    | (1 << CGEventType::LeftMouseUp.0)
    | (1 << CGEventType::LeftMouseDragged.0)
    | (1 << CGEventType::RightMouseDown.0)
    | (1 << CGEventType::RightMouseUp.0)
    | (1 << CGEventType::RightMouseDragged.0)
    | (1 << /* kCGSEventDockControl */ 30);

pub(crate) fn mouse_modifier_from_core_graphics_event_flags(
    core_graphics_event_flags: u32,
) -> MouseModifier {
    let mut flags: u8 = 0;

    if (core_graphics_event_flags as u64 & CGEventFlags::MaskAlternate.0)
        == CGEventFlags::MaskAlternate.0
    {
        flags |= MouseModifier::ALT.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskShift.0) == CGEventFlags::MaskShift.0 {
        flags |= MouseModifier::SHIFT.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskCommand.0)
        == CGEventFlags::MaskCommand.0
    {
        flags |= MouseModifier::COMMAND.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskControl.0)
        == CGEventFlags::MaskControl.0
    {
        flags |= MouseModifier::CONTROL.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskSecondaryFn.0)
        == CGEventFlags::MaskSecondaryFn.0
    {
        flags |= MouseModifier::FUNCTION.0;
    }

    MouseModifier(flags)
}

pub(crate) unsafe extern "C-unwind" fn handle_mouse_event_tap_callback(
    proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    context: *mut c_void,
) -> *mut CGEvent {
    let mouse_state = unsafe { &*(context as *const MouseTapState) };

    match event_type {
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
            let handle = mouse_state.handle.load(Ordering::Acquire);
            if let Some(handle) = unsafe { handle.as_ref() } {
                CGEventTapEnable(handle, true);
            }
        }
        CGEventType::LeftMouseDown | CGEventType::RightMouseDown => {
            let event_modifier = mouse_modifier_from_core_graphics_event_flags(
                CGEventGetFlags(Some(unsafe { event.as_ref() })).0 as u32,
            );
            post_event_to_event_loop(Event::MouseDown {
                event: CFRetainedAssumedSendAndSync(unsafe { CFRetained::retain(event) }),
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
            post_event_to_event_loop(Event::MouseUp {
                event: CFRetainedAssumedSendAndSync(unsafe { CFRetained::retain(event) }),
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
            post_event_to_event_loop(Event::MouseDragged {
                event: CFRetainedAssumedSendAndSync(unsafe { CFRetained::retain(event) }),
            });
        }
        CGEventType::MouseMoved => {
            let event_modifier = mouse_modifier_from_core_graphics_event_flags(
                CGEventGetFlags(Some(unsafe { event.as_ref() })).0 as u32,
            );
            if event_modifier.0 == mouse_state.modifier.load(Ordering::Relaxed) {
                return event.as_ptr();
            }

            post_event_to_event_loop(Event::MouseMoved {
                event: CFRetainedAssumedSendAndSync(unsafe { CFRetained::retain(event) }),
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
                        DOCK_SWIPE_GESTURE_IS_IN_PROGRESS.store(true, Ordering::Release);
                    } else if phase == /* kCGSGesturePhaseEnded */ 4
                        || phase == /* kCGSGesturePhaseCancelled */ 8
                    {
                        DOCK_SWIPE_GESTURE_IS_IN_PROGRESS.store(false, Ordering::Release);
                        LAST_DOCK_SWIPE_GESTURE_END_TIME
                            .store(read_system_clock_in_nanoseconds(), Ordering::Release);
                    }
                }
            }
        }
        _ => {}
    }

    event.as_ptr()
}

pub(crate) fn start_mouse_event_tap(mask: u32) -> bool {
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
            Some(handle_mouse_event_tap_callback),
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

pub(crate) fn stop_mouse_event_tap() {
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
