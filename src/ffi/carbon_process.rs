#![allow(non_upper_case_globals)]

use core::ffi::{c_char, c_int, c_void};
use libc::pid_t;
use objc2_core_foundation::CFString;
use objc2_core_graphics::CGError;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessSerialNumber {
    pub high_long_of_psn: u32,
    pub low_long_of_psn: u32,
}

pub const kNoProcess: u32 = 0;

const _: () = assert!(core::mem::size_of::<ProcessSerialNumber>() == 8);

#[repr(C, packed(2))]
pub struct ProcessInfoRec {
    pub process_info_length: u32,
    pub process_name: *mut c_char,
    pub process_number: ProcessSerialNumber,
    pub process_type: u32,
    pub process_signature: u32,
    pub process_mode: u32,
    pub process_location: *mut c_char,
    pub process_size: u32,
    pub process_free_mem: u32,
    pub process_launcher: ProcessSerialNumber,
    pub process_launch_date: u32,
    pub process_active_time: u32,
    pub process_app_ref: *mut c_void,
}

const _: () = assert!(core::mem::size_of::<ProcessInfoRec>() == 72);
const _: () = assert!(core::mem::align_of::<ProcessInfoRec>() == 2);
const _: () = assert!(core::mem::offset_of!(ProcessInfoRec, process_type) == 20);
const _: () = assert!(core::mem::offset_of!(ProcessInfoRec, process_app_ref) == 64);

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn GetProcessPID(
        process_serial_number: *const ProcessSerialNumber,
        process_id: *mut pid_t,
    ) -> i32;
    pub fn GetProcessInformation(
        process_serial_number: *const ProcessSerialNumber,
        process_information: *mut ProcessInfoRec,
    ) -> i16;
    pub fn CopyProcessName(
        process_serial_number: *const ProcessSerialNumber,
        name: *mut *mut CFString,
    ) -> i32;
    pub fn GetNextProcess(process_serial_number: *mut ProcessSerialNumber) -> i16;
    pub fn SameProcess(
        process_serial_number_a: *const ProcessSerialNumber,
        process_serial_number_b: *const ProcessSerialNumber,
        result: *mut u8,
    ) -> i16;
    pub fn IsProcessVisible(process_serial_number: *const ProcessSerialNumber) -> u8;

    pub fn CoreDockGetAutoHideEnabled() -> u8;
    pub fn CoreDockGetOrientationAndPinning(orientation: *mut c_int, pinning: *mut c_int);
    pub fn CoreDockSendNotification(notification: *const CFString, unknown: c_int) -> CGError;
}

pub fn is_same_process_serial_number(
    first: *const ProcessSerialNumber,
    second: *const ProcessSerialNumber,
) -> bool {
    let mut result: u8 = 0;
    unsafe { SameProcess(first, second, &mut result) };
    result == 1
}
