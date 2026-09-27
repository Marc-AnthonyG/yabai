use core::ffi::{c_ulong, c_void};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering, compiler_fence};
use std::sync::{Arc, Mutex, OnceLock};

use crate::display::display_space_list;
use crate::display_manager::display_manager_active_display_list;
use crate::event_loop::{Event, event_loop_post};
use crate::ffi::carbon_events::{
    EventHandlerCallRef, EventHandlerRef, EventHandlerUPP, EventRef, EventTypeSpec,
    GetApplicationEventTarget, GetCurrentEventTime, GetEventKind, GetEventParameter,
    InstallEventHandler, OSStatus, kEventAppFrontSwitched, kEventAppLaunched, kEventAppTerminated,
    kEventClassApplication, kEventParamProcessID, noErr, typeProcessSerialNumber,
};
use crate::ffi::carbon_process::{
    CopyProcessName, GetNextProcess, GetProcessInformation, GetProcessPID, ProcessInfoRec,
    ProcessSerialNumber, kNoProcess,
};
use crate::ffi::core_foundation::{
    CFString, cfarray_count, cfarray_of_cfnumbers, cfstring_copy, kCFNumberSInt64Type,
    take_create_rule_result,
};
use crate::ffi::libsystem::process_is_being_debugged;
use crate::ffi::skylight::{
    SLSCopyWindowsWithOptionsAndTags, SLSWindowIteratorAdvance, SLSWindowIteratorGetAttributes,
    SLSWindowIteratorGetLevel, SLSWindowIteratorGetParentID, SLSWindowIteratorGetTags,
    SLSWindowIteratorGetWindowID, SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows,
    _SLPSGetFrontProcess,
};
use crate::globals::CONNECTION;
use crate::handles::{ProcessId, SpaceId, WindowId};
use crate::support::strings::string_equals;
use crate::support::table::Table;
use crate::window::model::window_space;
use crate::workspace::{
    WORKSPACE_CONTEXT, workspace_application_create_running_ns_application,
    workspace_application_destroy_running_ns_application, workspace_application_unobserve,
};

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

pub(crate) struct ProcessManager {
    pub(crate) front_process_id: ProcessId,
    pub(crate) last_front_process_id: ProcessId,
    pub(crate) switch_event_time: f64,
    pub(crate) finder_process_serial_number: ProcessSerialNumber,
}

pub(crate) static PROCESS_TABLE: OnceLock<Mutex<Table<ProcessSerialNumber, Arc<Process>>>> =
    OnceLock::new();

pub(crate) const PROCESS_NAME_BLACKLIST: [&str; 4] = [
    "Übersicht",
    "Slack Helper (Plugin)",
    "Google Chrome Helper (Plugin)",
    "qlmanage",
];

pub(crate) fn hash_process_serial_number(key: &ProcessSerialNumber) -> u64 {
    key.low_long_of_psn as u64
}

pub(crate) fn process_pid_for_psn(process_serial_number: ProcessSerialNumber) -> ProcessId {
    let mut process_id: libc::pid_t = 0;
    unsafe { GetProcessPID(&process_serial_number, &mut process_id) };
    ProcessId(process_id)
}

pub(crate) fn process_create(
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
            "process_create"
        );
        return None;
    };

    let process_name = cfstring_copy(&process_name_ref);
    drop(process_name_ref);

    let process_name = process_name?;

    if { process_info.process_type } == 0x5850_4321 {
        crate::debug!(
            "{}: xpc service '{}' detected! ignoring..\n",
            "process_create",
            process_name
        );
        return None;
    }

    for blacklisted_process_name in PROCESS_NAME_BLACKLIST {
        if string_equals(Some(&process_name), Some(blacklisted_process_name)) {
            crate::debug!(
                "{}: {} is blacklisted! ignoring..\n",
                "process_create",
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
        workspace_application_create_running_ns_application(&process),
        Ordering::Release,
    );
    Some(process)
}

pub(crate) fn process_manager_active_space_for_psn(connection: i32) -> SpaceId {
    let mut space_id = SpaceId(0);

    let display_list = display_manager_active_display_list();

    let mut space_list: Vec<u64> = Vec::new();

    for display_id in display_list {
        let Some(list) = display_space_list(display_id) else {
            continue;
        };

        //
        // NOTE(asmvik): display_space_list(..) uses a linear allocator,
        // and so we only need to track the beginning of the first list along
        // with the total number of windows that have been allocated.
        //

        space_list.extend(list.iter().map(|space_id| space_id.0));
    }

    let mut set_tags: u64 = 0;
    let mut clear_tags: u64 = 0;
    let options: u32 = 0x2;

    let space_list_ref = cfarray_of_cfnumbers(&space_list, kCFNumberSInt64Type);
    let window_list_ref = unsafe {
        SLSCopyWindowsWithOptionsAndTags(
            *CONNECTION.get().unwrap(),
            connection as u32,
            &*space_list_ref,
            options,
            &mut set_tags,
            &mut clear_tags,
        )
    };
    let Some(window_list_ref) = (unsafe { take_create_rule_result(window_list_ref) }) else {
        return space_id;
    };

    let count = cfarray_count(&window_list_ref) as i32;
    if count == 0 {
        return space_id;
    }

    let query = unsafe {
        take_create_rule_result(SLSWindowQueryWindows(
            *CONNECTION.get().unwrap(),
            &*window_list_ref,
            count,
        ))
    };
    let Some(query) = query else {
        return space_id;
    };

    let iterator = unsafe { take_create_rule_result(SLSWindowQueryResultCopyWindows(&*query)) };
    let Some(iterator) = iterator else {
        return space_id;
    };

    while unsafe { SLSWindowIteratorAdvance(&*iterator) } {
        let tags = unsafe { SLSWindowIteratorGetTags(&*iterator) };
        let attributes = unsafe { SLSWindowIteratorGetAttributes(&*iterator) };
        let parent_window_id = unsafe { SLSWindowIteratorGetParentID(&*iterator) };
        let window_id = unsafe { SLSWindowIteratorGetWindowID(&*iterator) };
        let level = unsafe { SLSWindowIteratorGetLevel(&*iterator) };

        if parent_window_id == 0 {
            if level == 0 || level == 3 || level == 8 {
                if ((attributes & 0x2) != 0 || (tags & 0x400000000000000) != 0)
                    && ((tags & 0x1) != 0 || ((tags & 0x2) != 0 && (tags & 0x80000000) != 0))
                {
                    space_id = window_space(WindowId(window_id));
                    break;
                }
            }
        }
    }

    drop(query);
    drop(iterator);
    space_id
}

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

pub(crate) fn process_destroy(process: Arc<Process>) {
    workspace_application_destroy_running_ns_application(
        WORKSPACE_CONTEXT.get().unwrap(),
        &process,
    );
}
