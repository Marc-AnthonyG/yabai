#![allow(deprecated)]

use core::ptr::NonNull;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};

use crate::ffi::core_foundation::{
    CFRetained, CFType, CGPoint, CGRect, CGSize, cfarray_borrow_value_at_index,
    sls_window_disable_shadow, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGContext, CGContextClearRect, CGContextDrawImage, CGContextFlush, CGImage,
    CGRegionCreateEmptyRegion, CGSNewRegionWithRect,
};
use crate::ffi::skylight::{
    SLSGetWindowAlpha, SLSGetWindowBounds, SLSHWCaptureWindowList,
    SLSNewWindowWithOpaqueShapeAndContext, SLSReleaseWindow, SLSSetWindowAlpha, SLSSetWindowLevel,
    SLSSetWindowOpacity, SLSSetWindowResolution, SLSSetWindowSubLevel, SLWindowContextCreate,
};
use crate::support::image::cgimage_restore_alpha;
use crate::window::animation::WindowAnimation;
use crate::window::model::{window_level, window_sub_level};

pub(crate) struct WindowProxy {
    pub(crate) id: AtomicU32,
    pub(crate) core_graphics_objects: Mutex<WindowProxyCoreGraphicsObjects>,
    pub(crate) target_x: AtomicU32,
    pub(crate) target_y: AtomicU32,
    pub(crate) target_width: AtomicU32,
    pub(crate) target_height: AtomicU32,
    pub(crate) frame_origin_x: AtomicU64,
    pub(crate) frame_origin_y: AtomicU64,
    pub(crate) frame_size_width: AtomicU64,
    pub(crate) frame_size_height: AtomicU64,
    pub(crate) level: AtomicI32,
    pub(crate) sub_level: AtomicI32,
}

pub(crate) struct WindowProxyCoreGraphicsObjects {
    pub(crate) context: Option<CFRetained<CGContext>>,
    pub(crate) image: Option<CFRetained<CGImage>>,
}

pub(crate) fn load_window_proxy_frame(proxy: &WindowProxy) -> CGRect {
    CGRect::new(
        CGPoint::new(
            f64::from_bits(proxy.frame_origin_x.load(Ordering::Relaxed)),
            f64::from_bits(proxy.frame_origin_y.load(Ordering::Relaxed)),
        ),
        CGSize::new(
            f64::from_bits(proxy.frame_size_width.load(Ordering::Relaxed)),
            f64::from_bits(proxy.frame_size_height.load(Ordering::Relaxed)),
        ),
    )
}

pub(crate) fn store_window_proxy_frame(proxy: &WindowProxy, frame: CGRect) {
    proxy
        .frame_origin_x
        .store(frame.origin.x.to_bits(), Ordering::Relaxed);
    proxy
        .frame_origin_y
        .store(frame.origin.y.to_bits(), Ordering::Relaxed);
    proxy
        .frame_size_width
        .store(frame.size.width.to_bits(), Ordering::Relaxed);
    proxy
        .frame_size_height
        .store(frame.size.height.to_bits(), Ordering::Relaxed);
}

pub(crate) fn window_manager_create_window_proxy(
    animation_connection: i32,
    alpha: f32,
    proxy: &WindowProxy,
) {
    let mut core_graphics_objects_guard = proxy.core_graphics_objects.lock().unwrap();
    let core_graphics_objects = &mut *core_graphics_objects_guard;
    let Some(image) = core_graphics_objects.image.as_deref() else {
        return;
    };

    let mut proxy_frame = load_window_proxy_frame(proxy);
    let mut frame_region: *mut CFType = core::ptr::null_mut();
    unsafe { CGSNewRegionWithRect(&mut proxy_frame, &mut frame_region) };
    let empty_region = unsafe { CGRegionCreateEmptyRegion() };

    let mut tags: u64 = 1u64 << 46;
    let mut proxy_window_id = proxy.id.load(Ordering::Relaxed);
    unsafe {
        SLSNewWindowWithOpaqueShapeAndContext(
            animation_connection,
            2,
            frame_region,
            empty_region,
            13 | (1 << 18),
            &mut tags,
            0.0,
            0.0,
            64,
            &mut proxy_window_id,
            core::ptr::null_mut(),
        )
    };
    proxy.id.store(proxy_window_id, Ordering::Relaxed);
    sls_window_disable_shadow(proxy_window_id);
    unsafe {
        SLSSetWindowOpacity(animation_connection, proxy_window_id, false);
        SLSSetWindowResolution(animation_connection, proxy_window_id, 2.0f32 as f64);
        SLSSetWindowAlpha(animation_connection, proxy_window_id, alpha);
        SLSSetWindowLevel(
            animation_connection,
            proxy_window_id,
            proxy.level.load(Ordering::Relaxed),
        );
        SLSSetWindowSubLevel(
            animation_connection,
            proxy_window_id,
            proxy.sub_level.load(Ordering::Relaxed),
        );
    }
    core_graphics_objects.context = unsafe {
        take_create_rule_result(SLWindowContextCreate(
            animation_connection,
            proxy_window_id,
            core::ptr::null(),
        ))
    };

    let frame = CGRect::new(CGPoint::new(0.0, 0.0), proxy_frame.size);
    CGContextClearRect(core_graphics_objects.context.as_deref(), frame);
    CGContextDrawImage(core_graphics_objects.context.as_deref(), frame, Some(image));
    CGContextFlush(core_graphics_objects.context.as_deref());
    drop(unsafe { take_create_rule_result(frame_region) });
    drop(unsafe { take_create_rule_result(empty_region) });
}

pub(crate) fn window_manager_destroy_window_proxy(animation_connection: i32, proxy: &WindowProxy) {
    let mut core_graphics_objects = proxy.core_graphics_objects.lock().unwrap();

    if let Some(image) = core_graphics_objects.image.take() {
        drop(image);
    }

    if let Some(context) = core_graphics_objects.context.take() {
        drop(context);
    }

    drop(core_graphics_objects);

    let proxy_window_id = proxy.id.load(Ordering::Relaxed);
    if proxy_window_id != 0 {
        unsafe { SLSReleaseWindow(animation_connection, proxy_window_id) };
        proxy.id.store(0, Ordering::Relaxed);
    }
}

pub(crate) fn window_manager_build_window_proxy_thread_proc(window_animation: &WindowAnimation) {
    let mut alpha = 1.0f32;
    unsafe {
        SLSGetWindowAlpha(
            window_animation.connection_id,
            window_animation.window_id.0,
            &mut alpha,
        )
    };
    window_animation
        .proxy
        .level
        .store(window_level(window_animation.window_id), Ordering::Relaxed);
    window_animation.proxy.sub_level.store(
        window_sub_level(window_animation.window_id),
        Ordering::Relaxed,
    );
    let mut proxy_frame = load_window_proxy_frame(&window_animation.proxy);
    unsafe {
        SLSGetWindowBounds(
            window_animation.connection_id,
            window_animation.window_id.0,
            &mut proxy_frame,
        )
    };
    store_window_proxy_frame(&window_animation.proxy, proxy_frame);
    window_animation
        .proxy
        .target_x
        .store((proxy_frame.origin.x as f32).to_bits(), Ordering::Relaxed);
    window_animation
        .proxy
        .target_y
        .store((proxy_frame.origin.y as f32).to_bits(), Ordering::Relaxed);
    window_animation
        .proxy
        .target_width
        .store((proxy_frame.size.width as f32).to_bits(), Ordering::Relaxed);
    window_animation.proxy.target_height.store(
        (proxy_frame.size.height as f32).to_bits(),
        Ordering::Relaxed,
    );

    let mut window_id = window_animation.window_id.0;
    let image_array = unsafe {
        take_create_rule_result(SLSHWCaptureWindowList(
            window_animation.connection_id,
            &mut window_id,
            1,
            (1 << 11) | (1 << 8),
        ))
    };
    if let Some(image_array) = image_array {
        let image = match unsafe { cfarray_borrow_value_at_index::<CGImage>(&image_array, 0) } {
            Some(image) => {
                if alpha == 1.0f32 {
                    Some(unsafe { CFRetained::retain(NonNull::from(image)) })
                } else {
                    cgimage_restore_alpha(image)
                }
            }
            None => None,
        };
        window_animation
            .proxy
            .core_graphics_objects
            .lock()
            .unwrap()
            .image = image;
        drop(image_array);
    } else {
        window_animation
            .proxy
            .core_graphics_objects
            .lock()
            .unwrap()
            .image = None;
    }

    window_manager_create_window_proxy(
        window_animation.connection_id,
        alpha,
        &window_animation.proxy,
    );
}
