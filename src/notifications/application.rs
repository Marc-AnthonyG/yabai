#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;

use crate::application::Application;
use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::CFStringOwned;
use crate::ffi::accessibility::{
    AXError, AXObserver, AXObserverAddNotification, AXObserverCreate, AXObserverGetRunLoopSource,
    AXObserverRemoveNotification, AXUIElement, accessibility_error_constant_name,
    kAXCreatedNotification, kAXErrorSuccess, kAXFocusedWindowChangedNotification,
    kAXMenuClosedNotification, kAXMenuOpenedNotification, kAXTitleChangedNotification,
    kAXUIElementDestroyedNotification, kAXWindowDeminiaturizedNotification,
    kAXWindowMiniaturizedNotification, kAXWindowMovedNotification, kAXWindowResizedNotification,
    read_window_id_of_accessibility_element,
};
use crate::ffi::core_foundation::{
    CFEqual, CFRetained, CFRetainedAssumedSendAndSync, CFRunLoopAddSource, CFRunLoopGetMain,
    CFRunLoopSourceInvalidate, CFString, Type, as_cftype, kCFRunLoopDefaultMode,
};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::state::process_wide::WINDOW_FOCUS_NOTIFICATION_IS_PENDING;
use crate::support::handles::WindowId;
use crate::window::model::WindowLivenessCell;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ApplicationAccessibilityNotification(pub u8);

impl ApplicationAccessibilityNotification {
    pub(crate) const WINDOW_CREATED: ApplicationAccessibilityNotification =
        ApplicationAccessibilityNotification(1 << APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_CREATED);
    pub(crate) const WINDOW_FOCUSED: ApplicationAccessibilityNotification =
        ApplicationAccessibilityNotification(1 << APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_FOCUSED);
    pub(crate) const WINDOW_MOVED: ApplicationAccessibilityNotification =
        ApplicationAccessibilityNotification(1 << APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_MOVED);
    pub(crate) const WINDOW_RESIZED: ApplicationAccessibilityNotification =
        ApplicationAccessibilityNotification(1 << APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_RESIZED);
    pub(crate) const WINDOW_TITLE_CHANGED: ApplicationAccessibilityNotification =
        ApplicationAccessibilityNotification(
            1 << APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_TITLE_CHANGED,
        );
    pub(crate) const ALL: ApplicationAccessibilityNotification =
        ApplicationAccessibilityNotification(
            ApplicationAccessibilityNotification::WINDOW_CREATED.0
                | ApplicationAccessibilityNotification::WINDOW_FOCUSED.0
                | ApplicationAccessibilityNotification::WINDOW_MOVED.0
                | ApplicationAccessibilityNotification::WINDOW_RESIZED.0
                | ApplicationAccessibilityNotification::WINDOW_TITLE_CHANGED.0,
        );
}

pub(crate) const APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_CREATED: usize = 0;
pub(crate) const APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_FOCUSED: usize = 1;
pub(crate) const APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_MOVED: usize = 2;
pub(crate) const APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_RESIZED: usize = 3;
pub(crate) const APPLICATION_NOTIFICATION_INDEX_OF_WINDOW_TITLE_CHANGED: usize = 4;

pub(crate) static APPLICATION_NOTIFICATION_CONSTANT_NAMES: [&str; 7] = [
    "kAXCreatedNotification",
    "kAXFocusedWindowChangedNotification",
    "kAXWindowMovedNotification",
    "kAXWindowResizedNotification",
    "kAXTitleChangedNotification",
    "kAXMenuOpenedNotification",
    "kAXMenuClosedNotification",
];

pub(crate) static APPLICATION_NOTIFICATION_CFSTRINGS: OnceLock<[CFStringOwned; 7]> =
    OnceLock::new();

pub(crate) fn application_notification_cfstrings() -> &'static [CFStringOwned; 7] {
    APPLICATION_NOTIFICATION_CFSTRINGS.get_or_init(|| {
        [
            CFRetainedAssumedSendAndSync(kAXCreatedNotification().retain()),
            CFRetainedAssumedSendAndSync(kAXFocusedWindowChangedNotification().retain()),
            CFRetainedAssumedSendAndSync(kAXWindowMovedNotification().retain()),
            CFRetainedAssumedSendAndSync(kAXWindowResizedNotification().retain()),
            CFRetainedAssumedSendAndSync(kAXTitleChangedNotification().retain()),
            CFRetainedAssumedSendAndSync(kAXMenuOpenedNotification().retain()),
            CFRetainedAssumedSendAndSync(kAXMenuClosedNotification().retain()),
        ]
    })
}

pub(crate) unsafe extern "C-unwind" fn handle_application_accessibility_notification_callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    context: *mut c_void,
) {
    let notification = unsafe { notification.as_ref() };
    let element = unsafe { element.as_ref() };

    if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXCreatedNotification())),
    ) {
        post_event_to_event_loop(Event::WindowCreated(CFRetainedAssumedSendAndSync(
            element.retain(),
        )));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXFocusedWindowChangedNotification())),
    ) {
        WINDOW_FOCUS_NOTIFICATION_IS_PENDING.store(true, Ordering::Release);
        post_event_to_event_loop(Event::WindowFocused(WindowId(
            read_window_id_of_accessibility_element(element),
        )));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXWindowMovedNotification())),
    ) {
        post_event_to_event_loop(Event::WindowMoved(WindowId(
            read_window_id_of_accessibility_element(element),
        )));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXWindowResizedNotification())),
    ) {
        post_event_to_event_loop(Event::WindowResized(WindowId(
            read_window_id_of_accessibility_element(element),
        )));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXTitleChangedNotification())),
    ) {
        post_event_to_event_loop(Event::WindowTitleChanged(WindowId(
            read_window_id_of_accessibility_element(element),
        )));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXMenuOpenedNotification())),
    ) {
        post_event_to_event_loop(Event::MenuOpened(WindowId(
            read_window_id_of_accessibility_element(element),
        )));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXMenuClosedNotification())),
    ) {
        post_event_to_event_loop(Event::MenuClosed);
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXWindowMiniaturizedNotification())),
    ) {
        let window_liveness_cell = unsafe { &*context.cast::<WindowLivenessCell>() };
        if !window_liveness_cell.is_still_alive() {
            return;
        }

        post_event_to_event_loop(Event::WindowMinimized(window_liveness_cell.window_id));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXWindowDeminiaturizedNotification())),
    ) {
        let window_liveness_cell = unsafe { &*context.cast::<WindowLivenessCell>() };
        if !window_liveness_cell.is_still_alive() {
            return;
        }

        post_event_to_event_loop(Event::WindowDeminimized(window_liveness_cell.window_id));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXUIElementDestroyedNotification())),
    ) {
        let window_liveness_cell = unsafe { &*context.cast::<WindowLivenessCell>() };

        //
        // NOTE(asmvik): Flag events that are already queued, but not yet processed,
        // so that they will be ignored; the memory we allocated is still valid and will
        // be freed when this event is handled.
        //

        if !window_liveness_cell.claim_for_destruction() {
            return;
        }

        post_event_to_event_loop(Event::WindowDestroyed(window_liveness_cell.window_id));
    }
}

pub(crate) fn start_observing_application_notifications_reporting_whether_all_registered(
    application: &mut Application,
) -> bool {
    if unsafe {
        AXObserverCreate(
            application.process_id.0,
            Some(handle_application_accessibility_notification_callback),
            NonNull::from(&mut application.observer_ref),
        )
    } == kAXErrorSuccess
    {
        for index in 0..application_notification_cfstrings().len() {
            let result = unsafe {
                AXObserverAddNotification(
                    &*application.observer_ref,
                    &*application.element_ref,
                    application_notification_cfstrings()[index].as_ref(),
                    application.process_id.0 as isize as *mut c_void,
                )
            };
            if result == kAXErrorSuccess || result == AXError::NotificationAlreadyRegistered {
                application.notification |= 1u8 << index;
            } else {
                if result == AXError::CannotComplete {
                    application.ax_retry = true;
                }
                crate::debug!(
                    "{}: error '{}' for application '{}' and notification '{}'\n",
                    "start_observing_application_notifications_reporting_whether_all_registered",
                    accessibility_error_constant_name(result),
                    application.name,
                    APPLICATION_NOTIFICATION_CONSTANT_NAMES[index]
                );
            }
        }

        application.is_observing = true;
        let run_loop_source = unsafe { AXObserverGetRunLoopSource(&*application.observer_ref) };
        CFRunLoopAddSource(
            &CFRunLoopGetMain().unwrap(),
            Some(&run_loop_source),
            unsafe { kCFRunLoopDefaultMode },
        );
    }

    (application.notification & ApplicationAccessibilityNotification::ALL.0)
        == ApplicationAccessibilityNotification::ALL.0
}

pub(crate) fn stop_observing_application_notifications(application: &mut Application) {
    if application.is_observing {
        let notification = core::mem::replace(&mut application.notification, 0);
        let observer_ref = NonNull::new(core::mem::replace(
            &mut application.observer_ref,
            core::ptr::null_mut(),
        ));
        application.is_observing = false;

        let Some(observer_ref) = observer_ref else {
            return;
        };
        let observer_ref = unsafe { CFRetained::from_raw(observer_ref) };
        let element_ref = unsafe { &*application.element_ref }.retain();

        dispatch_after_on_main_queue(0, move || {
            for index in 0..application_notification_cfstrings().len() {
                if notification & (1u8 << index) == 0 {
                    continue;
                }

                unsafe {
                    AXObserverRemoveNotification(
                        &observer_ref,
                        &element_ref,
                        application_notification_cfstrings()[index].as_ref(),
                    )
                };
            }

            let run_loop_source = unsafe { AXObserverGetRunLoopSource(&observer_ref) };
            CFRunLoopSourceInvalidate(&run_loop_source);
        });
    }
}
