use core::ffi::{c_ulong, c_void};
use std::sync::Arc;
use std::sync::atomic::{Ordering, compiler_fence};

use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::carbon_events::{
    EventHandlerCallRef, EventRef, GetEventKind, GetEventParameter, OSStatus,
    kEventAppFrontSwitched, kEventAppLaunched, kEventAppTerminated, kEventParamProcessID, noErr,
    typeProcessSerialNumber,
};
use crate::ffi::carbon_process::ProcessSerialNumber;
use crate::ffi::libsystem::is_process_being_debugged;
use crate::notifications::workspace::{
    WORKSPACE_CONTEXT, stop_observing_application_launch_and_activation_policy,
};
use crate::process::manager::{PROCESS_TABLE, process_with_process_serial_number};
use crate::process::model::{
    create_process_unless_it_is_ignored, query_process_id_of_process_serial_number,
};

#[allow(non_upper_case_globals)]
pub(crate) unsafe extern "C-unwind" fn handle_carbon_application_event_callback(
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
            if process_with_process_serial_number(&process_serial_number).is_some() {
                //
                // NOTE(asmvik): Some garbage applications (e.g Steam) are reported twice with the same PID and PSN for some hecking reason.
                // It is by definition NOT possible for two processes to exist at the same time with the same PID and PSN.
                // If we detect such a scenario we simply discard the dupe notification..
                //

                return noErr;
            }

            let process_id = query_process_id_of_process_serial_number(process_serial_number);
            if is_process_being_debugged(process_id.0) {
                crate::debug!(
                    "{}: process with pid {} is running under a debugger! ignoring..\n",
                    "handle_carbon_application_event_callback",
                    process_id.0
                );
                return noErr;
            }

            let Some(process) =
                create_process_unless_it_is_ignored(process_serial_number, process_id)
            else {
                return noErr;
            };

            PROCESS_TABLE
                .lock()
                .unwrap()
                .entry(process.process_serial_number)
                .or_insert(Arc::clone(&process));
            post_event_to_event_loop(Event::ApplicationLaunched(process));
        }
        kEventAppTerminated => {
            let Some(process) = process_with_process_serial_number(&process_serial_number) else {
                return noErr;
            };

            process.terminated.store(true, Ordering::Release);
            PROCESS_TABLE.lock().unwrap().remove(&process_serial_number);
            stop_observing_application_launch_and_activation_policy(
                WORKSPACE_CONTEXT.get().unwrap(),
                &process,
            );
            compiler_fence(Ordering::SeqCst);

            post_event_to_event_loop(Event::ApplicationTerminated(process));
        }
        kEventAppFrontSwitched => {
            let Some(process) = process_with_process_serial_number(&process_serial_number) else {
                return noErr;
            };

            post_event_to_event_loop(Event::ApplicationFrontSwitched(process));
        }
        _ => {}
    }

    noErr
}
