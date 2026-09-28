use std::sync::Arc;

use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};
use crate::scripting_addition::client::swap_window_proxies_in_through_scripting_addition;
use crate::support::handles::WindowId;
use crate::window::animation_completion::AnimationCompletionJob;
use crate::window::animator::{
    retarget_moving_windows_and_collect_stationary_ones, set_animation_request_in_motion,
    window_animator_resources_starting_them_once,
};
use crate::window::frame::move_and_resize_window_through_accessibility;
use crate::window::janky_borders::notify_janky_borders_of_proxy_pairings;
use crate::window::manager::WindowManager;
use crate::window::proxy_builders::build_proxies_for_stationary_windows;
use crate::window::proxy_pairing::WindowProxyPairing;

#[derive(Clone, Copy)]
pub(crate) struct WindowWithTargetFrame {
    pub(crate) window_id: WindowId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl WindowWithTargetFrame {
    pub(crate) fn target_frame(&self) -> CGRect {
        CGRect::new(
            CGPoint::new(self.x as f64, self.y as f64),
            CGSize::new(self.width as f64, self.height as f64),
        )
    }
}

fn move_windows_to_their_target_frames_at_once(
    window_list: &[WindowWithTargetFrame],
    window_manager: &mut WindowManager,
) {
    for window_capture in window_list {
        move_and_resize_window_through_accessibility(
            window_capture.window_id,
            window_capture.x,
            window_capture.y,
            window_capture.width,
            window_capture.height,
            window_manager,
        );
    }
}

pub(crate) fn animate_windows_to_their_target_frames_through_proxies(
    window_list: &[WindowWithTargetFrame],
    window_manager: &mut WindowManager,
) {
    let window_animator = Arc::clone(&window_manager.window_animator);
    let Some(window_animator_resources) =
        window_animator_resources_starting_them_once(&window_animator)
    else {
        move_windows_to_their_target_frames_at_once(window_list, window_manager);
        return;
    };
    let animation_duration = window_manager.window_animation_duration;
    let animation_easing = window_manager.window_animation_easing;

    let stationary_window_list = retarget_moving_windows_and_collect_stationary_ones(
        &window_animator,
        window_list,
        animation_duration,
        animation_easing,
    );

    let proxies_with_their_target_frame = build_proxies_for_stationary_windows(
        window_animator_resources.animation_connection,
        &stationary_window_list,
    );

    let pairing_list: Vec<WindowProxyPairing> = proxies_with_their_target_frame
        .iter()
        .map(|(proxy, _)| proxy.pairing())
        .collect();
    let proxies_were_swapped_in =
        pairing_list.is_empty() || swap_window_proxies_in_through_scripting_addition(&pairing_list);
    if proxies_were_swapped_in && !pairing_list.is_empty() {
        notify_janky_borders_of_proxy_pairings(&pairing_list, 1325, false);
    }

    move_windows_to_their_target_frames_at_once(window_list, window_manager);

    if proxies_were_swapped_in {
        set_animation_request_in_motion(
            &window_animator,
            window_animator_resources,
            proxies_with_their_target_frame,
            animation_duration,
            animation_easing,
        );
    } else {
        window_animator_resources.hand_over_to_the_completion_worker(
            AnimationCompletionJob::ReleaseProxiesThatWereNeverSwappedIn(
                proxies_with_their_target_frame
                    .into_iter()
                    .map(|(proxy, _)| proxy)
                    .collect(),
            ),
        );
    }
}

pub(crate) fn move_windows_to_their_target_frames_animating_if_enabled(
    window_list: &[WindowWithTargetFrame],
    window_manager: &mut WindowManager,
) {
    if window_manager.window_animation_duration != 0.0f32 {
        animate_windows_to_their_target_frames_through_proxies(window_list, window_manager);
    } else {
        move_windows_to_their_target_frames_at_once(window_list, window_manager);
    }
}

pub(crate) fn move_window_to_its_target_frame_animating_if_enabled(
    capture: WindowWithTargetFrame,
    window_manager: &mut WindowManager,
) {
    if window_manager.window_animation_duration != 0.0f32 {
        animate_windows_to_their_target_frames_through_proxies(
            core::slice::from_ref(&capture),
            window_manager,
        );
    } else {
        move_and_resize_window_through_accessibility(
            capture.window_id,
            capture.x,
            capture.y,
            capture.width,
            capture.height,
            window_manager,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WindowWithTargetFrame, move_window_to_its_target_frame_animating_if_enabled,
        move_windows_to_their_target_frames_animating_if_enabled,
    };
    use crate::support::handles::WindowId;
    use crate::window::animator::window_animator_started_resources;
    use crate::window::manager::create_window_manager_tracking_nothing_with_its_initial_settings;

    fn capture_of_a_window_yabai_does_not_track() -> WindowWithTargetFrame {
        WindowWithTargetFrame {
            window_id: WindowId(424242),
            x: 10.0,
            y: 20.0,
            width: 640.0,
            height: 480.0,
        }
    }

    #[test]
    fn with_a_zero_animation_duration_frames_are_set_at_once_without_starting_the_animator() {
        let mut window_manager = create_window_manager_tracking_nothing_with_its_initial_settings();
        window_manager.window_animation_duration = 0.0;

        move_windows_to_their_target_frames_animating_if_enabled(
            &[
                capture_of_a_window_yabai_does_not_track(),
                capture_of_a_window_yabai_does_not_track(),
            ],
            &mut window_manager,
        );
        move_window_to_its_target_frame_animating_if_enabled(
            capture_of_a_window_yabai_does_not_track(),
            &mut window_manager,
        );

        assert!(window_animator_started_resources(&window_manager.window_animator).is_none());
    }
}
