#![allow(deprecated)]

use core::ffi::{c_int, c_void};
use core::ptr::NonNull;
use std::sync::{Arc, OnceLock};

use crate::ffi::CFStringOwned;
use crate::ffi::accessibility::{
    AXError, AXObserver, AXObserverAddNotification, AXObserverRemoveNotification, AXUIElement,
    accessibility_error_constant_name, kAXErrorSuccess, kAXUIElementDestroyedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
};
use crate::ffi::core_foundation::{CFRetained, CFRetainedAssumedSendAndSync};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::ffi::skylight::SLSRequestNotificationsForWindows;
use crate::space::manager::SpaceManager;
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::macos_version::{is_running_on_macos_sequoia, is_running_on_macos_tahoe};
use crate::window::manager::WindowManager;
use crate::window::model::{Window, WindowLivenessCell};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowAccessibilityNotification(pub u8);

impl WindowAccessibilityNotification {
    pub(crate) const MINIMIZED: WindowAccessibilityNotification =
        WindowAccessibilityNotification(1 << WINDOW_NOTIFICATION_INDEX_OF_MINIMIZED);
    pub(crate) const DEMINIMIZED: WindowAccessibilityNotification =
        WindowAccessibilityNotification(1 << WINDOW_NOTIFICATION_INDEX_OF_DEMINIMIZED);
    pub(crate) const DESTROYED: WindowAccessibilityNotification =
        WindowAccessibilityNotification(1 << WINDOW_NOTIFICATION_INDEX_OF_DESTROYED);
    pub(crate) const ALL: WindowAccessibilityNotification = WindowAccessibilityNotification(
        WindowAccessibilityNotification::DESTROYED.0
            | WindowAccessibilityNotification::MINIMIZED.0
            | WindowAccessibilityNotification::DEMINIMIZED.0,
    );
}

pub(crate) const WINDOW_NOTIFICATION_INDEX_OF_MINIMIZED: usize = 0;
pub(crate) const WINDOW_NOTIFICATION_INDEX_OF_DEMINIMIZED: usize = 1;
pub(crate) const WINDOW_NOTIFICATION_INDEX_OF_DESTROYED: usize = 2;

pub(crate) static WINDOW_NOTIFICATION_CONSTANT_NAMES: [&str; 3] = [
    "kAXWindowMiniaturizedNotification",
    "kAXWindowDeminiaturizedNotification",
    "kAXUIElementDestroyedNotification",
];

pub(crate) static WINDOW_NOTIFICATION_CFSTRINGS: OnceLock<[CFStringOwned; 3]> = OnceLock::new();

pub(crate) fn window_notification_cfstrings() -> &'static [CFStringOwned; 3] {
    WINDOW_NOTIFICATION_CFSTRINGS.get_or_init(|| {
        [
            CFRetainedAssumedSendAndSync(unsafe {
                CFRetained::retain(NonNull::from(kAXWindowMiniaturizedNotification()))
            }),
            CFRetainedAssumedSendAndSync(unsafe {
                CFRetained::retain(NonNull::from(kAXWindowDeminiaturizedNotification()))
            }),
            CFRetainedAssumedSendAndSync(unsafe {
                CFRetained::retain(NonNull::from(kAXUIElementDestroyedNotification()))
            }),
        ]
    })
}

pub(crate) fn start_observing_window_notifications_reporting_whether_all_registered(
    window: &mut Window,
    window_manager: &mut WindowManager,
) -> bool {
    let observer_ref = window
        .application
        .and_then(|process_id| window_manager.application.find(&process_id))
        .map(|application| application.observer_ref);
    let Some(observer) = observer_ref.and_then(|observer_ref| unsafe { observer_ref.as_ref() })
    else {
        return false;
    };

    let element_ref = window.element_ref;
    let Some(element) = (unsafe { element_ref.as_ref() }) else {
        return false;
    };

    let liveness_reference = Arc::into_raw(Arc::clone(&window.liveness));
    window.liveness_reference_held_by_the_observation = Some(liveness_reference);

    for index in 0..window_notification_cfstrings().len() {
        let result = unsafe {
            AXObserverAddNotification(
                observer,
                element,
                window_notification_cfstrings()[index].as_ref(),
                liveness_reference as *mut c_void,
            )
        };
        if result == kAXErrorSuccess || result == AXError::NotificationAlreadyRegistered {
            window.notification |= 1 << index;
        } else {
            crate::debug!(
                "{}: {} failed with error {}\n",
                "start_observing_window_notifications_reporting_whether_all_registered",
                WINDOW_NOTIFICATION_CONSTANT_NAMES[index],
                accessibility_error_constant_name(result)
            );
        }
    }

    (window.notification & WindowAccessibilityNotification::ALL.0)
        == WindowAccessibilityNotification::ALL.0
}

struct WindowNotificationRemovalRequest {
    observer_ref: Option<CFRetainedAssumedSendAndSync<AXObserver>>,
    window_ref: Option<CFRetainedAssumedSendAndSync<AXUIElement>>,
    notification: u8,
    liveness_reference: *const WindowLivenessCell,
}

pub(crate) fn stop_observing_window_notifications(
    window: &mut Window,
    window_manager: &mut WindowManager,
) {
    let Some(liveness_reference) = window.liveness_reference_held_by_the_observation.take() else {
        return;
    };

    let observer_ref = window
        .application
        .and_then(|process_id| window_manager.application.find(&process_id))
        .and_then(|application| NonNull::new(application.observer_ref))
        .map(|observer| CFRetainedAssumedSendAndSync(unsafe { CFRetained::retain(observer) }));

    let window_ref = NonNull::new(window.element_ref.cast_mut())
        .map(|element| CFRetainedAssumedSendAndSync(unsafe { CFRetained::retain(element) }));

    let request = Box::into_raw(Box::new(WindowNotificationRemovalRequest {
        observer_ref,
        window_ref,
        notification: std::mem::replace(&mut window.notification, 0),
        liveness_reference,
    }));

    dispatch_after_on_main_queue(0, move || {
        remove_window_notifications_on_main_queue(request)
    });
}

fn remove_window_notifications_on_main_queue(request: *mut WindowNotificationRemovalRequest) {
    let request = unsafe { Box::from_raw(request) };

    if let (Some(observer_ref), Some(window_ref)) = (&request.observer_ref, &request.window_ref) {
        for index in 0..window_notification_cfstrings().len() {
            if (request.notification & (1 << index)) == 0 {
                continue;
            }

            unsafe {
                AXObserverRemoveNotification(
                    observer_ref.as_ref(),
                    window_ref.as_ref(),
                    window_notification_cfstrings()[index].as_ref(),
                )
            };
        }
    }

    drop(unsafe { Arc::from_raw(request.liveness_reference) });
}

pub(crate) fn request_skylight_notifications_for_windows_that_need_them(
    window_manager: &mut WindowManager,
    space_manager: &SpaceManager,
) {
    let mut window_list: Vec<u32> = Vec::new();

    if is_running_on_macos_sequoia() || is_running_on_macos_tahoe() {
        // NOTE(asmvik): Subscribe to all windows because of window_destroyed (and ordered) notifications
        for window in window_manager.window.values() {
            window_list.push(window.id.0);
        }
    } else {
        // NOTE(asmvik): Subscribe to windows that have a feedback_border because of window_ordered notifications
        for (space_id, node_id) in window_manager.insert_feedback.values() {
            let Some(node) = space_manager
                .view
                .find(space_id)
                .and_then(|view| view.find_node(*node_id))
            else {
                continue;
            };
            window_list.push(node.window_order[0].0);
        }
    }

    let window_count = window_list.len() as c_int;
    unsafe {
        SLSRequestNotificationsForWindows(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            window_list.as_mut_ptr(),
            window_count,
        );
    }
}
