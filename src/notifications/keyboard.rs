#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::{NonNull, null_mut};
use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use libc::qos_class_t;

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
use crate::ffi::libsystem::set_the_quality_of_service_class_of_this_thread;
use crate::ffi::skylight::SLSIsSecureEventInputSet;
use crate::hotkey::action::{HotkeyAction, KeyDownOfAHotkey};
use crate::hotkey::chord::{Chord, HotkeyModifiers};
use crate::hotkey::keyboard_tap_state::{KEYBOARD_TAP_STATE, KeyboardTapState, KeyboardTapStatus};
use crate::hotkey::lone_modifier::LoneModifier;
use crate::hotkey::lone_modifier_tap::KeyboardTapEventSeenByTheLoneModifierTapDetector;
use crate::hotkey::shell_command_spawner::{
    ShellCommandOfAPressedHotkey, hand_the_shell_command_to_the_spawner_thread,
    start_the_thread_that_spawns_shell_hotkeys,
};
use crate::hotkey::table::HOTKEYS_THE_KEYBOARD_TAP_MATCHES;
use crate::state::process_wide::DAEMON_PROCESS_ID;

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
    if let Err(error) = start_the_thread_that_spawns_shell_hotkeys() {
        crate::warn!(
            "yabai: could not start the shell hotkey thread, so shell hotkeys will not run: \
             {error}\n"
        );
    }
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
    set_the_quality_of_service_class_of_this_thread(qos_class_t::QOS_CLASS_USER_INTERACTIVE);
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

    let tapped_lone_modifier = lone_modifier_tap_detector.lone_modifier_tapped_by(
        keyboard_tap_event_as_the_lone_modifier_tap_detector_sees_it(event_type, event_reference),
    );
    if let Some(tapped_lone_modifier) = tapped_lone_modifier {
        run_the_hotkey_of_the_lone_modifier_tap(tapped_lone_modifier);
    }

    if event_type == CGEventType::KeyDown && run_the_hotkey_the_key_down_presses(event_reference) {
        return null_mut();
    }
    event.as_ptr()
}

fn keyboard_tap_event_as_the_lone_modifier_tap_detector_sees_it(
    event_type: CGEventType,
    event: &CGEvent,
) -> KeyboardTapEventSeenByTheLoneModifierTapDetector {
    match event_type {
        CGEventType::FlagsChanged if is_secure_event_input_on() => {
            KeyboardTapEventSeenByTheLoneModifierTapDetector::FlagsChangedUnderSecureEventInput
        }
        CGEventType::FlagsChanged => {
            KeyboardTapEventSeenByTheLoneModifierTapDetector::FlagsChanged {
                flags_changed_keycode: ansi_virtual_keycode_of_event(event),
                event_flags: CGEventGetFlags(Some(event)).0,
                now_in_nanoseconds: read_system_clock_in_nanoseconds(),
            }
        }
        CGEventType::ScrollWheel if is_momentum_scrolling_the_user_no_longer_drives(event) => {
            KeyboardTapEventSeenByTheLoneModifierTapDetector::MomentumScrollingTheUserNoLongerDrives
        }
        _ => KeyboardTapEventSeenByTheLoneModifierTapDetector::AnyOtherEvent,
    }
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
    let key_down = key_down_as_pressed_or_repeated_while_held(event);
    let chord_and_action_to_run_of_the_pressed_hotkey = HOTKEYS_THE_KEYBOARD_TAP_MATCHES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .hotkey_pressed_by(ansi_virtual_keycode, modifiers_held)
        .map(|hotkey| {
            let action_to_run = hotkey
                .action
                .runs_on(key_down)
                .then(|| hotkey.action.clone());
            (hotkey.chord, action_to_run)
        });
    let Some((chord, action_to_run)) = chord_and_action_to_run_of_the_pressed_hotkey else {
        return false;
    };
    if let Some(action) = action_to_run {
        run_hotkey_action_without_waiting_for_it(chord, action);
    }
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
            hand_the_shell_command_to_the_spawner_thread(ShellCommandOfAPressedHotkey {
                chord,
                shell_command,
            })
        }
        HotkeyAction::Yabai(yabai_command) => post_event_to_event_loop(Event::HotkeyPressed {
            chord,
            yabai_command,
        }),
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

fn key_down_as_pressed_or_repeated_while_held(event: &CGEvent) -> KeyDownOfAHotkey {
    if CGEventGetIntegerValueField(Some(event), CGEventField::KeyboardEventAutorepeat) != 0 {
        KeyDownOfAHotkey::RepeatedWhileHeld
    } else {
        KeyDownOfAHotkey::Pressed
    }
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

#[cfg(test)]
mod tests {
    use super::hotkey_modifiers_held_according_to_event_flags;
    use crate::ffi::core_graphics::CGEventFlags;
    use crate::hotkey::chord::HotkeyModifiers;

    const LEFT_COMMAND_DEVICE_DEPENDENT_FLAG: u64 = 0x8;
    const RIGHT_COMMAND_DEVICE_DEPENDENT_FLAG: u64 = 0x10;

    #[test]
    fn each_modifier_flag_reads_as_its_hotkey_modifier() {
        assert_eq!(
            hotkey_modifiers_held_according_to_event_flags(
                CGEventFlags::MaskAlternate
                    | CGEventFlags::MaskShift
                    | CGEventFlags::MaskCommand
                    | CGEventFlags::MaskControl
                    | CGEventFlags::MaskSecondaryFn
            ),
            HotkeyModifiers::all()
        );
        assert_eq!(
            hotkey_modifiers_held_according_to_event_flags(CGEventFlags::empty()),
            HotkeyModifiers::empty()
        );
    }

    #[test]
    fn caps_lock_is_no_hotkey_modifier() {
        assert_eq!(
            hotkey_modifiers_held_according_to_event_flags(
                CGEventFlags::MaskAlternate | CGEventFlags::MaskAlphaShift
            ),
            HotkeyModifiers::ALT
        );
    }

    #[test]
    fn the_numeric_pad_flag_arrow_keys_carry_is_no_hotkey_modifier() {
        assert_eq!(
            hotkey_modifiers_held_according_to_event_flags(
                CGEventFlags::MaskControl
                    | CGEventFlags::MaskNumericPad
                    | CGEventFlags::MaskSecondaryFn
            ),
            HotkeyModifiers::CTRL | HotkeyModifiers::FN
        );
    }

    #[test]
    fn either_side_of_a_modifier_reads_as_the_same_hotkey_modifier() {
        for device_dependent_flag in [
            LEFT_COMMAND_DEVICE_DEPENDENT_FLAG,
            RIGHT_COMMAND_DEVICE_DEPENDENT_FLAG,
        ] {
            assert_eq!(
                hotkey_modifiers_held_according_to_event_flags(CGEventFlags(
                    CGEventFlags::MaskCommand.0 | device_dependent_flag
                )),
                HotkeyModifiers::CMD
            );
        }
    }
}
