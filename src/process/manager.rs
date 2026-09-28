use core::ffi::c_ulong;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::ffi::carbon_events::{
    EventHandlerRef, EventHandlerUPP, EventTypeSpec, GetApplicationEventTarget,
    GetCurrentEventTime, InstallEventHandler, kEventAppFrontSwitched, kEventAppLaunched,
    kEventAppTerminated, kEventClassApplication, noErr,
};
use crate::ffi::carbon_process::{GetNextProcess, GetProcessPID, ProcessSerialNumber, kNoProcess};
use crate::ffi::libsystem::is_process_being_debugged;
use crate::ffi::skylight::_SLPSGetFrontProcess;
use crate::notifications::process::handle_carbon_application_event_callback;
use crate::process::model::{
    Process, create_process_unless_it_is_ignored, query_process_id_of_process_serial_number,
};
use crate::support::handles::ProcessId;
use crate::support::strings::are_both_strings_present_and_equal;

pub(crate) struct ProcessManager {
    pub(crate) front_process_id: ProcessId,
    pub(crate) last_front_process_id: ProcessId,
    pub(crate) switch_event_time: f64,
    pub(crate) finder_process_serial_number: ProcessSerialNumber,
}

pub(crate) static PROCESS_TABLE: Mutex<BTreeMap<ProcessSerialNumber, Arc<Process>>> =
    Mutex::new(BTreeMap::new());

pub(crate) fn add_every_running_process_to_the_process_table(process_manager: &mut ProcessManager) {
    let mut process_serial_number = ProcessSerialNumber {
        high_long_of_psn: kNoProcess,
        low_long_of_psn: kNoProcess,
    };
    while unsafe { GetNextProcess(&mut process_serial_number) } == noErr as i16 {
        let process_id = query_process_id_of_process_serial_number(process_serial_number);
        if is_process_being_debugged(process_id.0) {
            crate::debug!(
                "{}: process with pid {} is running under a debugger! ignoring..\n",
                "add_every_running_process_to_the_process_table",
                process_id.0
            );
            continue;
        }

        let Some(process) = create_process_unless_it_is_ignored(process_serial_number, process_id)
        else {
            continue;
        };

        if are_both_strings_present_and_equal(Some(&process.name), Some("Finder")) {
            crate::debug!(
                "{}: {} ({}) was found! caching psn..\n",
                "add_every_running_process_to_the_process_table",
                process.name,
                process.process_id.0
            );
            process_manager.finder_process_serial_number = process_serial_number;
        }

        PROCESS_TABLE
            .lock()
            .unwrap()
            .entry(process.process_serial_number)
            .or_insert(process);
    }
}

pub(crate) fn start_process_manager_observing_application_events(
    process_manager: &mut ProcessManager,
) -> bool {
    let target = unsafe { GetApplicationEventTarget() };
    let handler = handle_carbon_application_event_callback as EventHandlerUPP;
    let event_type: [EventTypeSpec; 3] = [
        EventTypeSpec {
            event_class: kEventClassApplication,
            event_kind: kEventAppLaunched,
        },
        EventTypeSpec {
            event_class: kEventClassApplication,
            event_kind: kEventAppTerminated,
        },
        EventTypeSpec {
            event_class: kEventClassApplication,
            event_kind: kEventAppFrontSwitched,
        },
    ];

    objc2::rc::autoreleasepool(|_| add_every_running_process_to_the_process_table(process_manager));

    let mut front_process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };
    unsafe { _SLPSGetFrontProcess(&mut front_process_serial_number) };
    unsafe {
        GetProcessPID(
            &front_process_serial_number,
            &mut process_manager.front_process_id.0,
        )
    };
    process_manager.last_front_process_id = process_manager.front_process_id;
    process_manager.switch_event_time = unsafe { GetCurrentEventTime() };

    let mut handler_ref: EventHandlerRef = std::ptr::null_mut();
    let installed = unsafe {
        InstallEventHandler(
            target,
            handler,
            event_type.len() as c_ulong,
            event_type.as_ptr(),
            std::ptr::null_mut(),
            &mut handler_ref,
        )
    } == noErr;

    installed
}

pub(crate) fn process_with_process_serial_number(
    process_serial_number: &ProcessSerialNumber,
) -> Option<Arc<Process>> {
    PROCESS_TABLE
        .lock()
        .unwrap()
        .get(process_serial_number)
        .map(Arc::clone)
}
