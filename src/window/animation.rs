use std::sync::Arc;

use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};
use crate::scripting_addition::client::scripting_addition_swap_window_proxy_in;
use crate::support::handles::WindowId;
use crate::window::animator::{
    window_animator_resources_starting_them_once,
    window_animator_retarget_moving_windows_and_collect_stationary_ones,
    window_animator_set_request_in_motion,
};
use crate::window::frame::window_manager_set_window_frame;
use crate::window::janky_borders::window_manager_notify_jankyborders;
use crate::window::manager::WindowManager;
use crate::window::proxy_builders::window_manager_build_proxies_for_stationary_windows;
use crate::window::proxy_pairing::WindowProxyPairing;

#[derive(Clone, Copy)]
pub(crate) struct WindowCapture {
    pub(crate) window_id: WindowId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl WindowCapture {
    pub(crate) fn target_frame(&self) -> CGRect {
        CGRect::new(
            CGPoint::new(self.x as f64, self.y as f64),
            CGSize::new(self.width as f64, self.height as f64),
        )
    }
}

fn window_manager_set_window_list_frames_at_once(
    window_list: &[WindowCapture],
    window_manager: &mut WindowManager,
) {
    for window_capture in window_list {
        window_manager_set_window_frame(
            window_capture.window_id,
            window_capture.x,
            window_capture.y,
            window_capture.width,
            window_capture.height,
            window_manager,
        );
    }
}

pub(crate) fn window_manager_animate_window_list_async(
    window_list: &[WindowCapture],
    window_manager: &mut WindowManager,
) {
    let window_animator = Arc::clone(&window_manager.window_animator);
    let Some(window_animator_resources) =
        window_animator_resources_starting_them_once(&window_animator)
    else {
        window_manager_set_window_list_frames_at_once(window_list, window_manager);
        return;
    };
    let animation_duration = window_manager.window_animation_duration;
    let animation_easing = window_manager.window_animation_easing;

    let stationary_window_list =
        window_animator_retarget_moving_windows_and_collect_stationary_ones(
            &window_animator,
            window_list,
            animation_duration,
            animation_easing,
        );

    let proxies_with_their_target_frame = window_manager_build_proxies_for_stationary_windows(
        window_animator_resources.animation_connection,
        &stationary_window_list,
    );

    let pairing_list: Vec<WindowProxyPairing> = proxies_with_their_target_frame
        .iter()
        .map(|(proxy, _)| proxy.pairing())
        .collect();
    if !pairing_list.is_empty() {
        scripting_addition_swap_window_proxy_in(&pairing_list);
        window_manager_notify_jankyborders(&pairing_list, 1325, false);
    }

    window_manager_set_window_list_frames_at_once(window_list, window_manager);

    window_animator_set_request_in_motion(
        &window_animator,
        window_animator_resources,
        proxies_with_their_target_frame,
        animation_duration,
        animation_easing,
    );
}

pub(crate) fn window_manager_animate_window_list(
    window_list: &[WindowCapture],
    window_manager: &mut WindowManager,
) {
    if window_manager.window_animation_duration != 0.0f32 {
        window_manager_animate_window_list_async(window_list, window_manager);
    } else {
        window_manager_set_window_list_frames_at_once(window_list, window_manager);
    }
}

pub(crate) fn window_manager_animate_window(
    capture: WindowCapture,
    window_manager: &mut WindowManager,
) {
    if window_manager.window_animation_duration != 0.0f32 {
        window_manager_animate_window_list_async(core::slice::from_ref(&capture), window_manager);
    } else {
        window_manager_set_window_frame(
            capture.window_id,
            capture.x,
            capture.y,
            capture.width,
            capture.height,
            window_manager,
        );
    }
}
