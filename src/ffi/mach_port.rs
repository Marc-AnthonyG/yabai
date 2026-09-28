use core::ffi::c_void;

pub use mach2::bootstrap::bootstrap_look_up;
pub use mach2::mach_port::mach_port_deallocate;
pub use mach2::message::{
    MACH_MSG_TYPE_COPY_SEND, MACH_MSG_VIRTUAL_COPY, MACH_MSGH_BITS_COMPLEX,
    MACH_MSGH_BITS_REMOTE_MASK, MACH_RCV_MSG, MACH_SEND_MSG, mach_msg, mach_msg_header_t,
    mach_msg_ool_descriptor_t, mach_msg_size_t,
};
pub use mach2::ndr::{NDR_record, NDR_record_t};
pub use mach2::task::task_get_special_port;
pub use mach2::task_special_ports::TASK_BOOTSTRAP_PORT;
pub use mach2::traps::mach_task_self;

unsafe extern "C" {
    pub fn mig_get_special_reply_port() -> libc::mach_port_t;
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
            ..mach_msg_header_t::default()
        },
        descriptor_count: 1,
        descriptor: mach_msg_ool_descriptor_t::new(data, false, MACH_MSG_VIRTUAL_COPY, size),
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
