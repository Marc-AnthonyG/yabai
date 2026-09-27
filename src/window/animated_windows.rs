use crate::ffi::core_foundation::CGRect;
use crate::support::easing::AnimationEasingType;
use crate::support::handles::WindowId;
use crate::window::animation_frame_transaction::ProxyFrameUpdate;
use crate::window::animation_layers::LayeredFrameAnimation;
use crate::window::proxy::WindowProxy;
use crate::window::proxy_alpha_refresh::ProxyAlphaRefreshSchedule;

struct MovingWindow {
    proxy: WindowProxy,
    layered_frame_animation: LayeredFrameAnimation,
    proxy_alpha_refresh_schedule: ProxyAlphaRefreshSchedule,
}

enum AnimatedWindowPhase {
    Moving(MovingWindow),
    AwaitingProxySwapOut,
}

struct AnimatedWindow {
    window_id: WindowId,
    phase: AnimatedWindowPhase,
}

pub(crate) struct AnimatedWindows {
    animated_window_list: Vec<AnimatedWindow>,
}

pub(crate) struct AnimatedWindowsAdvancedToATick {
    pub(crate) proxy_frame_update_list: Vec<ProxyFrameUpdate>,
    pub(crate) proxies_that_came_to_rest: Vec<WindowProxy>,
}

impl AnimatedWindows {
    pub(crate) fn new() -> AnimatedWindows {
        AnimatedWindows {
            animated_window_list: Vec::new(),
        }
    }

    fn find(&self, window_id: WindowId) -> Option<&AnimatedWindow> {
        self.animated_window_list
            .iter()
            .find(|animated_window| animated_window.window_id == window_id)
    }

    fn find_mut(&mut self, window_id: WindowId) -> Option<&mut AnimatedWindow> {
        self.animated_window_list
            .iter_mut()
            .find(|animated_window| animated_window.window_id == window_id)
    }

    pub(crate) fn retarget_if_moving(
        &mut self,
        window_id: WindowId,
        target_frame: CGRect,
        duration_in_seconds: f32,
        easing: AnimationEasingType,
    ) -> bool {
        let Some(AnimatedWindow {
            phase: AnimatedWindowPhase::Moving(moving_window),
            ..
        }) = self.find_mut(window_id)
        else {
            return false;
        };

        moving_window
            .layered_frame_animation
            .retarget_with_a_held_layer(target_frame, duration_in_seconds, easing);
        true
    }

    pub(crate) fn is_awaiting_proxy_swap_out(&self, window_id: WindowId) -> bool {
        matches!(
            self.find(window_id),
            Some(AnimatedWindow {
                phase: AnimatedWindowPhase::AwaitingProxySwapOut,
                ..
            })
        )
    }

    pub(crate) fn begin_moving_with_a_held_layer(
        &mut self,
        proxy: WindowProxy,
        target_frame: CGRect,
        duration_in_seconds: f32,
        easing: AnimationEasingType,
    ) {
        let window_id = proxy.real_window_id;
        debug_assert!(self.find(window_id).is_none());

        let layered_frame_animation =
            LayeredFrameAnimation::from_current_frame_towards_target_with_a_held_layer(
                proxy.frame,
                target_frame,
                duration_in_seconds,
                easing,
            );
        self.animated_window_list.push(AnimatedWindow {
            window_id,
            phase: AnimatedWindowPhase::Moving(MovingWindow {
                proxy,
                layered_frame_animation,
                proxy_alpha_refresh_schedule: ProxyAlphaRefreshSchedule::fresh_from_the_proxy_build(
                ),
            }),
        });
    }

    pub(crate) fn release_layers_held_until_their_request_is_in_motion(&mut self) {
        for animated_window in &mut self.animated_window_list {
            if let AnimatedWindowPhase::Moving(moving_window) = &mut animated_window.phase {
                moving_window
                    .layered_frame_animation
                    .release_layers_held_until_their_request_is_in_motion();
            }
        }
    }

    pub(crate) fn has_a_moving_window(&self) -> bool {
        self.animated_window_list
            .iter()
            .any(|animated_window| matches!(animated_window.phase, AnimatedWindowPhase::Moving(_)))
    }

    pub(crate) fn advance_every_moving_window_to_display_link_tick(
        &mut self,
        tick_host_time: u64,
        output_host_time: u64,
        host_clock_frequency: f64,
    ) -> AnimatedWindowsAdvancedToATick {
        let mut advanced = AnimatedWindowsAdvancedToATick {
            proxy_frame_update_list: Vec::new(),
            proxies_that_came_to_rest: Vec::new(),
        };

        for animated_window in &mut self.animated_window_list {
            let AnimatedWindowPhase::Moving(moving_window) = &mut animated_window.phase else {
                continue;
            };

            let displayed_frame = moving_window
                .layered_frame_animation
                .advance_to_display_link_tick(
                    tick_host_time,
                    output_host_time,
                    host_clock_frequency,
                );
            let must_refresh_alpha = moving_window
                .proxy_alpha_refresh_schedule
                .claim_refresh_if_due(output_host_time, host_clock_frequency);
            advanced.proxy_frame_update_list.push(ProxyFrameUpdate {
                real_window_id: animated_window.window_id,
                proxy_window_id: moving_window.proxy.id,
                proxy_frame_size: moving_window.proxy.frame.size,
                displayed_frame,
                must_refresh_alpha,
            });

            if moving_window.layered_frame_animation.has_come_to_rest() {
                advanced
                    .proxies_that_came_to_rest
                    .extend(hand_proxy_over_for_swap_out(animated_window));
            }
        }

        advanced
    }

    pub(crate) fn hand_every_moving_proxy_over_for_swap_out(&mut self) -> Vec<WindowProxy> {
        self.animated_window_list
            .iter_mut()
            .filter_map(hand_proxy_over_for_swap_out)
            .collect()
    }

    pub(crate) fn forget_windows_whose_proxy_was_swapped_out(
        &mut self,
        window_id_list: &[WindowId],
    ) {
        self.animated_window_list.retain(|animated_window| {
            !(matches!(
                animated_window.phase,
                AnimatedWindowPhase::AwaitingProxySwapOut
            ) && window_id_list.contains(&animated_window.window_id))
        });
    }
}

fn hand_proxy_over_for_swap_out(animated_window: &mut AnimatedWindow) -> Option<WindowProxy> {
    if !matches!(animated_window.phase, AnimatedWindowPhase::Moving(_)) {
        return None;
    }

    match core::mem::replace(
        &mut animated_window.phase,
        AnimatedWindowPhase::AwaitingProxySwapOut,
    ) {
        AnimatedWindowPhase::Moving(moving_window) => Some(moving_window.proxy),
        AnimatedWindowPhase::AwaitingProxySwapOut => None,
    }
}
