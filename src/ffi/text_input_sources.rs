#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use core::ffi::c_void;

use objc2_core_foundation::{CFArray, CFDictionary, CFString, CFType};

use crate::ffi::carbon_events::OSStatus;

pub type TISInputSourceRef = *const CFType;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub static kTISPropertyInputSourceID: &'static CFString;
    pub static kTISPropertyInputSourceCategory: &'static CFString;
    pub static kTISPropertyInputSourceType: &'static CFString;
    pub static kTISPropertyInputSourceIsSelectCapable: &'static CFString;
    pub static kTISPropertyInputSourceIsEnabled: &'static CFString;
    pub static kTISCategoryKeyboardInputSource: &'static CFString;
    pub static kTISTypeKeyboardLayout: &'static CFString;
    pub static kTISTypeKeyboardInputMode: &'static CFString;

    pub fn TISCreateInputSourceList(
        properties: *const CFDictionary,
        include_all_installed: u8,
    ) -> *const CFArray;
    pub fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    pub fn TISGetInputSourceProperty(
        input_source: TISInputSourceRef,
        property_key: *const CFString,
    ) -> *const c_void;
    pub fn TISSelectInputSource(input_source: TISInputSourceRef) -> OSStatus;
}
