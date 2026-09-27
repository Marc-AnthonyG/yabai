use core::ffi::c_ulong;
use std::sync::{Arc, Mutex, OnceLock};

use crate::ffi::carbon_events::{
    EventHandlerRef, EventHandlerUPP, EventTypeSpec, GetApplicationEventTarget,
    GetCurrentEventTime, InstallEventHandler, kEventAppFrontSwitched, kEventAppLaunched,
    kEventAppTerminated, kEventClassApplication, noErr,
};
use crate::ffi::carbon_process::{GetNextProcess, GetProcessPID, ProcessSerialNumber, kNoProcess};
use crate::ffi::libsystem::process_is_being_debugged;
use crate::ffi::skylight::_SLPSGetFrontProcess;
use crate::handles::ProcessId;
use crate::process::model::{Process, process_create, process_pid_for_psn};
use crate::process::notifications::process_handler;
use crate::support::strings::string_equals;
use crate::support::table::Table;

pub(crate) struct ProcessManager {
    pub(crate) front_process_id: ProcessId,
    pub(crate) last_front_process_id: ProcessId,
    pub(crate) switch_event_time: f64,
    pub(crate) finder_process_serial_number: ProcessSerialNumber,
}

pub(crate) static PROCESS_TABLE: OnceLock<Mutex<Table<ProcessSerialNumber, Arc<Process>>>> =
    OnceLock::new();

pub(crate) fn hash_process_serial_number(key: &ProcessSerialNumber) -> u64 {
    key.low_long_of_psn as u64
}

pub(crate) fn process_manager_add_running_processes(process_manager: &mut ProcessManager) {
    let mut process_serial_number = ProcessSerialNumber {
        high_long_of_psn: kNoProcess,
        low_long_of_psn: kNoProcess,
    };
    while unsafe { GetNextProcess(&mut process_serial_number) } == noErr as i16 {
        let process_id = process_pid_for_psn(process_serial_number);
        if process_is_being_debugged(process_id.0) {
            crate::debug!(
                "{}: process with pid {} is running under a debugger! ignoring..\n",
                "process_manager_add_running_processes",
                process_id.0
            );
            continue;
        }

        let Some(process) = process_create(process_serial_number, process_id) else {
            continue;
        };

        if string_equals(Some(&process.name), Some("Finder")) {
            crate::debug!(
                "{}: {} ({}) was found! caching psn..\n",
                "process_manager_add_running_processes",
                process.name,
                process.process_id.0
            );
            process_manager.finder_process_serial_number = process_serial_number;
        }

        PROCESS_TABLE
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .add(process.process_serial_number, process);
    }
}

pub(crate) fn process_manager_begin(process_manager: &mut ProcessManager) -> bool {
    let target = unsafe { GetApplicationEventTarget() };
    let handler = process_handler as EventHandlerUPP;
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
    PROCESS_TABLE.get_or_init(|| Mutex::new(Table::new(125, hash_process_serial_number)));

    objc2::rc::autoreleasepool(|_| process_manager_add_running_processes(process_manager));

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

pub(crate) fn process_manager_find_process(
    process_serial_number: &ProcessSerialNumber,
) -> Option<Arc<Process>> {
    PROCESS_TABLE
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .find(process_serial_number)
        .map(Arc::clone)
}
