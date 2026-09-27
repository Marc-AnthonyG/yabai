use std::sync::{Arc, Condvar, Mutex, OnceLock};

use crate::ffi::core_foundation::CGRect;
use crate::ffi::core_video::CVDisplayLink;
use crate::support::easing::AnimationEasingType;
use crate::support::handles::WindowId;
use crate::window::animated_windows::AnimatedWindows;
use crate::window::animation::WindowCapture;
use crate::window::animation_completion::AnimationCompletionJob;
use crate::window::animation_display_link::{AnimationDisplayLink, animation_display_link_start};
use crate::window::animation_frame_transaction::ProxyFrameUpdate;
use crate::window::animator_resources::{WindowAnimatorResources, window_animator_resources_start};
use crate::window::proxy::WindowProxy;

pub(crate) struct WindowAnimator {
    shared_state: Mutex<WindowAnimatorSharedState>,
    proxy_swap_out_finished: Condvar,
    resources: OnceLock<WindowAnimatorResources>,
}

struct WindowAnimatorSharedState {
    animated_windows: AnimatedWindows,
    display_link: Option<AnimationDisplayLink>,
}

pub(crate) struct DisplayLinkTickWork {
    pub(crate) proxy_frame_update_list: Vec<ProxyFrameUpdate>,
    pub(crate) proxies_that_came_to_rest: Vec<WindowProxy>,
    pub(crate) retired_display_link: Option<AnimationDisplayLink>,
}

impl WindowAnimator {
    pub(crate) fn new() -> WindowAnimator {
        WindowAnimator {
            shared_state: Mutex::new(WindowAnimatorSharedState {
                animated_windows: AnimatedWindows::new(),
                display_link: None,
            }),
            proxy_swap_out_finished: Condvar::new(),
            resources: OnceLock::new(),
        }
    }
}

pub(crate) fn window_animator_started_resources(
    window_animator: &WindowAnimator,
) -> Option<&WindowAnimatorResources> {
    window_animator.resources.get()
}

pub(crate) fn window_animator_resources_starting_them_once(
    window_animator: &Arc<WindowAnimator>,
) -> Option<&WindowAnimatorResources> {
    if window_animator.resources.get().is_none() {
        let window_animator_resources =
            window_animator_resources_start(Arc::downgrade(window_animator))?;
        let _ = window_animator.resources.set(window_animator_resources);
    }
    window_animator.resources.get()
}

pub(crate) fn window_animator_retarget_moving_windows_and_collect_stationary_ones(
    window_animator: &WindowAnimator,
    window_list: &[WindowCapture],
    duration_in_seconds: f32,
    easing: AnimationEasingType,
) -> Vec<WindowCapture> {
    let mut shared_state = window_animator.shared_state.lock().unwrap();

    let was_retargeted_list: Vec<bool> = window_list
        .iter()
        .map(|window_capture| {
            shared_state.animated_windows.retarget_if_moving(
                window_capture.window_id,
                window_capture.target_frame(),
                duration_in_seconds,
                easing,
            )
        })
        .collect();

    while window_list.iter().any(|window_capture| {
        shared_state
            .animated_windows
            .is_awaiting_proxy_swap_out(window_capture.window_id)
    }) {
        shared_state = window_animator
            .proxy_swap_out_finished
            .wait(shared_state)
            .unwrap();
    }
    drop(shared_state);

    let mut stationary_window_list: Vec<WindowCapture> = Vec::new();
    for (window_capture, was_retargeted) in window_list.iter().zip(was_retargeted_list) {
        if was_retargeted {
            continue;
        }

        match stationary_window_list
            .iter_mut()
            .find(|stationary_window| stationary_window.window_id == window_capture.window_id)
        {
            Some(stationary_window) => *stationary_window = *window_capture,
            None => stationary_window_list.push(*window_capture),
        }
    }
    stationary_window_list
}

pub(crate) fn window_animator_set_request_in_motion(
    window_animator: &Arc<WindowAnimator>,
    window_animator_resources: &WindowAnimatorResources,
    proxies_with_their_target_frame: Vec<(WindowProxy, CGRect)>,
    duration_in_seconds: f32,
    easing: AnimationEasingType,
) {
    let needs_a_display_link = {
        let mut shared_state = window_animator.shared_state.lock().unwrap();

        for (proxy, target_frame) in proxies_with_their_target_frame {
            shared_state
                .animated_windows
                .begin_moving_with_a_held_layer(proxy, target_frame, duration_in_seconds, easing);
        }
        shared_state
            .animated_windows
            .release_layers_held_until_their_request_is_in_motion();

        shared_state.animated_windows.has_a_moving_window() && shared_state.display_link.is_none()
    };
    if !needs_a_display_link {
        return;
    }

    let started_display_link = animation_display_link_start(window_animator);

    let proxies_to_swap_out_at_once = {
        let mut shared_state = window_animator.shared_state.lock().unwrap();
        match started_display_link {
            Some(animation_display_link) => {
                debug_assert!(shared_state.display_link.is_none());
                shared_state.display_link = Some(animation_display_link);
                Vec::new()
            }
            None => shared_state
                .animated_windows
                .hand_every_moving_proxy_over_for_swap_out(),
        }
    };

    if !proxies_to_swap_out_at_once.is_empty() {
        window_animator_resources.hand_over_to_the_completion_worker(
            AnimationCompletionJob::SwapOutAndReleaseProxies(proxies_to_swap_out_at_once),
        );
    }
}

pub(crate) fn window_animator_advance_to_display_link_tick(
    window_animator: &WindowAnimator,
    display_link: &CVDisplayLink,
    tick_host_time: u64,
    output_host_time: u64,
    host_clock_frequency: f64,
) -> Option<DisplayLinkTickWork> {
    let mut shared_state = window_animator.shared_state.lock().unwrap();

    let is_driven_by_this_display_link =
        shared_state
            .display_link
            .as_ref()
            .is_some_and(|animation_display_link| {
                animation_display_link.is_the_display_link(display_link)
            });
    if !is_driven_by_this_display_link {
        return None;
    }

    let advanced = shared_state
        .animated_windows
        .advance_every_moving_window_to_display_link_tick(
            tick_host_time,
            output_host_time,
            host_clock_frequency,
        );

    let retired_display_link = if shared_state.animated_windows.has_a_moving_window() {
        None
    } else {
        shared_state.display_link.take()
    };

    Some(DisplayLinkTickWork {
        proxy_frame_update_list: advanced.proxy_frame_update_list,
        proxies_that_came_to_rest: advanced.proxies_that_came_to_rest,
        retired_display_link,
    })
}

pub(crate) fn window_animator_forget_windows_whose_proxy_was_swapped_out(
    window_animator: &WindowAnimator,
    window_id_list: &[WindowId],
) {
    window_animator
        .shared_state
        .lock()
        .unwrap()
        .animated_windows
        .forget_windows_whose_proxy_was_swapped_out(window_id_list);
    window_animator.proxy_swap_out_finished.notify_all();
}
