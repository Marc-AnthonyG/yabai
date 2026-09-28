use crate::ffi::core_video::CVDisplayLink;
use crate::state::process_wide::CORE_VIDEO_HOST_CLOCK_FREQUENCY;
use crate::window::animation_completion::AnimationCompletionJob;
use crate::window::animation_display_link::stop_display_link_from_its_own_callback;
use crate::window::animation_frame_transaction::commit_proxy_frames_in_one_window_server_transaction;
use crate::window::animator::{
    WindowAnimator, advance_window_animator_to_display_link_tick, window_animator_started_resources,
};

pub(crate) fn run_one_window_animator_display_link_tick(
    window_animator: &WindowAnimator,
    display_link: &CVDisplayLink,
    tick_host_time: u64,
    output_host_time: u64,
) {
    let Some(window_animator_resources) = window_animator_started_resources(window_animator) else {
        return;
    };
    let host_clock_frequency = *CORE_VIDEO_HOST_CLOCK_FREQUENCY.get().unwrap_or(&0.0);

    let Some(display_link_tick_work) = advance_window_animator_to_display_link_tick(
        window_animator,
        display_link,
        tick_host_time,
        output_host_time,
        host_clock_frequency,
    ) else {
        return;
    };

    commit_proxy_frames_in_one_window_server_transaction(
        window_animator_resources.animation_connection,
        &display_link_tick_work.proxy_frame_update_list,
    );

    if !display_link_tick_work.proxies_that_came_to_rest.is_empty() {
        window_animator_resources.hand_over_to_the_completion_worker(
            AnimationCompletionJob::SwapOutAndReleaseProxies(
                display_link_tick_work.proxies_that_came_to_rest,
            ),
        );
    }

    if let Some(retired_display_link) = display_link_tick_work.retired_display_link {
        stop_display_link_from_its_own_callback(display_link);
        window_animator_resources.hand_over_to_the_completion_worker(
            AnimationCompletionJob::StopAndReleaseDisplayLink(retired_display_link),
        );
    }
}
