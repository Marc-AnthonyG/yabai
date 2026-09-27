use core::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use crate::ffi::libsystem::{PROC_PIDPATHINFO_MAXSIZE, proc_name};
use crate::ffi::mach_port::{bootstrap_look_up, mach_port_deallocate, mach_send, mach_task_self};
use crate::ffi::skylight::SLSConnectionGetPID;
use crate::state::process_wide::BOOTSTRAP_PORT;
use crate::window::animation::WindowAnimation;

#[repr(C)]
pub(crate) struct JankyBordersEvent {
    pub(crate) event: u32,
    pub(crate) count: u32,
    pub(crate) proxy_window_id: [u32; 512],
    pub(crate) real_window_id: [u32; 512],
}

const _: () = assert!(core::mem::size_of::<JankyBordersEvent>() == 4104);

pub(crate) fn window_manager_notify_jankyborders(
    animation_list: &[WindowAnimation],
    event: u32,
    skip: bool,
    wait: bool,
) {
    let bootstrap_port = *BOOTSTRAP_PORT.get().unwrap_or(&0);
    let mut port: libc::mach_port_t = 0;
    if bootstrap_port != 0
        && unsafe { bootstrap_look_up(bootstrap_port, c"git.felix.jbevent".as_ptr(), &mut port) }
            == libc::KERN_SUCCESS
    {
        let mut data = JankyBordersEvent {
            event,
            count: 0,
            proxy_window_id: [0; 512],
            real_window_id: [0; 512],
        };

        for index in 0..animation_list.len() {
            if skip && animation_list[index].skip.load(Ordering::Relaxed) {
                continue;
            }

            if data.count as usize >= data.proxy_window_id.len() {
                break;
            }

            data.proxy_window_id[data.count as usize] =
                animation_list[index].proxy.id.load(Ordering::Relaxed);
            data.real_window_id[data.count as usize] = animation_list[index].window_id.0;

            data.count += 1;
        }

        mach_send(
            port,
            (&mut data as *mut JankyBordersEvent).cast::<c_void>(),
            core::mem::size_of::<JankyBordersEvent>() as u32,
        );
        unsafe { mach_port_deallocate(mach_task_self(), port) };
        if wait {
            unsafe { libc::usleep(20000) };
        }
    }
}

pub(crate) fn window_manager_window_connection_is_jankyborders(window_connection_id: i32) -> bool {
    static PROCESS_NAME: Mutex<[u8; PROC_PIDPATHINFO_MAXSIZE]> =
        Mutex::new([0u8; PROC_PIDPATHINFO_MAXSIZE]);
    let mut process_name = PROCESS_NAME.lock().unwrap();

    let mut window_process_id: libc::pid_t = 0;
    unsafe { SLSConnectionGetPID(window_connection_id, &mut window_process_id) };
    unsafe {
        proc_name(
            window_process_id,
            process_name.as_mut_ptr().cast::<c_void>(),
            process_name.len() as u32,
        )
    };

    let end = process_name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(process_name.len());
    &process_name[..end] == b"borders"
}
