#![allow(deprecated)]

use core::ffi::{c_int, c_void};
use core::ptr::NonNull;
use std::sync::{Arc, OnceLock};

use crate::ffi::CFStringOwned;
use crate::ffi::accessibility::{
    AXError, AXObserver, AXObserverAddNotification, AXObserverRemoveNotification, AXUIElement,
    ax_error_str, kAXErrorSuccess, kAXUIElementDestroyedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
};
use crate::ffi::core_foundation::{CFRetained, SendCFRetained};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::ffi::skylight::SLSRequestNotificationsForWindows;
use crate::space::manager::SpaceManager;
use crate::state::process_wide::CONNECTION;
use crate::support::macos_version::{workspace_is_macos_sequoia, workspace_is_macos_tahoe};
use crate::window::manager::WindowManager;
use crate::window::model::{Window, WindowLivenessCell};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct AxWindowNotification(pub u8);

impl AxWindowNotification {
    pub(crate) const MINIMIZED: AxWindowNotification =
        AxWindowNotification(1 << AX_WINDOW_MINIMIZED_INDEX);
    pub(crate) const DEMINIMIZED: AxWindowNotification =
        AxWindowNotification(1 << AX_WINDOW_DEMINIMIZED_INDEX);
    pub(crate) const DESTROYED: AxWindowNotification =
        AxWindowNotification(1 << AX_WINDOW_DESTROYED_INDEX);
    pub(crate) const ALL: AxWindowNotification = AxWindowNotification(
        AxWindowNotification::DESTROYED.0
            | AxWindowNotification::MINIMIZED.0
            | AxWindowNotification::DEMINIMIZED.0,
    );
}

pub(crate) const AX_WINDOW_MINIMIZED_INDEX: usize = 0;
pub(crate) const AX_WINDOW_DEMINIMIZED_INDEX: usize = 1;
pub(crate) const AX_WINDOW_DESTROYED_INDEX: usize = 2;

pub(crate) static AX_WINDOW_NOTIFICATION_STR: [&str; 3] = [
    "kAXWindowMiniaturizedNotification",
    "kAXWindowDeminiaturizedNotification",
    "kAXUIElementDestroyedNotification",
];

pub(crate) static AX_WINDOW_NOTIFICATION: OnceLock<[CFStringOwned; 3]> = OnceLock::new();

pub(crate) fn ax_window_notification() -> &'static [CFStringOwned; 3] {
    AX_WINDOW_NOTIFICATION.get_or_init(|| {
        [
            SendCFRetained(unsafe {
                CFRetained::retain(NonNull::from(kAXWindowMiniaturizedNotification()))
            }),
            SendCFRetained(unsafe {
                CFRetained::retain(NonNull::from(kAXWindowDeminiaturizedNotification()))
            }),
            SendCFRetained(unsafe {
                CFRetained::retain(NonNull::from(kAXUIElementDestroyedNotification()))
            }),
        ]
    })
}

pub(crate) fn window_observe(window: &mut Window, window_manager: &mut WindowManager) -> bool {
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

    for index in 0..ax_window_notification().len() {
        let result = unsafe {
            AXObserverAddNotification(
                observer,
                element,
                ax_window_notification()[index].as_ref(),
                liveness_reference as *mut c_void,
            )
        };
        if result == kAXErrorSuccess || result == AXError::NotificationAlreadyRegistered {
            window.notification |= 1 << index;
        } else {
            crate::debug!(
                "{}: {} failed with error {}\n",
                "window_observe",
                AX_WINDOW_NOTIFICATION_STR[index],
                ax_error_str(result)
            );
        }
    }

    (window.notification & AxWindowNotification::ALL.0) == AxWindowNotification::ALL.0
}

struct WindowUnobserveRequest {
    observer_ref: Option<SendCFRetained<AXObserver>>,
    window_ref: Option<SendCFRetained<AXUIElement>>,
    notification: u8,
    liveness_reference: *const WindowLivenessCell,
}

pub(crate) fn window_unobserve(window: &mut Window, window_manager: &mut WindowManager) {
    let Some(liveness_reference) = window.liveness_reference_held_by_the_observation.take() else {
        return;
    };

    let observer_ref = window
        .application
        .and_then(|process_id| window_manager.application.find(&process_id))
        .and_then(|application| NonNull::new(application.observer_ref))
        .map(|observer| SendCFRetained(unsafe { CFRetained::retain(observer) }));

    let window_ref = NonNull::new(window.element_ref.cast_mut())
        .map(|element| SendCFRetained(unsafe { CFRetained::retain(element) }));

    let request = Box::into_raw(Box::new(WindowUnobserveRequest {
        observer_ref,
        window_ref,
        notification: std::mem::replace(&mut window.notification, 0),
        liveness_reference,
    }));

    dispatch_after_on_main_queue(0, move || window_unobserve_on_main_queue(request));
}

fn window_unobserve_on_main_queue(request: *mut WindowUnobserveRequest) {
    let request = unsafe { Box::from_raw(request) };

    if let (Some(observer_ref), Some(window_ref)) = (&request.observer_ref, &request.window_ref) {
        for index in 0..ax_window_notification().len() {
            if (request.notification & (1 << index)) == 0 {
                continue;
            }

            unsafe {
                AXObserverRemoveNotification(
                    observer_ref.as_ref(),
                    window_ref.as_ref(),
                    ax_window_notification()[index].as_ref(),
                )
            };
        }
    }

    drop(unsafe { Arc::from_raw(request.liveness_reference) });
}

pub(crate) fn update_window_notifications(
    window_manager: &mut WindowManager,
    space_manager: &SpaceManager,
) {
    let mut window_list: Vec<u32> = Vec::new();

    if workspace_is_macos_sequoia() || workspace_is_macos_tahoe() {
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
            *CONNECTION.get().unwrap(),
            window_list.as_mut_ptr(),
            window_count,
        );
    }
}
