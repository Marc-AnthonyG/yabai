use std::sync::Weak;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::ffi::skylight::SLSReleaseConnection;
use crate::scripting_addition::client::swap_window_proxies_out_through_scripting_addition;
use crate::support::handles::WindowId;
use crate::window::animation_display_link::AnimationDisplayLink;
use crate::window::animator::{
    WindowAnimator, forget_swapped_out_windows_waking_requests_that_wait_for_them,
};
use crate::window::janky_borders::notify_janky_borders_of_proxy_pairings;
use crate::window::proxy::{WindowProxy, destroy_window_proxy};
use crate::window::proxy_pairing::WindowProxyPairing;

pub(crate) enum AnimationCompletionJob {
    SwapOutAndReleaseProxies(Vec<WindowProxy>),
    StopAndReleaseDisplayLink(AnimationDisplayLink),
}

pub(crate) fn spawn_animation_completion_worker(
    animation_connection: i32,
    window_animator: Weak<WindowAnimator>,
) -> Option<Sender<AnimationCompletionJob>> {
    let (completion_job_sender, completion_job_receiver) = channel();
    std::thread::Builder::new()
        .name(String::from("window animation completion"))
        .spawn(move || {
            run_animation_completion_worker_until_the_animator_is_gone(
                animation_connection,
                completion_job_receiver,
                window_animator,
            )
        })
        .ok()?;
    Some(completion_job_sender)
}

fn run_animation_completion_worker_until_the_animator_is_gone(
    animation_connection: i32,
    completion_job_receiver: Receiver<AnimationCompletionJob>,
    window_animator: Weak<WindowAnimator>,
) {
    while let Ok(first_completion_job) = completion_job_receiver.recv() {
        let mut proxies_to_swap_out: Vec<WindowProxy> = Vec::new();
        let mut next_completion_job = Some(first_completion_job);
        while let Some(completion_job) = next_completion_job {
            match completion_job {
                AnimationCompletionJob::SwapOutAndReleaseProxies(proxy_list) => {
                    proxies_to_swap_out.extend(proxy_list);
                }
                AnimationCompletionJob::StopAndReleaseDisplayLink(display_link) => {
                    drop(display_link);
                }
            }
            next_completion_job = completion_job_receiver.try_recv().ok();
        }

        if !proxies_to_swap_out.is_empty() {
            swap_out_and_release_window_proxies(
                animation_connection,
                proxies_to_swap_out,
                &window_animator,
            );
        }
    }

    unsafe { SLSReleaseConnection(animation_connection) };
}

fn swap_out_and_release_window_proxies(
    animation_connection: i32,
    proxy_list: Vec<WindowProxy>,
    window_animator: &Weak<WindowAnimator>,
) {
    let pairing_list: Vec<WindowProxyPairing> =
        proxy_list.iter().map(WindowProxy::pairing).collect();

    notify_janky_borders_of_proxy_pairings(&pairing_list, 1326, true);
    swap_window_proxies_out_through_scripting_addition(&pairing_list);

    if let Some(window_animator) = window_animator.upgrade() {
        let window_id_list: Vec<WindowId> = pairing_list
            .iter()
            .map(|pairing| pairing.real_window_id)
            .collect();
        forget_swapped_out_windows_waking_requests_that_wait_for_them(
            &window_animator,
            &window_id_list,
        );
    }

    for proxy in proxy_list {
        destroy_window_proxy(animation_connection, proxy);
    }
}
