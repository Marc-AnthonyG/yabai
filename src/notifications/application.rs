#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;

use crate::application::model::Application;
use crate::event::queue::{Event, event_loop_post};
use crate::ffi::CFStringOwned;
use crate::ffi::accessibility::{
    AXError, AXObserver, AXObserverAddNotification, AXObserverCreate, AXObserverGetRunLoopSource,
    AXObserverRemoveNotification, AXUIElement, ax_error_str, ax_window_id, kAXCreatedNotification,
    kAXErrorSuccess, kAXFocusedWindowChangedNotification, kAXMenuClosedNotification,
    kAXMenuOpenedNotification, kAXTitleChangedNotification, kAXUIElementDestroyedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
    kAXWindowMovedNotification, kAXWindowResizedNotification,
};
use crate::ffi::core_foundation::{
    CFEqual, CFRetained, CFRunLoopAddSource, CFRunLoopGetMain, CFRunLoopSourceInvalidate, CFString,
    SendCFRetained, Type, as_cftype, kCFRunLoopDefaultMode,
};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::state::process_wide::PENDING_WINDOW_FOCUS;
use crate::support::handles::WindowId;
use crate::window::model::WindowLivenessCell;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct AxApplicationNotification(pub u8);

impl AxApplicationNotification {
    pub(crate) const WINDOW_CREATED: AxApplicationNotification =
        AxApplicationNotification(1 << AX_APPLICATION_WINDOW_CREATED_INDEX);
    pub(crate) const WINDOW_FOCUSED: AxApplicationNotification =
        AxApplicationNotification(1 << AX_APPLICATION_WINDOW_FOCUSED_INDEX);
    pub(crate) const WINDOW_MOVED: AxApplicationNotification =
        AxApplicationNotification(1 << AX_APPLICATION_WINDOW_MOVED_INDEX);
    pub(crate) const WINDOW_RESIZED: AxApplicationNotification =
        AxApplicationNotification(1 << AX_APPLICATION_WINDOW_RESIZED_INDEX);
    pub(crate) const WINDOW_TITLE_CHANGED: AxApplicationNotification =
        AxApplicationNotification(1 << AX_APPLICATION_WINDOW_TITLE_CHANGED_INDEX);
    pub(crate) const ALL: AxApplicationNotification = AxApplicationNotification(
        AxApplicationNotification::WINDOW_CREATED.0
            | AxApplicationNotification::WINDOW_FOCUSED.0
            | AxApplicationNotification::WINDOW_MOVED.0
            | AxApplicationNotification::WINDOW_RESIZED.0
            | AxApplicationNotification::WINDOW_TITLE_CHANGED.0,
    );
}

pub(crate) const AX_APPLICATION_WINDOW_CREATED_INDEX: usize = 0;
pub(crate) const AX_APPLICATION_WINDOW_FOCUSED_INDEX: usize = 1;
pub(crate) const AX_APPLICATION_WINDOW_MOVED_INDEX: usize = 2;
pub(crate) const AX_APPLICATION_WINDOW_RESIZED_INDEX: usize = 3;
pub(crate) const AX_APPLICATION_WINDOW_TITLE_CHANGED_INDEX: usize = 4;

pub(crate) static AX_APPLICATION_NOTIFICATION_STR: [&str; 7] = [
    "kAXCreatedNotification",
    "kAXFocusedWindowChangedNotification",
    "kAXWindowMovedNotification",
    "kAXWindowResizedNotification",
    "kAXTitleChangedNotification",
    "kAXMenuOpenedNotification",
    "kAXMenuClosedNotification",
];

pub(crate) static AX_APPLICATION_NOTIFICATION: OnceLock<[CFStringOwned; 7]> = OnceLock::new();

pub(crate) fn ax_application_notification() -> &'static [CFStringOwned; 7] {
    AX_APPLICATION_NOTIFICATION.get_or_init(|| {
        [
            SendCFRetained(kAXCreatedNotification().retain()),
            SendCFRetained(kAXFocusedWindowChangedNotification().retain()),
            SendCFRetained(kAXWindowMovedNotification().retain()),
            SendCFRetained(kAXWindowResizedNotification().retain()),
            SendCFRetained(kAXTitleChangedNotification().retain()),
            SendCFRetained(kAXMenuOpenedNotification().retain()),
            SendCFRetained(kAXMenuClosedNotification().retain()),
        ]
    })
}

pub(crate) unsafe extern "C-unwind" fn application_notification_handler(
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
        event_loop_post(Event::WindowCreated(SendCFRetained(element.retain())));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXFocusedWindowChangedNotification())),
    ) {
        PENDING_WINDOW_FOCUS.store(true, Ordering::Release);
        event_loop_post(Event::WindowFocused(WindowId(ax_window_id(element))));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXWindowMovedNotification())),
    ) {
        event_loop_post(Event::WindowMoved(WindowId(ax_window_id(element))));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXWindowResizedNotification())),
    ) {
        event_loop_post(Event::WindowResized(WindowId(ax_window_id(element))));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXTitleChangedNotification())),
    ) {
        event_loop_post(Event::WindowTitleChanged(WindowId(ax_window_id(element))));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXMenuOpenedNotification())),
    ) {
        event_loop_post(Event::MenuOpened(WindowId(ax_window_id(element))));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXMenuClosedNotification())),
    ) {
        event_loop_post(Event::MenuClosed);
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXWindowMiniaturizedNotification())),
    ) {
        let window_liveness_cell = unsafe { &*context.cast::<WindowLivenessCell>() };
        if !window_liveness_cell.is_still_alive() {
            return;
        }

        event_loop_post(Event::WindowMinimized(window_liveness_cell.window_id));
    } else if CFEqual(
        Some(as_cftype(notification)),
        Some(as_cftype(kAXWindowDeminiaturizedNotification())),
    ) {
        let window_liveness_cell = unsafe { &*context.cast::<WindowLivenessCell>() };
        if !window_liveness_cell.is_still_alive() {
            return;
        }

        event_loop_post(Event::WindowDeminimized(window_liveness_cell.window_id));
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

        event_loop_post(Event::WindowDestroyed(window_liveness_cell.window_id));
    }
}

pub(crate) fn application_observe(application: &mut Application) -> bool {
    if unsafe {
        AXObserverCreate(
            application.process_id.0,
            Some(application_notification_handler),
            NonNull::from(&mut application.observer_ref),
        )
    } == kAXErrorSuccess
    {
        for index in 0..ax_application_notification().len() {
            let result = unsafe {
                AXObserverAddNotification(
                    &*application.observer_ref,
                    &*application.element_ref,
                    ax_application_notification()[index].as_ref(),
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
                    "application_observe",
                    ax_error_str(result),
                    application.name,
                    AX_APPLICATION_NOTIFICATION_STR[index]
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

    (application.notification & AxApplicationNotification::ALL.0)
        == AxApplicationNotification::ALL.0
}

pub(crate) fn application_unobserve(application: &mut Application) {
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
            for index in 0..ax_application_notification().len() {
                if notification & (1u8 << index) == 0 {
                    continue;
                }

                unsafe {
                    AXObserverRemoveNotification(
                        &observer_ref,
                        &element_ref,
                        ax_application_notification()[index].as_ref(),
                    )
                };
            }

            let run_loop_source = unsafe { AXObserverGetRunLoopSource(&observer_ref) };
            CFRunLoopSourceInvalidate(&run_loop_source);
        });
    }
}
