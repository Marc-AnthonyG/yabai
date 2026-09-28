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

    fn animated_window_with_id(&self, window_id: WindowId) -> Option<&AnimatedWindow> {
        self.animated_window_list
            .iter()
            .find(|animated_window| animated_window.window_id == window_id)
    }

    fn animated_window_with_id_mut(&mut self, window_id: WindowId) -> Option<&mut AnimatedWindow> {
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
        }) = self.animated_window_with_id_mut(window_id)
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
            self.animated_window_with_id(window_id),
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
        debug_assert!(self.animated_window_with_id(window_id).is_none());

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

#[cfg(test)]
mod tests {
    use super::{AnimatedWindowPhase, AnimatedWindows, AnimatedWindowsAdvancedToATick};
    use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};
    use crate::support::easing::AnimationEasingType;
    use crate::support::handles::WindowId;
    use crate::window::proxy::WindowProxy;

    const HOST_CLOCK_FREQUENCY: f64 = 24_000_000.0;
    const HOST_TICKS_PER_DISPLAY_FRAME: u64 = 400_000;
    const HOST_TIME_OF_THE_FIRST_TICK: u64 = 1_000_000_000;
    const DURATION_IN_SECONDS: f32 = 0.1;
    const INDEX_OF_THE_TICK_A_WINDOW_STARTED_ON_THE_FIRST_TICK_COMES_TO_REST: u64 = 6;

    #[derive(Debug, PartialEq)]
    enum Phase {
        Moving,
        AwaitingItsProxySwapOut,
        Absent,
    }

    fn frame(x: f64, y: f64, width: f64, height: f64) -> CGRect {
        CGRect::new(CGPoint::new(x, y), CGSize::new(width, height))
    }

    fn x_y_width_height(rect: CGRect) -> [f64; 4] {
        [
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        ]
    }

    fn proxy_window_id_of(window_id: u32) -> u32 {
        window_id + 5000
    }

    fn proxy_frame_of(window_id: u32) -> CGRect {
        frame(
            10.0 * window_id as f64,
            20.0,
            300.0 + window_id as f64,
            200.0,
        )
    }

    fn target_frame_of(window_id: u32) -> CGRect {
        frame(-700.0, 400.0 + window_id as f64, 640.0, 480.0)
    }

    fn proxy_standing_in_for(window_id: u32) -> WindowProxy {
        WindowProxy {
            real_window_id: WindowId(window_id),
            id: proxy_window_id_of(window_id),
            frame: proxy_frame_of(window_id),
            level: 0,
            sub_level: 0,
            context: None,
            image: None,
        }
    }

    fn begin_moving_with_held_layers(animated_windows: &mut AnimatedWindows, window_ids: &[u32]) {
        for window_id in window_ids {
            animated_windows.begin_moving_with_a_held_layer(
                proxy_standing_in_for(*window_id),
                target_frame_of(*window_id),
                DURATION_IN_SECONDS,
                AnimationEasingType::EaseOutQuad,
            );
        }
    }

    fn animated_windows_set_in_motion(window_ids: &[u32]) -> AnimatedWindows {
        let mut animated_windows = AnimatedWindows::new();
        begin_moving_with_held_layers(&mut animated_windows, window_ids);
        animated_windows.release_layers_held_until_their_request_is_in_motion();
        animated_windows
    }

    fn advance_to_display_frame(
        animated_windows: &mut AnimatedWindows,
        display_frame_index: u64,
    ) -> AnimatedWindowsAdvancedToATick {
        let tick_host_time =
            HOST_TIME_OF_THE_FIRST_TICK + display_frame_index * HOST_TICKS_PER_DISPLAY_FRAME;
        animated_windows.advance_every_moving_window_to_display_link_tick(
            tick_host_time,
            tick_host_time + HOST_TICKS_PER_DISPLAY_FRAME,
            HOST_CLOCK_FREQUENCY,
        )
    }

    fn phase_of(animated_windows: &AnimatedWindows, window_id: u32) -> Phase {
        match animated_windows
            .animated_window_list
            .iter()
            .find(|animated_window| animated_window.window_id == WindowId(window_id))
            .map(|animated_window| &animated_window.phase)
        {
            Some(AnimatedWindowPhase::Moving(_)) => Phase::Moving,
            Some(AnimatedWindowPhase::AwaitingProxySwapOut) => Phase::AwaitingItsProxySwapOut,
            None => Phase::Absent,
        }
    }

    fn proxy_window_ids(proxy_list: &[WindowProxy]) -> Vec<u32> {
        proxy_list.iter().map(|proxy| proxy.id).collect()
    }

    fn animated_windows_with_one_window_in_each_phase() -> AnimatedWindows {
        let mut animated_windows = animated_windows_set_in_motion(&[1, 2]);
        advance_to_display_frame(&mut animated_windows, 0);
        let handed_over_proxies = animated_windows.hand_every_moving_proxy_over_for_swap_out();
        assert_eq!(proxy_window_ids(&handed_over_proxies).len(), 2);
        animated_windows.forget_windows_whose_proxy_was_swapped_out(&[WindowId(2)]);
        begin_moving_with_held_layers(&mut animated_windows, &[3]);
        animated_windows.release_layers_held_until_their_request_is_in_motion();
        animated_windows
    }

    #[test]
    fn a_moving_window_is_handed_over_for_swap_out_on_the_tick_its_layers_run_their_course() {
        let mut animated_windows = animated_windows_set_in_motion(&[1]);

        for display_frame_index in
            0..INDEX_OF_THE_TICK_A_WINDOW_STARTED_ON_THE_FIRST_TICK_COMES_TO_REST
        {
            let advanced = advance_to_display_frame(&mut animated_windows, display_frame_index);

            assert!(advanced.proxies_that_came_to_rest.is_empty());
            assert_eq!(phase_of(&animated_windows, 1), Phase::Moving);
        }

        let advanced = advance_to_display_frame(
            &mut animated_windows,
            INDEX_OF_THE_TICK_A_WINDOW_STARTED_ON_THE_FIRST_TICK_COMES_TO_REST,
        );

        assert_eq!(
            proxy_window_ids(&advanced.proxies_that_came_to_rest),
            vec![proxy_window_id_of(1)]
        );
        assert_eq!(
            x_y_width_height(advanced.proxy_frame_update_list[0].displayed_frame),
            x_y_width_height(target_frame_of(1))
        );
        assert_eq!(
            phase_of(&animated_windows, 1),
            Phase::AwaitingItsProxySwapOut
        );
        assert!(animated_windows.is_awaiting_proxy_swap_out(WindowId(1)));
        assert!(!animated_windows.has_a_moving_window());
    }

    #[test]
    fn a_window_that_came_to_rest_is_handed_over_exactly_once() {
        let mut animated_windows = animated_windows_set_in_motion(&[1]);
        let mut proxies_handed_over = Vec::new();

        for display_frame_index in 0..40 {
            let advanced = advance_to_display_frame(&mut animated_windows, display_frame_index);
            if display_frame_index
                > INDEX_OF_THE_TICK_A_WINDOW_STARTED_ON_THE_FIRST_TICK_COMES_TO_REST
            {
                assert!(advanced.proxy_frame_update_list.is_empty());
            }
            proxies_handed_over.extend(advanced.proxies_that_came_to_rest);
        }
        proxies_handed_over.extend(animated_windows.hand_every_moving_proxy_over_for_swap_out());

        assert_eq!(
            proxy_window_ids(&proxies_handed_over),
            vec![proxy_window_id_of(1)]
        );
    }

    #[test]
    fn windows_come_to_rest_each_on_its_own_tick() {
        let mut animated_windows = animated_windows_set_in_motion(&[1]);
        advance_to_display_frame(&mut animated_windows, 0);
        advance_to_display_frame(&mut animated_windows, 1);
        advance_to_display_frame(&mut animated_windows, 2);
        begin_moving_with_held_layers(&mut animated_windows, &[2]);
        animated_windows.release_layers_held_until_their_request_is_in_motion();
        let mut ticks_on_which_each_proxy_came_to_rest = Vec::new();

        for display_frame_index in 3..20 {
            let advanced = advance_to_display_frame(&mut animated_windows, display_frame_index);
            for proxy in advanced.proxies_that_came_to_rest {
                ticks_on_which_each_proxy_came_to_rest.push((proxy.id, display_frame_index));
            }
        }

        assert_eq!(
            ticks_on_which_each_proxy_came_to_rest,
            vec![
                (
                    proxy_window_id_of(1),
                    INDEX_OF_THE_TICK_A_WINDOW_STARTED_ON_THE_FIRST_TICK_COMES_TO_REST
                ),
                (
                    proxy_window_id_of(2),
                    3 + INDEX_OF_THE_TICK_A_WINDOW_STARTED_ON_THE_FIRST_TICK_COMES_TO_REST
                ),
            ]
        );
    }

    #[test]
    fn a_window_whose_layer_is_held_shows_its_proxy_frame_and_keeps_moving_on_every_tick() {
        let mut animated_windows = AnimatedWindows::new();
        begin_moving_with_held_layers(&mut animated_windows, &[1]);

        for display_frame_index in 0..40 {
            let advanced = advance_to_display_frame(&mut animated_windows, display_frame_index);

            assert!(advanced.proxies_that_came_to_rest.is_empty());
            assert_eq!(
                x_y_width_height(advanced.proxy_frame_update_list[0].displayed_frame),
                x_y_width_height(proxy_frame_of(1))
            );
        }
        assert_eq!(phase_of(&animated_windows, 1), Phase::Moving);
    }

    #[test]
    fn only_a_moving_window_is_retargeted() {
        let mut animated_windows = animated_windows_with_one_window_in_each_phase();

        let was_retargeted_by_window_id: Vec<(u32, bool)> = [1, 2, 3]
            .into_iter()
            .map(|window_id| {
                (
                    window_id,
                    animated_windows.retarget_if_moving(
                        WindowId(window_id),
                        frame(0.0, 0.0, 10.0, 10.0),
                        DURATION_IN_SECONDS,
                        AnimationEasingType::EaseOutQuad,
                    ),
                )
            })
            .collect();

        assert_eq!(
            [
                phase_of(&animated_windows, 1),
                phase_of(&animated_windows, 2),
                phase_of(&animated_windows, 3),
            ],
            [Phase::AwaitingItsProxySwapOut, Phase::Absent, Phase::Moving]
        );
        assert_eq!(
            was_retargeted_by_window_id,
            vec![(1, false), (2, false), (3, true)]
        );
    }

    #[test]
    fn forgetting_removes_only_the_named_windows_that_await_their_swap_out() {
        let mut animated_windows = animated_windows_set_in_motion(&[1, 2, 3]);
        advance_to_display_frame(&mut animated_windows, 0);
        animated_windows.hand_every_moving_proxy_over_for_swap_out();
        begin_moving_with_held_layers(&mut animated_windows, &[4]);

        animated_windows.forget_windows_whose_proxy_was_swapped_out(&[
            WindowId(1),
            WindowId(3),
            WindowId(4),
        ]);

        assert_eq!(
            [1, 2, 3, 4].map(|window_id| phase_of(&animated_windows, window_id)),
            [
                Phase::Absent,
                Phase::AwaitingItsProxySwapOut,
                Phase::Absent,
                Phase::Moving
            ]
        );
    }

    #[test]
    fn handing_every_moving_proxy_over_hands_each_once_and_leaves_no_window_moving() {
        let mut animated_windows = animated_windows_with_one_window_in_each_phase();
        begin_moving_with_held_layers(&mut animated_windows, &[4]);

        let first_hand_over = animated_windows.hand_every_moving_proxy_over_for_swap_out();
        let second_hand_over = animated_windows.hand_every_moving_proxy_over_for_swap_out();

        assert_eq!(
            proxy_window_ids(&first_hand_over),
            vec![proxy_window_id_of(3), proxy_window_id_of(4)]
        );
        assert!(second_hand_over.is_empty());
        assert!(!animated_windows.has_a_moving_window());
        assert_eq!(
            [1, 3, 4].map(|window_id| phase_of(&animated_windows, window_id)),
            [
                Phase::AwaitingItsProxySwapOut,
                Phase::AwaitingItsProxySwapOut,
                Phase::AwaitingItsProxySwapOut
            ]
        );
    }

    #[test]
    fn each_frame_update_names_the_window_its_proxy_and_the_proxy_size_and_refreshes_alpha_every_thirtieth_of_a_second()
     {
        let mut animated_windows = animated_windows_set_in_motion(&[7, 8]);

        let alpha_refreshes_on_each_tick: Vec<Vec<bool>> = (0..5)
            .map(|display_frame_index| {
                let advanced = advance_to_display_frame(&mut animated_windows, display_frame_index);
                for (proxy_frame_update, window_id) in
                    advanced.proxy_frame_update_list.iter().zip([7, 8])
                {
                    assert_eq!(proxy_frame_update.real_window_id.0, window_id);
                    assert_eq!(
                        proxy_frame_update.proxy_window_id,
                        proxy_window_id_of(window_id)
                    );
                    assert_eq!(
                        [
                            proxy_frame_update.proxy_frame_size.width,
                            proxy_frame_update.proxy_frame_size.height
                        ],
                        [
                            proxy_frame_of(window_id).size.width,
                            proxy_frame_of(window_id).size.height
                        ]
                    );
                }
                advanced
                    .proxy_frame_update_list
                    .iter()
                    .map(|proxy_frame_update| proxy_frame_update.must_refresh_alpha)
                    .collect()
            })
            .collect();

        assert_eq!(
            alpha_refreshes_on_each_tick,
            vec![
                vec![false, false],
                vec![false, false],
                vec![true, true],
                vec![false, false],
                vec![true, true],
            ]
        );
    }
}
