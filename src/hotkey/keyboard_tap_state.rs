use std::sync::atomic::AtomicPtr;

use crate::ffi::core_foundation::CFMachPort;

pub(crate) struct KeyboardTapState {
    pub(crate) handle: AtomicPtr<CFMachPort>,
}

pub(crate) static KEYBOARD_TAP_STATE: KeyboardTapState = KeyboardTapState {
    handle: AtomicPtr::new(std::ptr::null_mut()),
};
