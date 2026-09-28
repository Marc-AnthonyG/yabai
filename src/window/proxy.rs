#![allow(deprecated)]

use core::ptr::NonNull;

use crate::ffi::core_foundation::{
    CFRetained, CFRetainedAssumedSendAndSync, CFType, CGPoint, CGRect,
    cfarray_borrow_value_at_index, disable_window_shadow_through_skylight, take_create_rule_result,
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
use crate::support::handles::WindowId;
use crate::support::image::copy_image_undoing_premultiplied_alpha_and_making_it_opaque;
use crate::window::model::{
    query_window_level_from_window_server, query_window_sub_level_from_window_server,
};
use crate::window::proxy_pairing::WindowProxyPairing;

pub(crate) struct WindowProxy {
    pub(crate) real_window_id: WindowId,
    pub(crate) id: u32,
    pub(crate) frame: CGRect,
    pub(crate) level: i32,
    pub(crate) sub_level: i32,
    pub(crate) context: Option<CFRetainedAssumedSendAndSync<CGContext>>,
    pub(crate) image: Option<CFRetained<CGImage>>,
}

impl WindowProxy {
    pub(crate) fn pairing(&self) -> WindowProxyPairing {
        WindowProxyPairing {
            real_window_id: self.real_window_id,
            proxy_window_id: self.id,
        }
    }
}

pub(crate) fn create_window_proxy_showing_its_captured_image(
    animation_connection: i32,
    alpha: f32,
    proxy: &mut WindowProxy,
) {
    let Some(image) = proxy.image.as_deref() else {
        return;
    };

    let mut frame_region: *mut CFType = core::ptr::null_mut();
    unsafe { CGSNewRegionWithRect(&mut proxy.frame, &mut frame_region) };
    let empty_region = unsafe { CGRegionCreateEmptyRegion() };

    let mut tags: u64 = 1u64 << 46;
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
            &mut proxy.id,
            core::ptr::null_mut(),
        )
    };
    disable_window_shadow_through_skylight(proxy.id);
    unsafe {
        SLSSetWindowOpacity(animation_connection, proxy.id, false);
        SLSSetWindowResolution(animation_connection, proxy.id, 2.0f32 as f64);
        SLSSetWindowAlpha(animation_connection, proxy.id, alpha);
        SLSSetWindowLevel(animation_connection, proxy.id, proxy.level);
        SLSSetWindowSubLevel(animation_connection, proxy.id, proxy.sub_level);
    }
    let context = unsafe {
        take_create_rule_result(SLWindowContextCreate(
            animation_connection,
            proxy.id,
            core::ptr::null(),
        ))
    };

    let frame = CGRect::new(CGPoint::new(0.0, 0.0), proxy.frame.size);
    CGContextClearRect(context.as_deref(), frame);
    CGContextDrawImage(context.as_deref(), frame, Some(image));
    CGContextFlush(context.as_deref());
    proxy.context = context.map(CFRetainedAssumedSendAndSync);
    drop(unsafe { take_create_rule_result(frame_region) });
    drop(unsafe { take_create_rule_result(empty_region) });
}

pub(crate) fn destroy_window_proxy(animation_connection: i32, proxy: WindowProxy) {
    let mut proxy = proxy;

    if let Some(image) = proxy.image.take() {
        drop(image);
    }

    if let Some(context) = proxy.context.take() {
        drop(context);
    }

    if proxy.id != 0 {
        unsafe { SLSReleaseWindow(animation_connection, proxy.id) };
        proxy.id = 0;
    }
}

pub(crate) fn build_window_proxy_from_a_capture_of_the_window(
    animation_connection: i32,
    window_id: WindowId,
) -> Option<WindowProxy> {
    let mut alpha = 1.0f32;
    unsafe { SLSGetWindowAlpha(animation_connection, window_id.0, &mut alpha) };
    let mut proxy = WindowProxy {
        real_window_id: window_id,
        id: 0,
        frame: CGRect::default(),
        level: query_window_level_from_window_server(window_id),
        sub_level: query_window_sub_level_from_window_server(window_id),
        context: None,
        image: None,
    };
    unsafe { SLSGetWindowBounds(animation_connection, window_id.0, &mut proxy.frame) };

    let mut capture_window_id = window_id.0;
    let image_array = unsafe {
        take_create_rule_result(SLSHWCaptureWindowList(
            animation_connection,
            &mut capture_window_id,
            1,
            (1 << 11) | (1 << 8),
        ))
    };
    if let Some(image_array) = image_array {
        proxy.image = match unsafe { cfarray_borrow_value_at_index::<CGImage>(&image_array, 0) } {
            Some(image) => {
                if alpha == 1.0f32 {
                    Some(unsafe { CFRetained::retain(NonNull::from(image)) })
                } else {
                    copy_image_undoing_premultiplied_alpha_and_making_it_opaque(image)
                }
            }
            None => None,
        };
        drop(image_array);
    } else {
        proxy.image = None;
    }

    create_window_proxy_showing_its_captured_image(animation_connection, alpha, &mut proxy);
    if proxy.id == 0 {
        destroy_window_proxy(animation_connection, proxy);
        return None;
    }
    Some(proxy)
}
