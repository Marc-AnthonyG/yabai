#![allow(deprecated)]

use crate::ffi::core_foundation::{
    CFType, CGAffineTransform, CGRect, CGSize, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGAffineTransformConcat, CGAffineTransformMakeScale, CGAffineTransformMakeTranslation,
};
use crate::ffi::skylight::{
    SLSGetWindowAlpha, SLSTransactionCommit, SLSTransactionCreate, SLSTransactionSetWindowAlpha,
    SLSTransactionSetWindowTransform,
};
use crate::support::handles::WindowId;

const SMALLEST_DISPLAYED_EDGE_A_PROXY_IS_SCALED_TO: f64 = 1.0;

pub(crate) struct ProxyFrameUpdate {
    pub(crate) real_window_id: WindowId,
    pub(crate) proxy_window_id: u32,
    pub(crate) proxy_frame_size: CGSize,
    pub(crate) displayed_frame: CGRect,
    pub(crate) must_refresh_alpha: bool,
}

pub(crate) fn animation_commit_proxy_frames(
    animation_connection: i32,
    proxy_frame_update_list: &[ProxyFrameUpdate],
) {
    if proxy_frame_update_list.is_empty() {
        return;
    }

    let transaction = unsafe { SLSTransactionCreate(animation_connection) };
    for proxy_frame_update in proxy_frame_update_list {
        unsafe {
            SLSTransactionSetWindowTransform(
                transaction,
                proxy_frame_update.proxy_window_id,
                0,
                0,
                proxy_transform_showing_displayed_frame(
                    proxy_frame_update.proxy_frame_size,
                    proxy_frame_update.displayed_frame,
                ),
            )
        };

        if proxy_frame_update.must_refresh_alpha {
            copy_real_window_alpha_onto_its_proxy(
                animation_connection,
                transaction,
                proxy_frame_update,
            );
        }
    }
    unsafe { SLSTransactionCommit(transaction, 0) };
    drop(unsafe { take_create_rule_result(transaction) });
}

fn proxy_transform_showing_displayed_frame(
    proxy_frame_size: CGSize,
    displayed_frame: CGRect,
) -> CGAffineTransform {
    let displayed_width = displayed_frame
        .size
        .width
        .max(SMALLEST_DISPLAYED_EDGE_A_PROXY_IS_SCALED_TO);
    let displayed_height = displayed_frame
        .size
        .height
        .max(SMALLEST_DISPLAYED_EDGE_A_PROXY_IS_SCALED_TO);

    let transform =
        CGAffineTransformMakeTranslation(-displayed_frame.origin.x, -displayed_frame.origin.y);
    let scale = CGAffineTransformMakeScale(
        proxy_frame_size.width / displayed_width,
        proxy_frame_size.height / displayed_height,
    );
    CGAffineTransformConcat(transform, scale)
}

fn copy_real_window_alpha_onto_its_proxy(
    animation_connection: i32,
    transaction: *mut CFType,
    proxy_frame_update: &ProxyFrameUpdate,
) {
    let mut alpha = 0.0f32;
    unsafe {
        SLSGetWindowAlpha(
            animation_connection,
            proxy_frame_update.real_window_id.0,
            &mut alpha,
        )
    };
    if alpha != 0.0f32 {
        unsafe {
            SLSTransactionSetWindowAlpha(transaction, proxy_frame_update.proxy_window_id, alpha)
        };
    }
}
