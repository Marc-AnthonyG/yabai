use core::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use objc2::rc::Retained;

use crate::ffi::appkit::{NSApplicationActivationPolicy, NSRunningApplication};
use crate::process::model::Process;

pub(crate) fn copy_running_application_of_process(process: &Arc<Process>) -> *mut c_void {
    match NSRunningApplication::runningApplicationWithProcessIdentifier(process.process_id.0) {
        Some(application) => Retained::into_raw(application).cast::<c_void>(),
        None => core::ptr::null_mut(),
    }
}

pub(crate) fn is_process_observable_refreshing_its_activation_policy(
    process: &Arc<Process>,
) -> bool {
    let application = process.ns_application.load(Ordering::Acquire);
    if let Some(application) = unsafe { application.cast::<NSRunningApplication>().as_ref() } {
        process
            .policy
            .store(application.activationPolicy().0 as i32, Ordering::Relaxed);
        process.policy.load(Ordering::Relaxed) as isize == NSApplicationActivationPolicy::Regular.0
    } else {
        process.policy.store(
            NSApplicationActivationPolicy::Prohibited.0 as i32,
            Ordering::Relaxed,
        );
        false
    }
}

pub(crate) fn has_process_finished_launching(process: &Arc<Process>) -> bool {
    let application = process.ns_application.load(Ordering::Acquire);
    if let Some(application) = unsafe { application.cast::<NSRunningApplication>().as_ref() } {
        application.isFinishedLaunching()
    } else {
        false
    }
}
