use std::sync::Weak;
use std::sync::mpsc::Sender;

use crate::ffi::skylight::{SLSNewConnection, SLSReleaseConnection};
use crate::window::animation_completion::{
    AnimationCompletionJob, spawn_animation_completion_worker,
};
use crate::window::animator::WindowAnimator;

pub(crate) struct WindowAnimatorResources {
    pub(crate) animation_connection: i32,
    completion_job_sender: Sender<AnimationCompletionJob>,
}

impl WindowAnimatorResources {
    pub(crate) fn hand_over_to_the_completion_worker(
        &self,
        completion_job: AnimationCompletionJob,
    ) {
        let _ = self.completion_job_sender.send(completion_job);
    }
}

pub(crate) fn start_window_animator_resources(
    window_animator: Weak<WindowAnimator>,
) -> Option<WindowAnimatorResources> {
    let mut animation_connection: i32 = 0;
    unsafe { SLSNewConnection(0, &mut animation_connection) };

    match spawn_animation_completion_worker(animation_connection, window_animator) {
        Some(completion_job_sender) => Some(WindowAnimatorResources {
            animation_connection,
            completion_job_sender,
        }),
        None => {
            unsafe { SLSReleaseConnection(animation_connection) };
            None
        }
    }
}
