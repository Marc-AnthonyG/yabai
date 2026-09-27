#![allow(non_snake_case)]

use objc2_core_foundation::CFUUID;

#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    pub fn CGDisplayCreateUUIDFromDisplayID(display_id: u32) -> *mut CFUUID;
    pub fn CGDisplayGetDisplayIDFromUUID(uuid: *const CFUUID) -> u32;
}
