use std::sync::atomic::AtomicPtr;

use crate::ffi::core_foundation::CFMachPort;
use crate::hotkey::lone_modifier_tap::LoneModifierTapDetector;

pub(crate) struct KeyboardTapState {
    pub(crate) handle: AtomicPtr<CFMachPort>,
    pub(crate) lone_modifier_tap_detector: LoneModifierTapDetector,
}

pub(crate) static KEYBOARD_TAP_STATE: KeyboardTapState = KeyboardTapState {
    handle: AtomicPtr::new(std::ptr::null_mut()),
    lone_modifier_tap_detector: LoneModifierTapDetector::new(),
};
