#![allow(non_camel_case_types)]

use core::ffi::{c_char, c_int, c_void};

pub type mach_msg_bits_t = u32;
pub type mach_msg_size_t = u32;
pub type mach_msg_id_t = i32;
pub type mach_msg_option_t = i32;
pub type mach_msg_timeout_t = u32;
pub type mach_msg_return_t = libc::kern_return_t;

#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct mach_msg_header_t {
    pub msgh_bits: mach_msg_bits_t,
    pub msgh_size: mach_msg_size_t,
    pub msgh_remote_port: libc::mach_port_t,
    pub msgh_local_port: libc::mach_port_t,
    pub msgh_voucher_port: libc::mach_port_t,
    pub msgh_id: mach_msg_id_t,
}

const _: () = assert!(core::mem::size_of::<mach_msg_header_t>() == 24);

#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct mach_msg_ool_descriptor_t {
    pub address: *mut c_void,
    pub deallocate: u8,
    pub copy: u8,
    pub pad1: u8,
    pub descriptor_type: u8,
    pub size: mach_msg_size_t,
}

const _: () = assert!(core::mem::size_of::<mach_msg_ool_descriptor_t>() == 16);
const _: () = assert!(core::mem::align_of::<mach_msg_ool_descriptor_t>() == 4);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NDR_record_t {
    pub mig_vers: u8,
    pub if_vers: u8,
    pub reserved1: u8,
    pub mig_encoding: u8,
    pub int_rep: u8,
    pub char_rep: u8,
    pub float_rep: u8,
    pub reserved2: u8,
}

const _: () = assert!(core::mem::size_of::<NDR_record_t>() == 8);

unsafe extern "C" {
    pub static NDR_record: NDR_record_t;
}

pub const MACH_MSG_TYPE_COPY_SEND: u32 = 19;
pub const MACH_MSGH_BITS_REMOTE_MASK: u32 = 0x1f;
pub const MACH_MSGH_BITS_COMPLEX: u32 = 0x8000_0000;
pub const MACH_MSG_OOL_DESCRIPTOR: u8 = 1;
pub const MACH_MSG_VIRTUAL_COPY: u8 = 1;
pub const MACH_SEND_MSG: mach_msg_option_t = 0x0000_0001;
pub const MACH_RCV_MSG: mach_msg_option_t = 0x0000_0002;
pub const TASK_BOOTSTRAP_PORT: c_int = 4;

unsafe extern "C" {
    pub fn mach_msg(
        message: *mut mach_msg_header_t,
        option: mach_msg_option_t,
        send_size: mach_msg_size_t,
        receive_size: mach_msg_size_t,
        receive_name: libc::mach_port_t,
        timeout: mach_msg_timeout_t,
        notify: libc::mach_port_t,
    ) -> mach_msg_return_t;

    pub fn task_get_special_port(
        task: libc::mach_port_t,
        which_port: c_int,
        special_port: *mut libc::mach_port_t,
    ) -> libc::kern_return_t;

    pub fn bootstrap_look_up(
        bootstrap_port: libc::mach_port_t,
        service_name: *const c_char,
        service_port: *mut libc::mach_port_t,
    ) -> libc::kern_return_t;

    pub fn mig_get_special_reply_port() -> libc::mach_port_t;

    pub fn mach_port_deallocate(
        task: libc::mach_port_t,
        name: libc::mach_port_t,
    ) -> libc::kern_return_t;

    static mach_task_self_: libc::mach_port_t;
}

pub fn mach_task_self() -> libc::mach_port_t {
    unsafe { mach_task_self_ }
}

#[repr(C, packed(4))]
struct MachMessageWithOneOutOfLineDescriptor {
    header: mach_msg_header_t,
    descriptor_count: mach_msg_size_t,
    descriptor: mach_msg_ool_descriptor_t,
}

const _: () = assert!(core::mem::size_of::<MachMessageWithOneOutOfLineDescriptor>() == 44);
const _: () =
    assert!(core::mem::offset_of!(MachMessageWithOneOutOfLineDescriptor, descriptor_count) == 24);
const _: () =
    assert!(core::mem::offset_of!(MachMessageWithOneOutOfLineDescriptor, descriptor) == 28);

pub fn send_bytes_out_of_line_to_mach_port(port: libc::mach_port_t, data: *mut c_void, size: u32) {
    let mut message = MachMessageWithOneOutOfLineDescriptor {
        header: mach_msg_header_t {
            msgh_bits: (MACH_MSG_TYPE_COPY_SEND & MACH_MSGH_BITS_REMOTE_MASK)
                | MACH_MSGH_BITS_COMPLEX,
            msgh_size: core::mem::size_of::<MachMessageWithOneOutOfLineDescriptor>()
                as mach_msg_size_t,
            msgh_remote_port: port,
            msgh_local_port: 0,
            msgh_voucher_port: 0,
            msgh_id: 0,
        },
        descriptor_count: 1,
        descriptor: mach_msg_ool_descriptor_t {
            address: data,
            deallocate: 0,
            copy: MACH_MSG_VIRTUAL_COPY,
            pad1: 0,
            descriptor_type: MACH_MSG_OOL_DESCRIPTOR,
            size,
        },
    };

    unsafe {
        mach_msg(
            (&raw mut message).cast::<mach_msg_header_t>(),
            MACH_SEND_MSG,
            core::mem::size_of::<MachMessageWithOneOutOfLineDescriptor>() as mach_msg_size_t,
            0,
            0,
            0,
            0,
        );
    }
}
