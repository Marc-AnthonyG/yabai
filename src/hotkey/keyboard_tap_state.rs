use std::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

use crate::ffi::core_foundation::CFMachPort;
use crate::hotkey::lone_modifier_tap::LoneModifierTapDetector;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum KeyboardTapStatus {
    StillStarting,
    Running,
    FailedToStart,
}

const EVERY_KEYBOARD_TAP_STATUS: [KeyboardTapStatus; 3] = [
    KeyboardTapStatus::StillStarting,
    KeyboardTapStatus::Running,
    KeyboardTapStatus::FailedToStart,
];

impl KeyboardTapStatus {
    fn of_discriminant(discriminant: u8) -> KeyboardTapStatus {
        EVERY_KEYBOARD_TAP_STATUS
            .into_iter()
            .find(|status| *status as u8 == discriminant)
            .unwrap_or(KeyboardTapStatus::StillStarting)
    }
}

pub(crate) struct KeyboardTapState {
    pub(crate) handle: AtomicPtr<CFMachPort>,
    pub(crate) lone_modifier_tap_detector: LoneModifierTapDetector,
    status: AtomicU8,
}

pub(crate) static KEYBOARD_TAP_STATE: KeyboardTapState = KeyboardTapState {
    handle: AtomicPtr::new(std::ptr::null_mut()),
    lone_modifier_tap_detector: LoneModifierTapDetector::new(),
    status: AtomicU8::new(KeyboardTapStatus::StillStarting as u8),
};

impl KeyboardTapState {
    pub(crate) fn status(&self) -> KeyboardTapStatus {
        KeyboardTapStatus::of_discriminant(self.status.load(Ordering::Acquire))
    }

    pub(crate) fn record_status(&self, status: KeyboardTapStatus) {
        self.status.store(status as u8, Ordering::Release);
    }
}
