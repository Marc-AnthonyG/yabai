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
