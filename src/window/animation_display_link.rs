#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::Arc;

use crate::ffi::core_foundation::{CFRetained, SendCFRetained};
use crate::ffi::core_video::{
    CVDisplayLink, CVDisplayLinkCreateWithActiveCGDisplays, CVDisplayLinkSetOutputCallback,
    CVDisplayLinkStart, CVDisplayLinkStop, CVOptionFlags, CVReturn, CVTimeStamp,
    kCVReturnDisplayLinkAlreadyRunning, kCVReturnSuccess,
};
use crate::window::animation_tick::window_animator_run_one_display_link_tick;
use crate::window::animator::WindowAnimator;

pub(crate) struct AnimationDisplayLink {
    display_link: SendCFRetained<CVDisplayLink>,
    window_animator_kept_alive_for_the_callback: *const WindowAnimator,
}

unsafe impl Send for AnimationDisplayLink {}

impl AnimationDisplayLink {
    pub(crate) fn is_the_display_link(&self, display_link: &CVDisplayLink) -> bool {
        core::ptr::eq(self.display_link.as_ref(), display_link)
    }
}

impl Drop for AnimationDisplayLink {
    fn drop(&mut self) {
        CVDisplayLinkStop(self.display_link.as_ref());
        drop(unsafe { Arc::from_raw(self.window_animator_kept_alive_for_the_callback) });
    }
}

pub(crate) fn animation_display_link_start(
    window_animator: &Arc<WindowAnimator>,
) -> Option<AnimationDisplayLink> {
    let mut display_link_pointer: *mut CVDisplayLink = core::ptr::null_mut();
    let creation_result = unsafe {
        CVDisplayLinkCreateWithActiveCGDisplays(NonNull::from(&mut display_link_pointer))
    };
    let display_link = NonNull::new(display_link_pointer)
        .map(|display_link_pointer| unsafe { CFRetained::from_raw(display_link_pointer) });
    if creation_result != kCVReturnSuccess {
        return None;
    }

    let animation_display_link = AnimationDisplayLink {
        display_link: SendCFRetained(display_link?),
        window_animator_kept_alive_for_the_callback: Arc::into_raw(Arc::clone(window_animator)),
    };

    let callback_result = unsafe {
        CVDisplayLinkSetOutputCallback(
            animation_display_link.display_link.as_ref(),
            Some(window_manager_animate_window_list_thread_proc),
            animation_display_link
                .window_animator_kept_alive_for_the_callback
                .cast::<c_void>()
                .cast_mut(),
        )
    };
    if callback_result != kCVReturnSuccess {
        return None;
    }

    let start_result = CVDisplayLinkStart(animation_display_link.display_link.as_ref());
    if start_result != kCVReturnSuccess && start_result != kCVReturnDisplayLinkAlreadyRunning {
        return None;
    }

    Some(animation_display_link)
}

pub(crate) fn animation_display_link_stop_from_its_own_callback(display_link: &CVDisplayLink) {
    CVDisplayLinkStop(display_link);
}

unsafe extern "C-unwind" fn window_manager_animate_window_list_thread_proc(
    link: NonNull<CVDisplayLink>,
    now: NonNull<CVTimeStamp>,
    output_time: NonNull<CVTimeStamp>,
    _flags: CVOptionFlags,
    _flags_out: NonNull<CVOptionFlags>,
    data: *mut c_void,
) -> CVReturn {
    let window_animator = unsafe { &*data.cast_const().cast::<WindowAnimator>() };
    window_animator_run_one_display_link_tick(
        window_animator,
        unsafe { link.as_ref() },
        unsafe { now.as_ref().hostTime },
        unsafe { output_time.as_ref().hostTime },
    );
    kCVReturnSuccess
}
