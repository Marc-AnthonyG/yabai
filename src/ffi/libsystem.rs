use core::ffi::{c_char, c_int, c_void};
use libc::pid_t;

unsafe extern "C" {
    pub fn csr_get_active_config(config: *mut u32) -> c_int;

    pub fn proc_name(process_id: c_int, buffer: *mut c_void, buffer_size: u32) -> c_int;
    pub fn _NSGetExecutablePath(buffer: *mut c_char, buffer_size: *mut u32) -> c_int;
}

pub const CSR_ALLOW_UNRESTRICTED_FS: u32 = 0x02;
pub const CSR_ALLOW_TASK_FOR_PID: u32 = 0x04;

pub const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;

pub const P_TRACED: i32 = 0x0000_0800;
pub const SIZE_OF_KINFO_PROC: usize = 648;
pub const OFFSET_OF_P_FLAG_IN_KINFO_PROC: usize = 32;

pub fn process_is_being_debugged(process_id: pid_t) -> bool {
    let mut process_information = [0u8; SIZE_OF_KINFO_PROC];
    let mut size = core::mem::size_of_val(&process_information);
    let mut management_information_base: [c_int; 4] = [
        libc::CTL_KERN,
        libc::KERN_PROC,
        libc::KERN_PROC_PID,
        process_id,
    ];

    unsafe {
        libc::sysctl(
            management_information_base.as_mut_ptr(),
            management_information_base.len() as u32,
            process_information.as_mut_ptr().cast::<c_void>(),
            &mut size,
            core::ptr::null_mut(),
            0,
        );
    }

    let p_flag = i32::from_ne_bytes(
        process_information[OFFSET_OF_P_FLAG_IN_KINFO_PROC..OFFSET_OF_P_FLAG_IN_KINFO_PROC + 4]
            .try_into()
            .unwrap(),
    );
    (p_flag & P_TRACED) != 0
}
