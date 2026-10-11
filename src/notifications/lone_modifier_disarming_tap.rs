#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::atomic::Ordering;

use crate::ffi::core_foundation::{
    CFMachPortCreateRunLoopSource, CFMachPortInvalidate, CFRetained, CFRunLoopActivity,
    CFRunLoopAddObserver, CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopObserver,
    CFRunLoopObserverContext, CFRunLoopObserverCreate, kCFRunLoopCommonModes,
};
use crate::ffi::core_graphics::{
    CGEvent, CGEventField, CGEventGetIntegerValueField, CGEventMask, CGEventTapCreate,
    CGEventTapEnable, CGEventTapProxy, CGEventType, kCGEventTapOptionDefault, kCGHIDEventTap,
    kCGHeadInsertEventTap,
};
use crate::hotkey::keyboard_tap_state::KeyboardTapState;
use crate::hotkey::lone_modifier_tap::KeyboardTapEventSeenByTheLoneModifierTapDetector;
use crate::notifications::keyboard::was_posted_by_this_process;

const SYSTEM_DEFINED_EVENT_TYPE_OF_MEDIA_KEYS_NX_SYSDEFINED: u32 = 14;

const EVENT_MASK_OF_WHAT_DISARMS_A_LONE_MODIFIER_BESIDES_KEYS: CGEventMask = (1
    << CGEventType::LeftMouseDown.0)
    | (1 << CGEventType::RightMouseDown.0)
    | (1 << CGEventType::OtherMouseDown.0)
    | (1 << CGEventType::ScrollWheel.0)
    | (1 << SYSTEM_DEFINED_EVENT_TYPE_OF_MEDIA_KEYS_NX_SYSDEFINED);

pub(crate) fn create_the_disarming_tap_disabled_and_schedule_it_on_this_thread(
    keyboard_tap_state: &'static KeyboardTapState,
) -> Result<(), &'static str> {
    let handle = unsafe {
        CGEventTapCreate(
            kCGHIDEventTap,
            kCGHeadInsertEventTap,
            kCGEventTapOptionDefault,
            EVENT_MASK_OF_WHAT_DISARMS_A_LONE_MODIFIER_BESIDES_KEYS,
            Some(handle_lone_modifier_disarming_tap_callback),
            keyboard_tap_state as *const KeyboardTapState as *mut c_void,
        )
    };
    let Some(handle) = handle else {
        return Err("could not create the tap that cancels a lone modifier tap");
    };
    let handle = CFRetained::into_raw(handle);
    keyboard_tap_state
        .disarming_tap_handle
        .store(handle.as_ptr(), Ordering::Release);
    let handle = unsafe { handle.as_ref() };
    CGEventTapEnable(handle, false);

    let (Some(run_loop_source), Some(observer), Some(run_loop_of_this_thread)) = (
        CFMachPortCreateRunLoopSource(None, Some(handle), 0),
        observer_matching_the_disarming_tap_to_the_armed_lone_modifier(keyboard_tap_state),
        CFRunLoopGetCurrent(),
    ) else {
        CFMachPortInvalidate(handle);
        return Err("could not schedule the tap that cancels a lone modifier tap");
    };
    let common_modes = unsafe { kCFRunLoopCommonModes };
    CFRunLoopAddSource(
        &run_loop_of_this_thread,
        Some(&run_loop_source),
        common_modes,
    );
    CFRunLoopAddObserver(&run_loop_of_this_thread, Some(&observer), common_modes);
    Ok(())
}

fn observer_matching_the_disarming_tap_to_the_armed_lone_modifier(
    keyboard_tap_state: &'static KeyboardTapState,
) -> Option<CFRetained<CFRunLoopObserver>> {
    let mut context = CFRunLoopObserverContext {
        version: 0,
        info: keyboard_tap_state as *const KeyboardTapState as *mut c_void,
        retain: None,
        release: None,
        copyDescription: None,
    };
    unsafe {
        CFRunLoopObserverCreate(
            None,
            CFRunLoopActivity::BeforeWaiting.0,
            true,
            0,
            Some(enable_the_disarming_tap_exactly_while_a_lone_modifier_is_armed),
            &mut context,
        )
    }
}

unsafe extern "C-unwind" fn enable_the_disarming_tap_exactly_while_a_lone_modifier_is_armed(
    _observer: *mut CFRunLoopObserver,
    _activity: CFRunLoopActivity,
    context: *mut c_void,
) {
    let keyboard_tap_state = unsafe { &*(context as *const KeyboardTapState) };
    let should_be_enabled = keyboard_tap_state
        .lone_modifier_tap_detector
        .is_a_lone_modifier_armed();
    let was_enabled = keyboard_tap_state
        .disarming_tap_is_enabled
        .swap(should_be_enabled, Ordering::Relaxed);
    if was_enabled == should_be_enabled {
        return;
    }
    let handle = keyboard_tap_state
        .disarming_tap_handle
        .load(Ordering::Acquire);
    if let Some(handle) = unsafe { handle.as_ref() } {
        CGEventTapEnable(handle, should_be_enabled);
    }
}

unsafe extern "C-unwind" fn handle_lone_modifier_disarming_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    context: *mut c_void,
) -> *mut CGEvent {
    let keyboard_tap_state = unsafe { &*(context as *const KeyboardTapState) };
    let lone_modifier_tap_detector = &keyboard_tap_state.lone_modifier_tap_detector;

    if let CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput = event_type {
        lone_modifier_tap_detector.forget_the_armed_lone_modifier();
        keyboard_tap_state
            .disarming_tap_is_enabled
            .store(false, Ordering::Relaxed);
        return event.as_ptr();
    }

    let event_reference = unsafe { event.as_ref() };
    if !was_posted_by_this_process(event_reference) {
        lone_modifier_tap_detector.lone_modifier_tapped_by(
            disarming_event_as_the_lone_modifier_tap_detector_sees_it(event_type, event_reference),
        );
    }
    event.as_ptr()
}

fn disarming_event_as_the_lone_modifier_tap_detector_sees_it(
    event_type: CGEventType,
    event: &CGEvent,
) -> KeyboardTapEventSeenByTheLoneModifierTapDetector {
    if event_type == CGEventType::ScrollWheel
        && is_momentum_scrolling_the_user_no_longer_drives(event)
    {
        KeyboardTapEventSeenByTheLoneModifierTapDetector::MomentumScrollingTheUserNoLongerDrives
    } else {
        KeyboardTapEventSeenByTheLoneModifierTapDetector::AnyOtherEvent
    }
}

fn is_momentum_scrolling_the_user_no_longer_drives(event: &CGEvent) -> bool {
    CGEventGetIntegerValueField(Some(event), CGEventField::ScrollWheelEventMomentumPhase) != 0
}
