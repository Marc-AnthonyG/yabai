#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::{NonNull, null_mut};
use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::carbon_core::read_system_clock_in_nanoseconds;
use crate::ffi::core_foundation::{
    CFMachPortCreateRunLoopSource, CFMachPortInvalidate, CFRetained, CFRunLoopAddSource,
    CFRunLoopGetCurrent, CFRunLoopRun, kCFRunLoopCommonModes,
};
use crate::ffi::core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventGetFlags, CGEventGetIntegerValueField, CGEventMask,
    CGEventTapCreate, CGEventTapEnable, CGEventTapIsEnabled, CGEventTapProxy, CGEventType,
    kCGEventTapOptionDefault, kCGHIDEventTap, kCGHeadInsertEventTap,
};
use crate::ffi::skylight::SLSIsSecureEventInputSet;
use crate::hotkey::action::HotkeyAction;
use crate::hotkey::chord::{Chord, HotkeyModifiers};
use crate::hotkey::keyboard_tap_state::{KEYBOARD_TAP_STATE, KeyboardTapState, KeyboardTapStatus};
use crate::hotkey::lone_modifier::LoneModifier;
use crate::hotkey::table::HOTKEYS_THE_KEYBOARD_TAP_MATCHES;
use crate::state::process_wide::DAEMON_PROCESS_ID;
use crate::support::shell_command::spawn_shell_running_the_command_without_waiting_for_it;

const SYSTEM_DEFINED_EVENT_TYPE_OF_MEDIA_KEYS_NX_SYSDEFINED: u32 = 14;

const KEYBOARD_EVENT_MASK_FOR_HOTKEYS_AND_LONE_MODIFIER_TAPS: CGEventMask = (1
    << CGEventType::KeyDown.0)
    | (1 << CGEventType::FlagsChanged.0)
    | (1 << CGEventType::LeftMouseDown.0)
    | (1 << CGEventType::RightMouseDown.0)
    | (1 << CGEventType::OtherMouseDown.0)
    | (1 << CGEventType::ScrollWheel.0)
    | (1 << SYSTEM_DEFINED_EVENT_TYPE_OF_MEDIA_KEYS_NX_SYSDEFINED);

const EVENT_FLAG_OF_EACH_HOTKEY_MODIFIER: [(CGEventFlags, HotkeyModifiers); 5] = [
    (CGEventFlags::MaskAlternate, HotkeyModifiers::ALT),
    (CGEventFlags::MaskShift, HotkeyModifiers::SHIFT),
    (CGEventFlags::MaskCommand, HotkeyModifiers::CMD),
    (CGEventFlags::MaskControl, HotkeyModifiers::CTRL),
    (CGEventFlags::MaskSecondaryFn, HotkeyModifiers::FN),
];

pub(crate) fn start_keyboard_event_tap_on_its_own_thread() {
    map_the_shared_memory_secure_event_input_is_read_from_on_the_main_thread();
    let spawned = std::thread::Builder::new()
        .name(String::from("yabai-keyboard-tap"))
        .spawn(run_the_keyboard_event_tap_on_this_thread_for_as_long_as_the_daemon_runs);
    if let Err(error) = spawned {
        KEYBOARD_TAP_STATE.record_status(KeyboardTapStatus::FailedToStart);
        crate::warn!(
            "yabai: could not start the keyboard tap thread, so hotkeys will not work: {error}\n"
        );
    }
}

fn map_the_shared_memory_secure_event_input_is_read_from_on_the_main_thread() {
    let _ = is_secure_event_input_on();
}

fn run_the_keyboard_event_tap_on_this_thread_for_as_long_as_the_daemon_runs() {
    if let Err(failure) = create_the_keyboard_event_tap_and_schedule_it_on_this_thread() {
        KEYBOARD_TAP_STATE.record_status(KeyboardTapStatus::FailedToStart);
        crate::warn!("yabai: {failure}, so hotkeys will not work\n");
        return;
    }
    KEYBOARD_TAP_STATE.record_status(KeyboardTapStatus::Running);
    CFRunLoopRun();
}

fn create_the_keyboard_event_tap_and_schedule_it_on_this_thread() -> Result<(), &'static str> {
    let keyboard_tap_state = &KEYBOARD_TAP_STATE;
    let handle = unsafe {
        CGEventTapCreate(
            kCGHIDEventTap,
            kCGHeadInsertEventTap,
            kCGEventTapOptionDefault,
            KEYBOARD_EVENT_MASK_FOR_HOTKEYS_AND_LONE_MODIFIER_TAPS,
            Some(handle_keyboard_event_tap_callback),
            keyboard_tap_state as *const KeyboardTapState as *mut c_void,
        )
    };
    let Some(handle) = handle else {
        return Err("could not create the keyboard event tap");
    };
    let handle = CFRetained::into_raw(handle);
    keyboard_tap_state
        .handle
        .store(handle.as_ptr(), Ordering::Release);
    let handle = unsafe { handle.as_ref() };

    if !CGEventTapIsEnabled(handle) {
        CFMachPortInvalidate(handle);
        return Err("the keyboard event tap is disabled");
    }

    let (Some(run_loop_source), Some(run_loop_of_this_thread)) = (
        CFMachPortCreateRunLoopSource(None, Some(handle), 0),
        CFRunLoopGetCurrent(),
    ) else {
        CFMachPortInvalidate(handle);
        return Err("could not schedule the keyboard event tap");
    };
    CFRunLoopAddSource(&run_loop_of_this_thread, Some(&run_loop_source), unsafe {
        kCFRunLoopCommonModes
    });
    Ok(())
}

unsafe extern "C-unwind" fn handle_keyboard_event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    context: *mut c_void,
) -> *mut CGEvent {
    let keyboard_tap_state = unsafe { &*(context as *const KeyboardTapState) };
    let lone_modifier_tap_detector = &keyboard_tap_state.lone_modifier_tap_detector;

    if let CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput = event_type {
        lone_modifier_tap_detector.forget_the_armed_lone_modifier();
        re_enable_the_keyboard_event_tap(keyboard_tap_state);
        return event.as_ptr();
    }

    let event_reference = unsafe { event.as_ref() };
    if was_posted_by_this_process(event_reference) {
        return event.as_ptr();
    }

    match event_type {
        CGEventType::KeyDown => {
            lone_modifier_tap_detector.forget_the_armed_lone_modifier();
            if run_the_hotkey_the_key_down_presses(event_reference) {
                return null_mut();
            }
        }
        CGEventType::FlagsChanged if is_secure_event_input_on() => {
            lone_modifier_tap_detector.forget_the_armed_lone_modifier();
        }
        CGEventType::FlagsChanged => {
            let tapped_lone_modifier = lone_modifier_tap_detector
                .lone_modifier_tapped_by_flags_change(
                    ansi_virtual_keycode_of_event(event_reference),
                    CGEventGetFlags(Some(event_reference)).0,
                    read_system_clock_in_nanoseconds(),
                );
            if let Some(tapped_lone_modifier) = tapped_lone_modifier {
                run_the_hotkey_of_the_lone_modifier_tap(tapped_lone_modifier);
            }
        }
        CGEventType::ScrollWheel
            if is_momentum_scrolling_the_user_no_longer_drives(event_reference) => {}
        _ => lone_modifier_tap_detector.forget_the_armed_lone_modifier(),
    }

    event.as_ptr()
}

fn re_enable_the_keyboard_event_tap(keyboard_tap_state: &KeyboardTapState) {
    let handle = keyboard_tap_state.handle.load(Ordering::Acquire);
    if let Some(handle) = unsafe { handle.as_ref() } {
        CGEventTapEnable(handle, true);
    }
}

fn run_the_hotkey_the_key_down_presses(event: &CGEvent) -> bool {
    let ansi_virtual_keycode = ansi_virtual_keycode_of_event(event);
    let modifiers_held =
        hotkey_modifiers_held_according_to_event_flags(CGEventGetFlags(Some(event)));
    let chord_and_action_of_the_pressed_hotkey = HOTKEYS_THE_KEYBOARD_TAP_MATCHES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .hotkey_pressed_by(ansi_virtual_keycode, modifiers_held)
        .map(|hotkey| (hotkey.chord, hotkey.action.clone()));
    let Some((chord, action)) = chord_and_action_of_the_pressed_hotkey else {
        return false;
    };
    run_hotkey_action_without_waiting_for_it(chord, action);
    true
}

fn run_the_hotkey_of_the_lone_modifier_tap(lone_modifier: LoneModifier) {
    let chord_and_action_of_the_tapped_hotkey = HOTKEYS_THE_KEYBOARD_TAP_MATCHES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .hotkey_of_lone_modifier_tap(lone_modifier)
        .map(|hotkey| (hotkey.chord, hotkey.action.clone()));
    if let Some((chord, action)) = chord_and_action_of_the_tapped_hotkey {
        run_hotkey_action_without_waiting_for_it(chord, action);
    }
}

fn run_hotkey_action_without_waiting_for_it(chord: Chord, action: HotkeyAction) {
    match action {
        HotkeyAction::Shell(shell_command) => {
            let shell = spawn_shell_running_the_command_without_waiting_for_it(
                &shell_command,
                std::iter::empty::<(&str, &str)>(),
            );
            if let Err(error) = shell {
                crate::warn!("yabai: hotkey '{chord}' could not run its shell command: {error}\n");
            }
        }
        HotkeyAction::Yabai(yabai_command_arguments) => {
            post_event_to_event_loop(Event::HotkeyPressed {
                chord,
                yabai_command_arguments,
            })
        }
    }
}

fn hotkey_modifiers_held_according_to_event_flags(event_flags: CGEventFlags) -> HotkeyModifiers {
    EVENT_FLAG_OF_EACH_HOTKEY_MODIFIER
        .into_iter()
        .filter(|(event_flag, _)| event_flags.0 & event_flag.0 == event_flag.0)
        .fold(HotkeyModifiers::empty(), |modifiers_held, (_, modifier)| {
            modifiers_held | modifier
        })
}

fn ansi_virtual_keycode_of_event(event: &CGEvent) -> u16 {
    CGEventGetIntegerValueField(Some(event), CGEventField::KeyboardEventKeycode) as u16
}

fn is_momentum_scrolling_the_user_no_longer_drives(event: &CGEvent) -> bool {
    CGEventGetIntegerValueField(Some(event), CGEventField::ScrollWheelEventMomentumPhase) != 0
}

fn was_posted_by_this_process(event: &CGEvent) -> bool {
    let process_id_of_the_poster =
        CGEventGetIntegerValueField(Some(event), CGEventField::EventSourceUnixProcessID);
    DAEMON_PROCESS_ID
        .get()
        .is_some_and(|daemon_process_id| i64::from(*daemon_process_id) == process_id_of_the_poster)
}

fn is_secure_event_input_on() -> bool {
    unsafe { SLSIsSecureEventInputSet() }
}
