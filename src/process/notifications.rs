use core::ffi::{c_ulong, c_void};
use std::sync::Arc;
use std::sync::atomic::{Ordering, compiler_fence};

use crate::event_loop::{Event, event_loop_post};
use crate::ffi::carbon_events::{
    EventHandlerCallRef, EventRef, GetEventKind, GetEventParameter, OSStatus,
    kEventAppFrontSwitched, kEventAppLaunched, kEventAppTerminated, kEventParamProcessID, noErr,
    typeProcessSerialNumber,
};
use crate::ffi::carbon_process::ProcessSerialNumber;
use crate::ffi::libsystem::process_is_being_debugged;
use crate::process::manager::{PROCESS_TABLE, process_manager_find_process};
use crate::process::model::{process_create, process_pid_for_psn};
use crate::workspace::{WORKSPACE_CONTEXT, workspace_application_unobserve};

#[allow(non_upper_case_globals)]
pub(crate) unsafe extern "C-unwind" fn process_handler(
    _handler_call_ref: EventHandlerCallRef,
    event: EventRef,
    _context: *mut c_void,
) -> OSStatus {
    let mut process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };
    if unsafe {
        GetEventParameter(
            event,
            kEventParamProcessID,
            typeProcessSerialNumber,
            std::ptr::null_mut(),
            std::mem::size_of::<ProcessSerialNumber>() as c_ulong,
            std::ptr::null_mut(),
            (&mut process_serial_number as *mut ProcessSerialNumber).cast::<c_void>(),
        )
    } != noErr
    {
        return -1;
    }

    match unsafe { GetEventKind(event) } {
        kEventAppLaunched => {
            if process_manager_find_process(&process_serial_number).is_some() {
                //
                // NOTE(asmvik): Some garbage applications (e.g Steam) are reported twice with the same PID and PSN for some hecking reason.
                // It is by definition NOT possible for two processes to exist at the same time with the same PID and PSN.
                // If we detect such a scenario we simply discard the dupe notification..
                //

                return noErr;
            }

            let process_id = process_pid_for_psn(process_serial_number);
            if process_is_being_debugged(process_id.0) {
                crate::debug!(
                    "{}: process with pid {} is running under a debugger! ignoring..\n",
                    "process_handler",
                    process_id.0
                );
                return noErr;
            }

            let Some(process) = process_create(process_serial_number, process_id) else {
                return noErr;
            };

            PROCESS_TABLE
                .get()
                .unwrap()
                .lock()
                .unwrap()
                .add(process.process_serial_number, Arc::clone(&process));
            event_loop_post(Event::ApplicationLaunched(process));
        }
        kEventAppTerminated => {
            let Some(process) = process_manager_find_process(&process_serial_number) else {
                return noErr;
            };

            process.terminated.store(true, Ordering::Release);
            PROCESS_TABLE
                .get()
                .unwrap()
                .lock()
                .unwrap()
                .remove(&process_serial_number);
            workspace_application_unobserve(WORKSPACE_CONTEXT.get().unwrap(), &process);
            compiler_fence(Ordering::SeqCst);

            event_loop_post(Event::ApplicationTerminated(process));
        }
        kEventAppFrontSwitched => {
            let Some(process) = process_manager_find_process(&process_serial_number) else {
                return noErr;
            };

            event_loop_post(Event::ApplicationFrontSwitched(process));
        }
        _ => {}
    }

    noErr
}
