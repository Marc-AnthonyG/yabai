#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::Mutex;

use crate::event::queue::{Event, post_event_to_event_loop};
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

pub(crate) unsafe extern "C-unwind" fn handle_mission_control_accessibility_notification_callback(
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
        post_event_to_event_loop(Event::MissionControlShowAllWindows);
    } else if CFEqual(
        Some(notification),
        Some(as_cftype(kAXExposeShowFrontWindows())),
    ) {
        post_event_to_event_loop(Event::MissionControlShowFrontWindows);
    } else if CFEqual(Some(notification), Some(as_cftype(kAXExposeShowDesktop()))) {
        post_event_to_event_loop(Event::MissionControlShowDesktop);
    } else if CFEqual(Some(notification), Some(as_cftype(kAXExposeExit()))) {
        post_event_to_event_loop(Event::MissionControlExit);
    }
}

pub(crate) fn start_observing_mission_control_through_the_dock() {
    let mut mission_control_observer = MISSION_CONTROL_OBSERVER.lock().unwrap();

    if mission_control_observer.is_none() {
        let process_id: u32 = find_dock_process_id().0 as u32;
        let element = unsafe { AXUIElementCreateApplication(process_id as libc::pid_t) };

        if process_id != 0 {
            let mut observer_ref: AXObserverRef = core::ptr::null_mut();

            if unsafe {
                AXObserverCreate(
                    process_id as libc::pid_t,
                    Some(handle_mission_control_accessibility_notification_callback),
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

pub(crate) fn stop_observing_mission_control_through_the_dock() {
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

pub(crate) fn find_dock_process_id() -> ProcessId {
    let list = NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
        "com.apple.dock",
    ));

    if list.count() == 1 {
        let dock = list.objectAtIndexedSubscript(0);
        return ProcessId(dock.processIdentifier());
    }

    ProcessId(0)
}
