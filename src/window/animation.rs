#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering, compiler_fence};
use std::sync::{Arc, Mutex};

use crate::ffi::core_foundation::{CFRetained, CGPoint, CGRect, CGSize, take_create_rule_result};
use crate::ffi::core_graphics::{
    CGAffineTransformConcat, CGAffineTransformMakeScale, CGAffineTransformMakeTranslation,
};
use crate::ffi::core_video::{
    CVDisplayLink, CVDisplayLinkCreateWithActiveCGDisplays, CVDisplayLinkSetOutputCallback,
    CVDisplayLinkStart, CVDisplayLinkStop, CVOptionFlags, CVReturn, CVTimeStamp, kCVReturnSuccess,
};
use crate::ffi::skylight::{
    SLSDisableUpdate, SLSGetWindowAlpha, SLSNewConnection, SLSReenableUpdate, SLSReleaseConnection,
    SLSTransactionCommit, SLSTransactionCreate, SLSTransactionOrderWindowGroup,
    SLSTransactionSetWindowAlpha, SLSTransactionSetWindowSystemAlpha,
    SLSTransactionSetWindowTransform,
};
use crate::globals::CV_HOST_CLOCK_FREQUENCY;
use crate::handles::WindowId;
use crate::scripting_addition::client::{
    scripting_addition_swap_window_proxy_in, scripting_addition_swap_window_proxy_out,
};
use crate::support::arithmetic::lerp;
use crate::support::easing::AnimationEasingType;
use crate::support::table::Table;
use crate::window::frame::window_manager_set_window_frame;
use crate::window::janky_borders::window_manager_notify_jankyborders;
use crate::window::manager::WindowManager;
use crate::window::proxy::{
    WindowProxy, WindowProxyCoreGraphicsObjects, load_window_proxy_frame, store_window_proxy_frame,
    window_manager_build_window_proxy_thread_proc, window_manager_create_window_proxy,
    window_manager_destroy_window_proxy,
};

pub(crate) struct WindowCapture {
    pub(crate) window_id: WindowId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

pub(crate) struct WindowAnimation {
    pub(crate) window_id: WindowId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) connection_id: i32,
    pub(crate) proxy: WindowProxy,
    pub(crate) skip: AtomicBool,
}

pub(crate) struct AnimationContext {
    pub(crate) animation_connection: i32,
    pub(crate) animation_easing: AnimationEasingType,
    pub(crate) animation_duration: f32,
    pub(crate) animation_clock: AtomicU64,
    pub(crate) animation_list: Vec<WindowAnimation>,
    pub(crate) animation_count: i32,
    pub(crate) window_animations_table: Arc<Mutex<Table<WindowId, (Arc<AnimationContext>, usize)>>>,
}

unsafe impl Send for AnimationContext {}
unsafe impl Sync for AnimationContext {}

pub(crate) unsafe extern "C-unwind" fn window_manager_animate_window_list_thread_proc(
    link: NonNull<CVDisplayLink>,
    now: NonNull<CVTimeStamp>,
    output_time: NonNull<CVTimeStamp>,
    _flags: CVOptionFlags,
    _flags_out: NonNull<CVOptionFlags>,
    data: *mut c_void,
) -> CVReturn {
    let animation_context =
        ManuallyDrop::new(unsafe { Arc::from_raw(data.cast::<AnimationContext>()) });
    let animation_count = animation_context.animation_count;

    let current_clock = unsafe { output_time.as_ref().hostTime };
    if animation_context.animation_clock.load(Ordering::Relaxed) == 0 {
        animation_context
            .animation_clock
            .store(unsafe { now.as_ref().hostTime }, Ordering::Relaxed);
    }

    let mut interpolant = (current_clock
        .wrapping_sub(animation_context.animation_clock.load(Ordering::Relaxed)))
        as f64
        / (animation_context.animation_duration as f64
            * *CV_HOST_CLOCK_FREQUENCY.get().unwrap_or(&0.0));
    if interpolant <= 0.0 {
        interpolant = 0.0f32 as f64;
    }
    if interpolant >= 1.0 {
        interpolant = 1.0f32 as f64;
    }

    let eased_interpolant = animation_context.animation_easing.apply(interpolant as f32);

    let transaction = unsafe { SLSTransactionCreate(animation_context.animation_connection) };
    for index in 0..animation_count as usize {
        let window_animation = &animation_context.animation_list[index];
        if window_animation.skip.load(Ordering::Relaxed) {
            continue;
        }

        let proxy_frame = load_window_proxy_frame(&window_animation.proxy);
        let target_x = lerp(proxy_frame.origin.x, eased_interpolant, window_animation.x) as f32;
        let target_y = lerp(proxy_frame.origin.y, eased_interpolant, window_animation.y) as f32;
        let target_width = lerp(
            proxy_frame.size.width,
            eased_interpolant,
            window_animation.width,
        ) as f32;
        let target_height = lerp(
            proxy_frame.size.height,
            eased_interpolant,
            window_animation.height,
        ) as f32;
        window_animation
            .proxy
            .target_x
            .store(target_x.to_bits(), Ordering::Relaxed);
        window_animation
            .proxy
            .target_y
            .store(target_y.to_bits(), Ordering::Relaxed);
        window_animation
            .proxy
            .target_width
            .store(target_width.to_bits(), Ordering::Relaxed);
        window_animation
            .proxy
            .target_height
            .store(target_height.to_bits(), Ordering::Relaxed);

        let transform = CGAffineTransformMakeTranslation((-target_x) as f64, (-target_y) as f64);
        let scale = CGAffineTransformMakeScale(
            proxy_frame.size.width / target_width as f64,
            proxy_frame.size.height / target_height as f64,
        );
        unsafe {
            SLSTransactionSetWindowTransform(
                transaction,
                window_animation.proxy.id.load(Ordering::Relaxed),
                0,
                0,
                CGAffineTransformConcat(transform, scale),
            )
        };

        let mut alpha = 0.0f32;
        unsafe {
            SLSGetWindowAlpha(
                animation_context.animation_connection,
                window_animation.window_id.0,
                &mut alpha,
            )
        };
        if alpha != 0.0f32 {
            unsafe {
                SLSTransactionSetWindowAlpha(
                    transaction,
                    window_animation.proxy.id.load(Ordering::Relaxed),
                    alpha,
                )
            };
        }
    }
    unsafe { SLSTransactionCommit(transaction, 0) };
    drop(unsafe { take_create_rule_result(transaction) });
    if interpolant != 1.0 {
        return kCVReturnSuccess;
    }

    {
        let mut window_animations_table = animation_context.window_animations_table.lock().unwrap();
        unsafe { SLSDisableUpdate(animation_context.animation_connection) };
        window_manager_notify_jankyborders(
            &animation_context.animation_list[..animation_context.animation_count as usize],
            1326,
            true,
            true,
        );
        scripting_addition_swap_window_proxy_out(
            &animation_context.animation_list[..animation_context.animation_count as usize],
        );
        for index in 0..animation_count as usize {
            if animation_context.animation_list[index]
                .skip
                .load(Ordering::Relaxed)
            {
                continue;
            }

            window_animations_table.remove(&animation_context.animation_list[index].window_id);
            window_manager_destroy_window_proxy(
                animation_context.animation_connection,
                &animation_context.animation_list[index].proxy,
            );
        }
        unsafe { SLSReenableUpdate(animation_context.animation_connection) };
    }

    unsafe { SLSReleaseConnection(animation_context.animation_connection) };
    drop(unsafe { Arc::from_raw(data.cast::<AnimationContext>()) });

    unsafe { CVDisplayLinkStop(link.as_ref()) };
    drop(unsafe { CFRetained::from_raw(link) });

    kCVReturnSuccess
}

pub(crate) fn window_manager_animate_window_list_async(
    window_list: &[WindowCapture],
    window_manager: &mut WindowManager,
) {
    let window_count = window_list.len();

    let mut animation_connection: i32 = 0;
    unsafe { SLSNewConnection(0, &mut animation_connection) };

    let mut animation_list: Vec<WindowAnimation> = Vec::with_capacity(window_count);
    for index in 0..window_count {
        animation_list.push(WindowAnimation {
            window_id: window_list[index].window_id,
            x: window_list[index].x,
            y: window_list[index].y,
            width: window_list[index].width,
            height: window_list[index].height,
            connection_id: animation_connection,
            skip: AtomicBool::new(false),
            proxy: WindowProxy {
                id: AtomicU32::new(0),
                core_graphics_objects: Mutex::new(WindowProxyCoreGraphicsObjects {
                    context: None,
                    image: None,
                }),
                target_x: AtomicU32::new(0),
                target_y: AtomicU32::new(0),
                target_width: AtomicU32::new(0),
                target_height: AtomicU32::new(0),
                frame_origin_x: AtomicU64::new(0),
                frame_origin_y: AtomicU64::new(0),
                frame_size_width: AtomicU64::new(0),
                frame_size_height: AtomicU64::new(0),
                level: AtomicI32::new(0),
                sub_level: AtomicI32::new(0),
            },
        });
    }

    let animation_context = Arc::new(AnimationContext {
        animation_connection,
        animation_count: window_count as i32,
        animation_list,
        animation_duration: window_manager.window_animation_duration,
        animation_easing: window_manager.window_animation_easing,
        animation_clock: AtomicU64::new(0),
        window_animations_table: Arc::clone(&window_manager.window_animations_table),
    });

    let mut builders_to_spawn: Vec<usize> = Vec::new();

    unsafe { SLSDisableUpdate(animation_context.animation_connection) };
    {
        let mut window_animations_table = animation_context.window_animations_table.lock().unwrap();
        for index in 0..window_count {
            let window_animation = &animation_context.animation_list[index];
            let window_id = window_animation.window_id;

            match window_animations_table.remove(&window_id) {
                Some(existing_animation_handle) => {
                    let existing_animation =
                        &existing_animation_handle.0.animation_list[existing_animation_handle.1];
                    existing_animation.skip.store(true, Ordering::Release);

                    let existing_target_x =
                        f32::from_bits(existing_animation.proxy.target_x.load(Ordering::Relaxed));
                    let existing_target_y =
                        f32::from_bits(existing_animation.proxy.target_y.load(Ordering::Relaxed));
                    let existing_target_width = f32::from_bits(
                        existing_animation
                            .proxy
                            .target_width
                            .load(Ordering::Relaxed),
                    );
                    let existing_target_height = f32::from_bits(
                        existing_animation
                            .proxy
                            .target_height
                            .load(Ordering::Relaxed),
                    );

                    store_window_proxy_frame(
                        &window_animation.proxy,
                        CGRect::new(
                            CGPoint::new(
                                existing_target_x as i32 as f64,
                                existing_target_y as i32 as f64,
                            ),
                            CGSize::new(
                                existing_target_width as i32 as f64,
                                existing_target_height as i32 as f64,
                            ),
                        ),
                    );
                    window_animation.proxy.target_x.store(
                        (existing_target_x as i32 as f32).to_bits(),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.target_y.store(
                        (existing_target_y as i32 as f32).to_bits(),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.target_width.store(
                        (existing_target_width as i32 as f32).to_bits(),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.target_height.store(
                        (existing_target_height as i32 as f32).to_bits(),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.level.store(
                        existing_animation.proxy.level.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.sub_level.store(
                        existing_animation.proxy.sub_level.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                    let existing_image = existing_animation
                        .proxy
                        .core_graphics_objects
                        .lock()
                        .unwrap()
                        .image
                        .clone();
                    window_animation
                        .proxy
                        .core_graphics_objects
                        .lock()
                        .unwrap()
                        .image = existing_image;
                    compiler_fence(Ordering::SeqCst);

                    let mut alpha = 1.0f32;
                    unsafe {
                        SLSGetWindowAlpha(
                            animation_context.animation_connection,
                            window_animation.window_id.0,
                            &mut alpha,
                        )
                    };
                    window_manager_create_window_proxy(
                        animation_context.animation_connection,
                        alpha,
                        &window_animation.proxy,
                    );
                    window_manager_notify_jankyborders(
                        &animation_context.animation_list[index..index + 1],
                        1325,
                        true,
                        false,
                    );
                    window_manager_notify_jankyborders(
                        &existing_animation_handle.0.animation_list
                            [existing_animation_handle.1..existing_animation_handle.1 + 1],
                        1326,
                        false,
                        false,
                    );

                    let transaction =
                        unsafe { SLSTransactionCreate(animation_context.animation_connection) };
                    unsafe {
                        SLSTransactionOrderWindowGroup(
                            transaction,
                            window_animation.proxy.id.load(Ordering::Relaxed),
                            1,
                            window_animation.window_id.0,
                        );
                        SLSTransactionSetWindowSystemAlpha(
                            transaction,
                            existing_animation.proxy.id.load(Ordering::Relaxed),
                            0.0,
                        );
                        SLSTransactionCommit(transaction, 0);
                    }
                    drop(unsafe { take_create_rule_result(transaction) });

                    window_manager_destroy_window_proxy(
                        existing_animation.connection_id,
                        &existing_animation.proxy,
                    );
                    drop(existing_animation_handle);
                }
                None => {
                    builders_to_spawn.push(index);
                }
            }

            window_animations_table.add(window_id, (Arc::clone(&animation_context), index));
        }
    }

    std::thread::scope(|scope| {
        for index in builders_to_spawn.iter().copied() {
            let animation_context_for_builder = Arc::clone(&animation_context);
            let builder_result = std::thread::Builder::new().spawn_scoped(scope, move || {
                window_manager_build_window_proxy_thread_proc(
                    &animation_context_for_builder.animation_list[index],
                );
            });
            if builder_result.is_err() {
                window_manager_build_window_proxy_thread_proc(
                    &animation_context.animation_list[index],
                );
            }
        }
    });

    scripting_addition_swap_window_proxy_in(
        &animation_context.animation_list[..animation_context.animation_count as usize],
    );

    window_manager_notify_jankyborders(
        &animation_context.animation_list[..animation_context.animation_count as usize],
        1325,
        true,
        false,
    );

    for index in 0..window_count {
        let window_animation = &animation_context.animation_list[index];
        let window_id = window_animation.window_id;
        let x = window_animation.x;
        let y = window_animation.y;
        let width = window_animation.width;
        let height = window_animation.height;
        window_manager_set_window_frame(window_id, x, y, width, height, window_manager);
    }

    let mut link: *mut CVDisplayLink = core::ptr::null_mut();
    unsafe { SLSReenableUpdate(animation_context.animation_connection) };
    unsafe { CVDisplayLinkCreateWithActiveCGDisplays(NonNull::from(&mut link)) };
    if let Some(link) = NonNull::new(link) {
        unsafe {
            CVDisplayLinkSetOutputCallback(
                link.as_ref(),
                Some(window_manager_animate_window_list_thread_proc),
                Arc::into_raw(animation_context).cast::<c_void>().cast_mut(),
            );
            CVDisplayLinkStart(link.as_ref());
        }
    }
}

pub(crate) fn window_manager_animate_window_list(
    window_list: &[WindowCapture],
    window_manager: &mut WindowManager,
) {
    if window_manager.window_animation_duration != 0.0f32 {
        window_manager_animate_window_list_async(window_list, window_manager);
    } else {
        for index in 0..window_list.len() {
            window_manager_set_window_frame(
                window_list[index].window_id,
                window_list[index].x,
                window_list[index].y,
                window_list[index].width,
                window_list[index].height,
                window_manager,
            );
        }
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
