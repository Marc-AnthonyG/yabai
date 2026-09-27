#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::Mutex;

use crate::event::queue::{Event, event_loop_post};
use crate::ffi::accessibility::{
    AXObserver, AXObserverAddNotification, AXObserverCreate, AXObserverGetRunLoopSource,
    AXObserverRef, AXObserverRemoveNotification, AXUIElement, AXUIElementCreateApplication,
    AXUIElementRef, kAXErrorSuccess, kAXExposeExit, kAXExposeShowAllWindows, kAXExposeShowDesktop,
    kAXExposeShowFrontWindows,
};
use crate::ffi::appkit::NSRunningApplication;
use crate::ffi::core_foundation::{
    CFEqual, CFRetained, CFRunLoopAddSource, CFRunLoopGetMain, CFRunLoopSourceInvalidate, CFString,
    as_cftype, kCFRunLoopDefaultMode, take_create_rule_result,
};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::ffi::foundation::NSString;
use crate::support::handles::ProcessId;

pub(crate) struct MissionControlObserver {
    pub(crate) element_ref: AXUIElementRef,
    pub(crate) observer_ref: AXObserverRef,
}

unsafe impl Send for MissionControlObserver {}

pub(crate) static MISSION_CONTROL_OBSERVER: Mutex<Option<MissionControlObserver>> =
    Mutex::new(None);

pub(crate) unsafe extern "C-unwind" fn mission_control_notification_handler(
    _observer: NonNull<AXObserver>,
    _element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    _context: *mut c_void,
) {
    let notification = as_cftype(unsafe { notification.as_ref() });

    if CFEqual(
        Some(notification),
        Some(as_cftype(kAXExposeShowAllWindows())),
    ) {
        event_loop_post(Event::MissionControlShowAllWindows);
    } else if CFEqual(
        Some(notification),
        Some(as_cftype(kAXExposeShowFrontWindows())),
    ) {
        event_loop_post(Event::MissionControlShowFrontWindows);
    } else if CFEqual(Some(notification), Some(as_cftype(kAXExposeShowDesktop()))) {
        event_loop_post(Event::MissionControlShowDesktop);
    } else if CFEqual(Some(notification), Some(as_cftype(kAXExposeExit()))) {
        event_loop_post(Event::MissionControlExit);
    }
}

pub(crate) fn mission_control_observe() {
    let mut mission_control_observer = MISSION_CONTROL_OBSERVER.lock().unwrap();

    if mission_control_observer.is_none() {
        let process_id: u32 = workspace_get_dock_pid().0 as u32;
        let element = unsafe { AXUIElementCreateApplication(process_id as libc::pid_t) };

        if process_id != 0 {
            let mut observer_ref: AXObserverRef = core::ptr::null_mut();

            if unsafe {
                AXObserverCreate(
                    process_id as libc::pid_t,
                    Some(mission_control_notification_handler),
                    NonNull::from(&mut observer_ref),
                )
            } == kAXErrorSuccess
            {
                let observer: &AXObserver = unsafe { &*observer_ref };

                unsafe {
                    AXObserverAddNotification(
                        observer,
                        &element,
                        kAXExposeShowAllWindows(),
                        core::ptr::null_mut(),
                    );
                    AXObserverAddNotification(
                        observer,
                        &element,
                        kAXExposeShowFrontWindows(),
                        core::ptr::null_mut(),
                    );
                    AXObserverAddNotification(
                        observer,
                        &element,
                        kAXExposeShowDesktop(),
                        core::ptr::null_mut(),
                    );
                    AXObserverAddNotification(
                        observer,
                        &element,
                        kAXExposeExit(),
                        core::ptr::null_mut(),
                    );
                }

                *mission_control_observer = Some(MissionControlObserver {
                    element_ref: CFRetained::into_raw(element).as_ptr().cast_const(),
                    observer_ref,
                });

                if let Some(main_run_loop) = CFRunLoopGetMain() {
                    let run_loop_source = unsafe { AXObserverGetRunLoopSource(observer) };
                    CFRunLoopAddSource(&main_run_loop, Some(&run_loop_source), unsafe {
                        kCFRunLoopDefaultMode
                    });
                }
            }
        }
    }
}

pub(crate) fn mission_control_unobserve() {
    let Some(mission_control_observer) = MISSION_CONTROL_OBSERVER.lock().unwrap().take() else {
        return;
    };

    dispatch_after_on_main_queue(0, move || {
        let observer: &AXObserver = unsafe { &*mission_control_observer.observer_ref };
        let element: &AXUIElement = unsafe { &*mission_control_observer.element_ref };

        unsafe {
            AXObserverRemoveNotification(observer, element, kAXExposeShowAllWindows());
            AXObserverRemoveNotification(observer, element, kAXExposeShowFrontWindows());
            AXObserverRemoveNotification(observer, element, kAXExposeShowDesktop());
            AXObserverRemoveNotification(observer, element, kAXExposeExit());
        }

        let run_loop_source = unsafe { AXObserverGetRunLoopSource(observer) };
        CFRunLoopSourceInvalidate(&run_loop_source);
        drop(unsafe {
            take_create_rule_result(mission_control_observer.observer_ref.cast_const())
        });
        drop(unsafe { take_create_rule_result(mission_control_observer.element_ref) });
    });
}

pub(crate) fn workspace_get_dock_pid() -> ProcessId {
    let list =
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
            "com.apple.dock",
        ));

    if list.count() == 1 {
        let dock = list.objectAtIndexedSubscript(0);
        return ProcessId(dock.processIdentifier());
    }

    ProcessId(0)
}
