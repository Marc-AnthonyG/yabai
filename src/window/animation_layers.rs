use crate::ffi::core_foundation::CGRect;
use crate::support::easing::AnimationEasingType;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct FrameDelta {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl FrameDelta {
    pub(crate) fn from_frame_to_frame(from_frame: CGRect, to_frame: CGRect) -> FrameDelta {
        FrameDelta {
            x: to_frame.origin.x - from_frame.origin.x,
            y: to_frame.origin.y - from_frame.origin.y,
            width: to_frame.size.width - from_frame.size.width,
            height: to_frame.size.height - from_frame.size.height,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum AdditiveLayerStart {
    HeldUntilItsRequestIsInMotion,
    OnTheNextDisplayLinkTick,
    AtHostTime(u64),
}

#[derive(Clone, Copy)]
pub(crate) struct AdditiveFrameLayer {
    pub(crate) frame_delta: FrameDelta,
    pub(crate) start: AdditiveLayerStart,
    pub(crate) duration_in_seconds: f32,
    pub(crate) easing: AnimationEasingType,
}

impl AdditiveFrameLayer {
    fn progress_at(&self, host_time: u64, host_clock_frequency: f64) -> f64 {
        let AdditiveLayerStart::AtHostTime(start_host_time) = self.start else {
            return 0.0;
        };

        let duration_in_host_ticks = self.duration_in_seconds as f64 * host_clock_frequency;
        if duration_in_host_ticks.is_nan() || duration_in_host_ticks <= 0.0 {
            return 1.0;
        }

        let elapsed_host_ticks = host_time.saturating_sub(start_host_time) as f64;
        (elapsed_host_ticks / duration_in_host_ticks).clamp(0.0, 1.0)
    }

    fn has_run_its_course_at(&self, host_time: u64, host_clock_frequency: f64) -> bool {
        self.progress_at(host_time, host_clock_frequency) >= 1.0
    }

    fn share_of_delta_still_ahead_at(&self, host_time: u64, host_clock_frequency: f64) -> f64 {
        let progress = self.progress_at(host_time, host_clock_frequency);
        if progress >= 1.0 {
            return 0.0;
        }
        1.0 - self.easing.apply(progress as f32) as f64
    }
}

pub(crate) struct LayeredFrameAnimation {
    target_frame: CGRect,
    layers: Vec<AdditiveFrameLayer>,
}

impl LayeredFrameAnimation {
    pub(crate) fn from_current_frame_towards_target_with_a_held_layer(
        current_frame: CGRect,
        target_frame: CGRect,
        duration_in_seconds: f32,
        easing: AnimationEasingType,
    ) -> LayeredFrameAnimation {
        let mut layered_frame_animation = LayeredFrameAnimation {
            target_frame: current_frame,
            layers: Vec::new(),
        };
        layered_frame_animation.retarget_with_a_held_layer(
            target_frame,
            duration_in_seconds,
            easing,
        );
        layered_frame_animation
    }

    pub(crate) fn retarget_with_a_held_layer(
        &mut self,
        new_target_frame: CGRect,
        duration_in_seconds: f32,
        easing: AnimationEasingType,
    ) {
        self.layers.push(AdditiveFrameLayer {
            frame_delta: FrameDelta::from_frame_to_frame(self.target_frame, new_target_frame),
            start: AdditiveLayerStart::HeldUntilItsRequestIsInMotion,
            duration_in_seconds,
            easing,
        });
        self.target_frame = new_target_frame;
    }

    pub(crate) fn release_layers_held_until_their_request_is_in_motion(&mut self) {
        for layer in &mut self.layers {
            if layer.start == AdditiveLayerStart::HeldUntilItsRequestIsInMotion {
                layer.start = AdditiveLayerStart::OnTheNextDisplayLinkTick;
            }
        }
    }

    pub(crate) fn start_layers_waiting_for_the_display_link(&mut self, tick_host_time: u64) {
        for layer in &mut self.layers {
            if layer.start == AdditiveLayerStart::OnTheNextDisplayLinkTick {
                layer.start = AdditiveLayerStart::AtHostTime(tick_host_time);
            }
        }
    }

    pub(crate) fn displayed_frame_at(&self, host_time: u64, host_clock_frequency: f64) -> CGRect {
        let mut displayed_frame = self.target_frame;
        for layer in &self.layers {
            let share_of_delta_still_ahead =
                layer.share_of_delta_still_ahead_at(host_time, host_clock_frequency);
            displayed_frame.origin.x -= layer.frame_delta.x * share_of_delta_still_ahead;
            displayed_frame.origin.y -= layer.frame_delta.y * share_of_delta_still_ahead;
            displayed_frame.size.width -= layer.frame_delta.width * share_of_delta_still_ahead;
            displayed_frame.size.height -= layer.frame_delta.height * share_of_delta_still_ahead;
        }
        displayed_frame
    }

    pub(crate) fn retire_layers_that_have_run_their_course(
        &mut self,
        host_time: u64,
        host_clock_frequency: f64,
    ) {
        self.layers
            .retain(|layer| !layer.has_run_its_course_at(host_time, host_clock_frequency));
    }

    pub(crate) fn advance_to_display_link_tick(
        &mut self,
        tick_host_time: u64,
        output_host_time: u64,
        host_clock_frequency: f64,
    ) -> CGRect {
        self.start_layers_waiting_for_the_display_link(tick_host_time);
        let displayed_frame = self.displayed_frame_at(output_host_time, host_clock_frequency);
        self.retire_layers_that_have_run_their_course(output_host_time, host_clock_frequency);
        displayed_frame
    }

    pub(crate) fn has_come_to_rest(&self) -> bool {
        self.layers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::LayeredFrameAnimation;
    use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};
    use crate::support::easing::AnimationEasingType;

    const HOST_CLOCK_FREQUENCY: f64 = 24_000_000.0;
    const HOST_TICKS_PER_DISPLAY_FRAME: u64 = 400_000;
    const HOST_TIME_OF_THE_FIRST_TICK: u64 = 1_000_000_000;
    const DURATION_OF_THE_C_PROBE_IN_SECONDS: f32 = 0.35;
    const STEEPEST_SLOPE_OF_EASE_OUT_CUBIC: f64 = 3.0;

    const FRAME_THE_C_FORMULA_STORED_IN_F32_AT_EACH_HOST_TIME_AFTER_THE_FIRST_TICK: [(
        AnimationEasingType,
        u64,
        [f64; 4],
    ); 32] = [
        (
            AnimationEasingType::EaseOutCubic,
            0,
            [100.0, 200.0, 800.0, 600.0],
        ),
        (
            AnimationEasingType::EaseOutCubic,
            400_000,
            [
                258.0164794921875,
                177.9416961669922,
                868.0811767578125,
                660.047607421875,
            ],
        ),
        (
            AnimationEasingType::EaseOutCubic,
            2_100_000,
            [770.9140625, 106.34375, 1089.0625, 854.953125],
        ),
        (
            AnimationEasingType::EaseOutCubic,
            4_200_000,
            [1115.4375, 58.25, 1237.5, 985.875],
        ),
        (
            AnimationEasingType::EaseOutCubic,
            6_300_000,
            [1242.3671875, 40.53125, 1292.1875, 1034.109375],
        ),
        (
            AnimationEasingType::EaseOutCubic,
            8_399_999,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseOutCubic,
            8_400_000,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseOutCubic,
            9_000_000,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseInOutSine,
            0,
            [100.0, 200.0, 800.0, 600.0],
        ),
        (
            AnimationEasingType::EaseInOutSine,
            400_000,
            [
                106.48092651367188,
                199.0952911376953,
                802.7922973632812,
                602.4628295898438,
            ],
        ),
        (
            AnimationEasingType::EaseInOutSine,
            2_100_000,
            [
                269.9512939453125,
                176.27565002441406,
                873.2233276367188,
                664.5829467773438,
            ],
        ),
        (
            AnimationEasingType::EaseInOutSine,
            4_200_000,
            [680.25, 119.0, 1050.0, 820.5],
        ),
        (
            AnimationEasingType::EaseInOutSine,
            6_300_000,
            [
                1090.5487060546875,
                61.72434616088867,
                1226.776611328125,
                976.4171142578125,
            ],
        ),
        (
            AnimationEasingType::EaseInOutSine,
            8_399_999,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseInOutSine,
            8_400_000,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseInOutSine,
            9_000_000,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseOutCirc,
            0,
            [100.0, 200.0, 800.0, 600.0],
        ),
        (
            AnimationEasingType::EaseOutCirc,
            400_000,
            [
                453.8487854003906,
                150.60447692871094,
                952.455322265625,
                734.465576171875,
            ],
        ),
        (
            AnimationEasingType::EaseOutCirc,
            2_100_000,
            [
                867.5985717773438,
                92.84707641601562,
                1130.7188720703125,
                891.694091796875,
            ],
        ),
        (
            AnimationEasingType::EaseOutCirc,
            4_200_000,
            [
                1105.0224609375,
                59.703887939453125,
                1233.0126953125,
                981.9171752929688,
            ],
        ),
        (
            AnimationEasingType::EaseOutCirc,
            6_300_000,
            [
                1223.6492919921875,
                43.144168853759766,
                1284.1229248046875,
                1026.9964599609375,
            ],
        ),
        (
            AnimationEasingType::EaseOutCirc,
            8_399_999,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseOutCirc,
            8_400_000,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseOutCirc,
            9_000_000,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseInOutCubic,
            0,
            [100.0, 200.0, 800.0, 600.0],
        ),
        (
            AnimationEasingType::EaseInOutCubic,
            400_000,
            [
                100.5012435913086,
                199.93002319335938,
                800.2159423828125,
                600.1904907226562,
            ],
        ),
        (
            AnimationEasingType::EaseInOutCubic,
            2_100_000,
            [172.53125, 189.875, 831.25, 627.5625],
        ),
        (
            AnimationEasingType::EaseInOutCubic,
            4_200_000,
            [680.25, 119.0, 1050.0, 820.5],
        ),
        (
            AnimationEasingType::EaseInOutCubic,
            6_300_000,
            [1187.96875, 48.125, 1268.75, 1013.4375],
        ),
        (
            AnimationEasingType::EaseInOutCubic,
            8_399_999,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseInOutCubic,
            8_400_000,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
        (
            AnimationEasingType::EaseInOutCubic,
            9_000_000,
            [1260.5, 38.0, 1300.0, 1041.0],
        ),
    ];

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

    fn assert_frame_is_within(actual: CGRect, expected: [f64; 4], tolerance: f64, context: &str) {
        let actual = x_y_width_height(actual);
        for (actual_component, expected_component) in actual.into_iter().zip(expected) {
            assert!(
                (actual_component - expected_component).abs() <= tolerance,
                "{context}: displayed {actual:?}, expected {expected:?}"
            );
        }
    }

    fn host_time_of_display_frame(display_frame_index: u64) -> u64 {
        HOST_TIME_OF_THE_FIRST_TICK + display_frame_index * HOST_TICKS_PER_DISPLAY_FRAME
    }

    fn advance_to_display_frame(
        layered_frame_animation: &mut LayeredFrameAnimation,
        display_frame_index: u64,
    ) -> CGRect {
        let tick_host_time = host_time_of_display_frame(display_frame_index);
        layered_frame_animation.advance_to_display_link_tick(
            tick_host_time,
            tick_host_time + HOST_TICKS_PER_DISPLAY_FRAME,
            HOST_CLOCK_FREQUENCY,
        )
    }

    fn animation_started_on_the_tick_at(
        current_frame: CGRect,
        target_frame: CGRect,
        duration_in_seconds: f32,
        easing: AnimationEasingType,
        tick_host_time: u64,
    ) -> LayeredFrameAnimation {
        let mut layered_frame_animation =
            LayeredFrameAnimation::from_current_frame_towards_target_with_a_held_layer(
                current_frame,
                target_frame,
                duration_in_seconds,
                easing,
            );
        layered_frame_animation.release_layers_held_until_their_request_is_in_motion();
        layered_frame_animation.start_layers_waiting_for_the_display_link(tick_host_time);
        layered_frame_animation
    }

    fn retarget_on_the_tick_at(
        layered_frame_animation: &mut LayeredFrameAnimation,
        new_target_frame: CGRect,
        duration_in_seconds: f32,
        easing: AnimationEasingType,
        tick_host_time: u64,
    ) {
        layered_frame_animation.retarget_with_a_held_layer(
            new_target_frame,
            duration_in_seconds,
            easing,
        );
        layered_frame_animation.release_layers_held_until_their_request_is_in_motion();
        layered_frame_animation.start_layers_waiting_for_the_display_link(tick_host_time);
    }

    #[test]
    fn a_held_layer_shows_the_current_frame_on_every_tick_until_its_request_is_in_motion() {
        let current_frame = frame(100.0, 200.0, 800.0, 600.0);
        let mut layered_frame_animation =
            LayeredFrameAnimation::from_current_frame_towards_target_with_a_held_layer(
                current_frame,
                frame(1260.5, 38.0, 1300.0, 1041.0),
                DURATION_OF_THE_C_PROBE_IN_SECONDS,
                AnimationEasingType::EaseOutCubic,
            );

        for display_frame_index in 0..60 {
            let displayed_frame =
                advance_to_display_frame(&mut layered_frame_animation, display_frame_index);

            assert_eq!(
                x_y_width_height(displayed_frame),
                x_y_width_height(current_frame),
                "display frame {display_frame_index}"
            );
            assert!(!layered_frame_animation.has_come_to_rest());
        }

        layered_frame_animation.release_layers_held_until_their_request_is_in_motion();
        assert_eq!(
            x_y_width_height(
                layered_frame_animation
                    .displayed_frame_at(host_time_of_display_frame(1000), HOST_CLOCK_FREQUENCY)
            ),
            x_y_width_height(current_frame)
        );

        let displayed_frame = advance_to_display_frame(&mut layered_frame_animation, 60);
        assert_ne!(
            x_y_width_height(displayed_frame),
            x_y_width_height(current_frame)
        );
    }

    #[test]
    fn one_layer_displays_the_frame_the_c_formula_lerps_to_the_f32_precision_the_c_stored_it_in() {
        for (easing, host_ticks_after_the_first_tick, frame_the_c_stored_in_f32) in
            FRAME_THE_C_FORMULA_STORED_IN_F32_AT_EACH_HOST_TIME_AFTER_THE_FIRST_TICK
        {
            let mut layered_frame_animation =
                LayeredFrameAnimation::from_current_frame_towards_target_with_a_held_layer(
                    frame(100.0, 200.0, 800.0, 600.0),
                    frame(1260.5, 38.0, 1300.0, 1041.0),
                    DURATION_OF_THE_C_PROBE_IN_SECONDS,
                    easing,
                );
            layered_frame_animation.release_layers_held_until_their_request_is_in_motion();

            let displayed_frame = layered_frame_animation.advance_to_display_link_tick(
                HOST_TIME_OF_THE_FIRST_TICK,
                HOST_TIME_OF_THE_FIRST_TICK + host_ticks_after_the_first_tick,
                HOST_CLOCK_FREQUENCY,
            );

            let target_frame = x_y_width_height(frame(1260.5, 38.0, 1300.0, 1041.0));
            for component in 0..4 {
                let one_f32_rounding_of_the_c_product_and_of_its_result = f32::EPSILON as f64
                    * (target_frame[component].abs() + frame_the_c_stored_in_f32[component].abs());
                assert!(
                    (x_y_width_height(displayed_frame)[component]
                        - frame_the_c_stored_in_f32[component])
                        .abs()
                        <= one_f32_rounding_of_the_c_product_and_of_its_result,
                    "easing {} at {host_ticks_after_the_first_tick} host ticks: displayed {:?}, the c stored {frame_the_c_stored_in_f32:?}",
                    easing as usize,
                    x_y_width_height(displayed_frame)
                );
            }
        }
    }

    #[test]
    fn one_layer_comes_to_rest_on_the_first_tick_whose_output_time_reaches_its_duration() {
        for (host_ticks_after_the_first_tick, expected_to_be_at_rest) in
            [(8_399_999, false), (8_400_000, true), (9_000_000, true)]
        {
            let mut layered_frame_animation =
                LayeredFrameAnimation::from_current_frame_towards_target_with_a_held_layer(
                    frame(100.0, 200.0, 800.0, 600.0),
                    frame(1260.5, 38.0, 1300.0, 1041.0),
                    DURATION_OF_THE_C_PROBE_IN_SECONDS,
                    AnimationEasingType::EaseOutCirc,
                );
            layered_frame_animation.release_layers_held_until_their_request_is_in_motion();

            layered_frame_animation.advance_to_display_link_tick(
                HOST_TIME_OF_THE_FIRST_TICK,
                HOST_TIME_OF_THE_FIRST_TICK + host_ticks_after_the_first_tick,
                HOST_CLOCK_FREQUENCY,
            );

            assert_eq!(
                layered_frame_animation.has_come_to_rest(),
                expected_to_be_at_rest,
                "{host_ticks_after_the_first_tick} host ticks after the first tick"
            );
        }
    }

    #[test]
    fn a_retarget_leaves_the_displayed_frame_unchanged_at_the_instant_it_is_made() {
        let mut layered_frame_animation = animation_started_on_the_tick_at(
            frame(100.0, 200.0, 800.0, 600.0),
            frame(1260.5, 38.0, 1300.0, 1041.0),
            DURATION_OF_THE_C_PROBE_IN_SECONDS,
            AnimationEasingType::EaseOutCirc,
            host_time_of_display_frame(0),
        );
        let instant_of_the_retargets = host_time_of_display_frame(4) + 123_457;
        let displayed_frame_before_the_retargets = x_y_width_height(
            layered_frame_animation
                .displayed_frame_at(instant_of_the_retargets, HOST_CLOCK_FREQUENCY),
        );

        for (new_target_frame, duration_in_seconds, easing) in [
            (
                frame(-1728.0, 38.0, 864.0, 1079.0),
                0.2,
                AnimationEasingType::EaseInOutCubic,
            ),
            (
                frame(-1728.0, 38.0, 864.0, 1079.0),
                0.5,
                AnimationEasingType::EaseOutQuad,
            ),
            (
                frame(0.1, 0.2, 0.3, 0.4),
                0.35,
                AnimationEasingType::EaseInExpo,
            ),
        ] {
            layered_frame_animation.retarget_with_a_held_layer(
                new_target_frame,
                duration_in_seconds,
                easing,
            );
            assert_frame_is_within(
                layered_frame_animation
                    .displayed_frame_at(instant_of_the_retargets, HOST_CLOCK_FREQUENCY),
                displayed_frame_before_the_retargets,
                1e-9,
                "while the new layer is held",
            );

            layered_frame_animation.release_layers_held_until_their_request_is_in_motion();
            assert_frame_is_within(
                layered_frame_animation
                    .displayed_frame_at(instant_of_the_retargets, HOST_CLOCK_FREQUENCY),
                displayed_frame_before_the_retargets,
                1e-9,
                "once the new layer waits for the next tick",
            );
        }
    }

    #[test]
    fn once_every_layer_has_run_its_course_the_frame_is_exactly_the_latest_target_and_at_rest() {
        let latest_target_frame = frame(-311.25, 47.5, 977.0, 613.0);
        let mut layered_frame_animation = animation_started_on_the_tick_at(
            frame(100.0, 200.0, 800.0, 600.0),
            frame(1260.5, 38.0, 1300.0, 1041.0),
            DURATION_OF_THE_C_PROBE_IN_SECONDS,
            AnimationEasingType::EaseOutCirc,
            host_time_of_display_frame(0),
        );
        let mut displayed_frames = Vec::new();

        for display_frame_index in 0..200 {
            if display_frame_index == 5 {
                retarget_on_the_tick_at(
                    &mut layered_frame_animation,
                    frame(2000.0, 900.0, 640.0, 480.0),
                    0.5,
                    AnimationEasingType::EaseInOutSine,
                    host_time_of_display_frame(display_frame_index),
                );
            }
            if display_frame_index == 9 {
                retarget_on_the_tick_at(
                    &mut layered_frame_animation,
                    latest_target_frame,
                    0.2,
                    AnimationEasingType::EaseOutCubic,
                    host_time_of_display_frame(display_frame_index),
                );
            }

            displayed_frames.push(advance_to_display_frame(
                &mut layered_frame_animation,
                display_frame_index,
            ));
            if layered_frame_animation.has_come_to_rest() {
                break;
            }
        }

        assert!(layered_frame_animation.has_come_to_rest());
        let (last_displayed_frame, earlier_displayed_frames) =
            displayed_frames.split_last().unwrap();
        assert_eq!(
            x_y_width_height(*last_displayed_frame),
            x_y_width_height(latest_target_frame)
        );
        assert_ne!(
            x_y_width_height(*earlier_displayed_frames.last().unwrap()),
            x_y_width_height(latest_target_frame)
        );
    }

    #[test]
    fn layers_of_different_durations_and_easings_add_up_to_the_separate_animations_they_would_be() {
        let first_frame = frame(0.0, 0.0, 400.0, 300.0);
        let second_frame = frame(500.0, 100.0, 800.0, 600.0);
        let third_frame = frame(-300.0, 700.0, 1200.0, 400.0);
        let host_time_of_the_second_layer_start = host_time_of_display_frame(7);

        let mut layered_frame_animation = animation_started_on_the_tick_at(
            first_frame,
            second_frame,
            0.2,
            AnimationEasingType::EaseInQuad,
            host_time_of_display_frame(0),
        );
        retarget_on_the_tick_at(
            &mut layered_frame_animation,
            third_frame,
            0.5,
            AnimationEasingType::EaseOutCubic,
            host_time_of_the_second_layer_start,
        );
        let first_layer_alone = animation_started_on_the_tick_at(
            first_frame,
            second_frame,
            0.2,
            AnimationEasingType::EaseInQuad,
            host_time_of_display_frame(0),
        );
        let second_layer_alone = animation_started_on_the_tick_at(
            second_frame,
            third_frame,
            0.5,
            AnimationEasingType::EaseOutCubic,
            host_time_of_the_second_layer_start,
        );

        for host_time in (host_time_of_display_frame(0) - 50_000..host_time_of_display_frame(45))
            .step_by(133_333)
        {
            let first_alone = x_y_width_height(
                first_layer_alone.displayed_frame_at(host_time, HOST_CLOCK_FREQUENCY),
            );
            let second_alone = x_y_width_height(
                second_layer_alone.displayed_frame_at(host_time, HOST_CLOCK_FREQUENCY),
            );
            let second = x_y_width_height(second_frame);
            let sum_of_the_separate_animations: [f64; 4] = std::array::from_fn(|index| {
                first_alone[index] + second_alone[index] - second[index]
            });

            assert_frame_is_within(
                layered_frame_animation.displayed_frame_at(host_time, HOST_CLOCK_FREQUENCY),
                sum_of_the_separate_animations,
                1e-9,
                &format!("host time {host_time}"),
            );
        }
    }

    #[test]
    fn layers_expire_one_at_a_time_in_the_order_they_started() {
        let mut layered_frame_animation = animation_started_on_the_tick_at(
            frame(0.0, 0.0, 100.0, 100.0),
            frame(100.0, 0.0, 100.0, 100.0),
            0.1,
            AnimationEasingType::EaseOutQuad,
            host_time_of_display_frame(0),
        );
        let mut horizontal_deltas_of_the_layers_after_each_change = vec![vec![100.0]];

        for display_frame_index in 0..60 {
            if display_frame_index == 2 {
                retarget_on_the_tick_at(
                    &mut layered_frame_animation,
                    frame(300.0, 0.0, 100.0, 100.0),
                    0.1,
                    AnimationEasingType::EaseOutQuad,
                    host_time_of_display_frame(display_frame_index),
                );
            }
            if display_frame_index == 4 {
                retarget_on_the_tick_at(
                    &mut layered_frame_animation,
                    frame(600.0, 0.0, 100.0, 100.0),
                    0.1,
                    AnimationEasingType::EaseOutQuad,
                    host_time_of_display_frame(display_frame_index),
                );
            }

            advance_to_display_frame(&mut layered_frame_animation, display_frame_index);

            let horizontal_deltas_of_the_layers: Vec<f64> = layered_frame_animation
                .layers
                .iter()
                .map(|layer| layer.frame_delta.x)
                .collect();
            if horizontal_deltas_of_the_layers_after_each_change.last()
                != Some(&horizontal_deltas_of_the_layers)
            {
                horizontal_deltas_of_the_layers_after_each_change
                    .push(horizontal_deltas_of_the_layers);
            }
        }

        assert_eq!(
            horizontal_deltas_of_the_layers_after_each_change,
            vec![
                vec![100.0],
                vec![100.0, 200.0],
                vec![100.0, 200.0, 300.0],
                vec![200.0, 300.0],
                vec![300.0],
                vec![],
            ]
        );
    }

    #[test]
    fn a_layer_of_zero_or_negative_duration_reaches_its_target_on_its_first_tick() {
        for duration_in_seconds in [0.0f32, -0.0, -0.25, f32::NAN] {
            let current_frame = frame(100.0, 200.0, 800.0, 600.0);
            let target_frame = frame(1260.5, 38.0, 1300.0, 1041.0);
            let mut layered_frame_animation =
                LayeredFrameAnimation::from_current_frame_towards_target_with_a_held_layer(
                    current_frame,
                    target_frame,
                    duration_in_seconds,
                    AnimationEasingType::EaseOutCirc,
                );
            layered_frame_animation.release_layers_held_until_their_request_is_in_motion();
            assert_eq!(
                x_y_width_height(
                    layered_frame_animation
                        .displayed_frame_at(host_time_of_display_frame(0), HOST_CLOCK_FREQUENCY)
                ),
                x_y_width_height(current_frame),
                "duration {duration_in_seconds}"
            );

            let displayed_frame = advance_to_display_frame(&mut layered_frame_animation, 0);

            assert_eq!(
                x_y_width_height(displayed_frame),
                x_y_width_height(target_frame),
                "duration {duration_in_seconds}"
            );
            assert!(layered_frame_animation.has_come_to_rest());
        }
    }

    #[test]
    fn a_host_time_before_the_layer_started_counts_as_no_progress() {
        let current_frame = frame(100.0, 200.0, 800.0, 600.0);
        let layered_frame_animation = animation_started_on_the_tick_at(
            current_frame,
            frame(1260.5, 38.0, 1300.0, 1041.0),
            DURATION_OF_THE_C_PROBE_IN_SECONDS,
            AnimationEasingType::EaseOutCirc,
            HOST_TIME_OF_THE_FIRST_TICK,
        );

        for host_time in [
            0,
            HOST_TIME_OF_THE_FIRST_TICK - 1,
            HOST_TIME_OF_THE_FIRST_TICK,
        ] {
            assert_eq!(
                x_y_width_height(
                    layered_frame_animation.displayed_frame_at(host_time, HOST_CLOCK_FREQUENCY)
                ),
                x_y_width_height(current_frame),
                "host time {host_time}"
            );
        }
    }

    fn zigzagging_target_frame(retarget_index: u64) -> CGRect {
        let side = if retarget_index % 2 == 0 { 1.0 } else { -1.0 };
        frame(
            side * 2000.0 + retarget_index as f64 * 13.0,
            side * -900.0 + (retarget_index % 3) as f64 * 71.5,
            400.0 + (retarget_index % 7) as f64 * 150.0,
            300.0 + (retarget_index % 5) as f64 * 170.0,
        )
    }

    fn furthest_each_component_may_move_in_one_display_frame(
        layered_frame_animation: &LayeredFrameAnimation,
    ) -> [f64; 4] {
        let mut furthest_motion = [0.0f64; 4];
        for layer in &layered_frame_animation.layers {
            let share_of_the_layer_run_in_one_display_frame = HOST_TICKS_PER_DISPLAY_FRAME as f64
                / (layer.duration_in_seconds as f64 * HOST_CLOCK_FREQUENCY);
            let deltas = [
                layer.frame_delta.x,
                layer.frame_delta.y,
                layer.frame_delta.width,
                layer.frame_delta.height,
            ];
            for (furthest_component_motion, delta) in furthest_motion.iter_mut().zip(deltas) {
                *furthest_component_motion += delta.abs()
                    * (STEEPEST_SLOPE_OF_EASE_OUT_CUBIC
                        * share_of_the_layer_run_in_one_display_frame
                        + 1e-6);
            }
        }
        furthest_motion
    }

    #[test]
    fn many_retargets_in_a_row_move_the_displayed_frame_no_further_per_tick_than_its_layers_allow()
    {
        let mut layered_frame_animation =
            LayeredFrameAnimation::from_current_frame_towards_target_with_a_held_layer(
                frame(100.0, 200.0, 800.0, 600.0),
                zigzagging_target_frame(0),
                0.3,
                AnimationEasingType::EaseOutCubic,
            );
        layered_frame_animation.release_layers_held_until_their_request_is_in_motion();
        let mut previous_displayed_frame = x_y_width_height(frame(100.0, 200.0, 800.0, 600.0));
        let mut largest_step_seen = 0.0f64;

        for display_frame_index in 0..120 {
            if (1..60).contains(&display_frame_index) {
                layered_frame_animation.retarget_with_a_held_layer(
                    zigzagging_target_frame(display_frame_index),
                    0.3,
                    AnimationEasingType::EaseOutCubic,
                );
                layered_frame_animation.release_layers_held_until_their_request_is_in_motion();
            }
            let furthest_motion =
                furthest_each_component_may_move_in_one_display_frame(&layered_frame_animation);

            let displayed_frame = x_y_width_height(advance_to_display_frame(
                &mut layered_frame_animation,
                display_frame_index,
            ));

            for component in 0..4 {
                let step = (displayed_frame[component] - previous_displayed_frame[component]).abs();
                largest_step_seen = largest_step_seen.max(step);
                assert!(
                    step <= furthest_motion[component],
                    "display frame {display_frame_index} component {component}: moved {step}, at most {}",
                    furthest_motion[component]
                );
            }
            previous_displayed_frame = displayed_frame;
        }

        assert!(largest_step_seen > 1.0);
        assert!(layered_frame_animation.has_come_to_rest());
        assert_eq!(
            previous_displayed_frame,
            x_y_width_height(zigzagging_target_frame(59))
        );
    }
}
