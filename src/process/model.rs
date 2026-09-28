use core::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering};

use crate::ffi::carbon_process::{
    CopyProcessName, GetProcessInformation, GetProcessPID, ProcessInfoRec, ProcessSerialNumber,
};
use crate::ffi::core_foundation::{CFString, take_create_rule_result};
use crate::notifications::workspace::{
    WORKSPACE_CONTEXT, release_running_application_removing_its_observations,
};
use crate::process::running_application::copy_running_application_of_process;
use crate::support::handles::ProcessId;
use crate::support::strings::are_both_strings_present_and_equal;

pub(crate) struct Process {
    pub(crate) process_serial_number: ProcessSerialNumber,
    pub(crate) process_id: ProcessId,
    pub(crate) name: Arc<str>,
    pub(crate) ns_application: AtomicPtr<c_void>,
    pub(crate) policy: AtomicI32,
    pub(crate) terminated: AtomicBool,
}

unsafe impl Send for Process {}
unsafe impl Sync for Process {}

pub(crate) const PROCESS_NAME_BLACKLIST: [&str; 4] = [
    "Übersicht",
    "Slack Helper (Plugin)",
    "Google Chrome Helper (Plugin)",
    "qlmanage",
];

pub(crate) fn query_process_id_of_process_serial_number(
    process_serial_number: ProcessSerialNumber,
) -> ProcessId {
    let mut process_id: libc::pid_t = 0;
    unsafe { GetProcessPID(&process_serial_number, &mut process_id) };
    ProcessId(process_id)
}

pub(crate) fn create_process_unless_it_is_ignored(
    process_serial_number: ProcessSerialNumber,
    process_id: ProcessId,
) -> Option<Arc<Process>> {
    let mut process_info: ProcessInfoRec = unsafe { std::mem::zeroed() };
    process_info.process_info_length = std::mem::size_of::<ProcessInfoRec>() as u32;
    unsafe { GetProcessInformation(&process_serial_number, &mut process_info) };

    let mut process_name_ref: *mut CFString = std::ptr::null_mut();
    unsafe { CopyProcessName(&process_serial_number, &mut process_name_ref) };

    let Some(process_name_ref) = (unsafe { take_create_rule_result(process_name_ref) }) else {
        crate::debug!(
            "{}: could not retrieve process name! ignoring..\n",
            "create_process_unless_it_is_ignored"
        );
        return None;
    };

    let process_name = process_name_ref.to_string();

    if { process_info.process_type } == 0x5850_4321 {
        crate::debug!(
            "{}: xpc service '{}' detected! ignoring..\n",
            "create_process_unless_it_is_ignored",
            process_name
        );
        return None;
    }

    for blacklisted_process_name in PROCESS_NAME_BLACKLIST {
        if are_both_strings_present_and_equal(Some(&process_name), Some(blacklisted_process_name)) {
            crate::debug!(
                "{}: {} is blacklisted! ignoring..\n",
                "create_process_unless_it_is_ignored",
                process_name
            );
            return None;
        }
    }

    let process = Arc::new(Process {
        process_serial_number,
        process_id,
        name: process_name.into(),
        ns_application: AtomicPtr::new(std::ptr::null_mut()),
        policy: AtomicI32::new(0),
        terminated: AtomicBool::new(false),
    });
    process.ns_application.store(
        copy_running_application_of_process(&process),
        Ordering::Release,
    );
    Some(process)
}

pub(crate) fn destroy_process_releasing_its_running_application(process: Arc<Process>) {
    release_running_application_removing_its_observations(
        WORKSPACE_CONTEXT.get().unwrap(),
        &process,
    );
}
