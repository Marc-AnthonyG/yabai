#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::{NonNull, null_mut};
use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::core_foundation::{
    CFMachPortCreateRunLoopSource, CFMachPortInvalidate, CFRetained, CFRunLoopAddSource,
    CFRunLoopGetCurrent, CFRunLoopRun, kCFRunLoopCommonModes,
};
use crate::ffi::core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventGetFlags, CGEventGetIntegerValueField, CGEventMask,
    CGEventTapCreate, CGEventTapEnable, CGEventTapIsEnabled, CGEventTapProxy, CGEventType,
    kCGEventTapOptionDefault, kCGHeadInsertEventTap, kCGSessionEventTap,
};
use crate::hotkey::action::HotkeyAction;
use crate::hotkey::chord::{Chord, HotkeyModifiers};
use crate::hotkey::keyboard_tap_state::{KEYBOARD_TAP_STATE, KeyboardTapState};
use crate::hotkey::table::HOTKEYS_THE_KEYBOARD_TAP_MATCHES;
use crate::state::process_wide::DAEMON_PROCESS_ID;
use crate::support::shell_command::spawn_shell_running_the_command_without_waiting_for_it;

const KEYBOARD_EVENT_MASK_FOR_HOTKEYS: CGEventMask = 1 << CGEventType::KeyDown.0;

const EVENT_FLAG_OF_EACH_HOTKEY_MODIFIER: [(CGEventFlags, HotkeyModifiers); 5] = [
    (CGEventFlags::MaskAlternate, HotkeyModifiers::ALT),
    (CGEventFlags::MaskShift, HotkeyModifiers::SHIFT),
    (CGEventFlags::MaskCommand, HotkeyModifiers::CMD),
    (CGEventFlags::MaskControl, HotkeyModifiers::CTRL),
    (CGEventFlags::MaskSecondaryFn, HotkeyModifiers::FN),
];

pub(crate) fn start_keyboard_event_tap_on_its_own_thread() {
    let spawned = std::thread::Builder::new()
        .name(String::from("yabai-keyboard-tap"))
        .spawn(run_the_keyboard_event_tap_on_this_thread_for_as_long_as_the_daemon_runs);
    if let Err(error) = spawned {
        crate::warn!(
            "yabai: could not start the keyboard tap thread, so hotkeys will not work: {error}\n"
        );
    }
}

fn run_the_keyboard_event_tap_on_this_thread_for_as_long_as_the_daemon_runs() {
    let keyboard_tap_state = &KEYBOARD_TAP_STATE;
    let handle = unsafe {
        CGEventTapCreate(
            kCGSessionEventTap,
            kCGHeadInsertEventTap,
            kCGEventTapOptionDefault,
            KEYBOARD_EVENT_MASK_FOR_HOTKEYS,
            Some(handle_keyboard_event_tap_callback),
            keyboard_tap_state as *const KeyboardTapState as *mut c_void,
        )
    };
    let Some(handle) = handle else {
        crate::warn!("yabai: could not create the keyboard event tap, so hotkeys will not work\n");
        return;
    };
    let handle = CFRetained::into_raw(handle);
    keyboard_tap_state
        .handle
        .store(handle.as_ptr(), Ordering::Release);
    let handle = unsafe { handle.as_ref() };

    if !CGEventTapIsEnabled(handle) {
        CFMachPortInvalidate(handle);
        crate::warn!("yabai: the keyboard event tap is disabled, so hotkeys will not work\n");
        return;
    }

    let (Some(run_loop_source), Some(run_loop_of_this_thread)) = (
        CFMachPortCreateRunLoopSource(None, Some(handle), 0),
        CFRunLoopGetCurrent(),
    ) else {
        crate::warn!(
            "yabai: could not schedule the keyboard event tap, so hotkeys will not work\n"
        );
        return;
    };
    CFRunLoopAddSource(&run_loop_of_this_thread, Some(&run_loop_source), unsafe {
        kCFRunLoopCommonModes
    });
    CFRunLoopRun();
}

unsafe extern "C-unwind" fn handle_keyboard_event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    context: *mut c_void,
) -> *mut CGEvent {
    let keyboard_tap_state = unsafe { &*(context as *const KeyboardTapState) };

    if let CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput = event_type {
        re_enable_the_keyboard_event_tap(keyboard_tap_state);
        return event.as_ptr();
    }

    let event_reference = unsafe { event.as_ref() };
    if was_posted_by_this_process(event_reference) {
        return event.as_ptr();
    }

    if event_type == CGEventType::KeyDown && run_the_hotkey_the_key_down_presses(event_reference) {
        return null_mut();
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

fn was_posted_by_this_process(event: &CGEvent) -> bool {
    let process_id_of_the_poster =
        CGEventGetIntegerValueField(Some(event), CGEventField::EventSourceUnixProcessID);
    DAEMON_PROCESS_ID
        .get()
        .is_some_and(|daemon_process_id| i64::from(*daemon_process_id) == process_id_of_the_poster)
}
