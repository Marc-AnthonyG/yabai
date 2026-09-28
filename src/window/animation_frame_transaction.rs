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

pub(crate) fn commit_proxy_frames_in_one_window_server_transaction(
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

#[cfg(test)]
mod tests {
    use super::proxy_transform_showing_displayed_frame;
    use crate::ffi::core_foundation::{CGAffineTransform, CGPoint, CGRect, CGSize};

    fn frame(x: f64, y: f64, width: f64, height: f64) -> CGRect {
        CGRect::new(CGPoint::new(x, y), CGSize::new(width, height))
    }

    fn components(transform: CGAffineTransform) -> [f64; 6] {
        [
            transform.a,
            transform.b,
            transform.c,
            transform.d,
            transform.tx,
            transform.ty,
        ]
    }

    fn apply(transform: CGAffineTransform, x: f64, y: f64) -> (f64, f64) {
        (
            transform.a * x + transform.c * y + transform.tx,
            transform.b * x + transform.d * y + transform.ty,
        )
    }

    #[test]
    fn the_transform_is_the_one_the_c_built_from_the_proxy_size_and_the_displayed_frame() {
        let expected_components = [
            (
                CGSize::new(800.0, 600.0),
                frame(100.0, 200.0, 800.0, 600.0),
                [1.0, 0.0, 0.0, 1.0, -100.0, -200.0],
            ),
            (
                CGSize::new(800.0, 600.0),
                frame(258.25, 177.5, 868.0, 660.0),
                [
                    0.9216589861751152,
                    0.0,
                    0.0,
                    0.9090909090909091,
                    -238.0184331797235,
                    -161.36363636363635,
                ],
            ),
            (
                CGSize::new(1300.0, 1041.0),
                frame(-1085.0, 38.0, 650.0, 520.5),
                [2.0, 0.0, 0.0, 2.0, 2170.0, -76.0],
            ),
        ];

        for (proxy_frame_size, displayed_frame, expected) in expected_components {
            assert_eq!(
                components(proxy_transform_showing_displayed_frame(
                    proxy_frame_size,
                    displayed_frame
                )),
                expected
            );
        }
    }

    #[test]
    fn the_transform_maps_the_displayed_frame_onto_the_whole_proxy() {
        for (proxy_frame_size, displayed_frame) in [
            (
                CGSize::new(800.0, 600.0),
                frame(258.25, 177.5, 868.0, 660.0),
            ),
            (
                CGSize::new(1300.0, 1041.0),
                frame(-1085.0, 38.0, 650.0, 520.5),
            ),
            (CGSize::new(640.0, 480.0), frame(3.0, -2.0, 1.5, 1.0)),
        ] {
            let transform =
                proxy_transform_showing_displayed_frame(proxy_frame_size, displayed_frame);

            let (origin_x, origin_y) = apply(
                transform,
                displayed_frame.origin.x,
                displayed_frame.origin.y,
            );
            let (far_corner_x, far_corner_y) = apply(
                transform,
                displayed_frame.origin.x + displayed_frame.size.width,
                displayed_frame.origin.y + displayed_frame.size.height,
            );

            assert!(origin_x.abs() <= 1e-9 && origin_y.abs() <= 1e-9);
            assert!((far_corner_x - proxy_frame_size.width).abs() <= 1e-9);
            assert!((far_corner_y - proxy_frame_size.height).abs() <= 1e-9);
        }
    }

    #[test]
    fn the_transform_never_scales_against_a_displayed_edge_shorter_than_one_point() {
        for (displayed_width, displayed_height) in [
            (0.25, 1.0),
            (1.0, 0.0),
            (0.0, 0.0),
            (-3.0, 0.5),
            (1.0, -0.0),
        ] {
            let transform = proxy_transform_showing_displayed_frame(
                CGSize::new(640.0, 480.0),
                frame(10.0, 20.0, displayed_width, displayed_height),
            );

            assert_eq!(
                [transform.a, transform.d],
                [640.0, 480.0],
                "displayed size {displayed_width} x {displayed_height}"
            );
            assert_eq!([transform.tx, transform.ty], [-6400.0, -9600.0]);
        }
    }
}
