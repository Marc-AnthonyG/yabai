#![allow(non_upper_case_globals)]

use core::ffi::{c_ulong, c_void};

pub type OSStatus = i32;
pub const noErr: OSStatus = 0;

pub type EventTime = f64;

#[repr(C)]
pub struct OpaqueEventRef {
    _private: [u8; 0],
}
pub type EventRef = *mut OpaqueEventRef;

#[repr(C)]
pub struct OpaqueEventHandlerRef {
    _private: [u8; 0],
}
pub type EventHandlerRef = *mut OpaqueEventHandlerRef;

#[repr(C)]
pub struct OpaqueEventHandlerCallRef {
    _private: [u8; 0],
}
pub type EventHandlerCallRef = *mut OpaqueEventHandlerCallRef;

#[repr(C)]
pub struct OpaqueEventTargetRef {
    _private: [u8; 0],
}
pub type EventTargetRef = *mut OpaqueEventTargetRef;

pub type EventHandlerProcPtr = unsafe extern "C-unwind" fn(
    in_handler_call_ref: EventHandlerCallRef,
    in_event: EventRef,
    in_user_data: *mut c_void,
) -> OSStatus;
pub type EventHandlerUPP = EventHandlerProcPtr;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct EventTypeSpec {
    pub event_class: u32,
    pub event_kind: u32,
}

const _: () = assert!(core::mem::size_of::<EventTypeSpec>() == 8);

pub const kEventClassApplication: u32 = 0x6170_706C;
pub const kEventAppLaunched: u32 = 5;
pub const kEventAppTerminated: u32 = 6;
pub const kEventAppFrontSwitched: u32 = 7;
pub const kEventParamProcessID: u32 = 0x7073_6E20;
pub const typeProcessSerialNumber: u32 = 0x7073_6E20;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub fn GetApplicationEventTarget() -> EventTargetRef;
    pub fn InstallEventHandler(
        in_target: EventTargetRef,
        in_handler: EventHandlerUPP,
        in_num_types: c_ulong,
        in_list: *const EventTypeSpec,
        in_user_data: *mut c_void,
        out_ref: *mut EventHandlerRef,
    ) -> OSStatus;
    pub fn GetEventParameter(
        in_event: EventRef,
        in_name: u32,
        in_desired_type: u32,
        out_actual_type: *mut u32,
        in_buffer_size: c_ulong,
        out_actual_size: *mut c_ulong,
        out_data: *mut c_void,
    ) -> OSStatus;
    pub fn GetEventKind(in_event: EventRef) -> u32;
    pub fn GetCurrentEventTime() -> EventTime;
}
