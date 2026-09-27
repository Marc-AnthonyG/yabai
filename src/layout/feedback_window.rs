#![allow(deprecated)]

use std::time::{Duration, Instant};

use crate::event::queue::{Event, event_loop_post};
use crate::ffi::core_foundation::{
    CFType, CGPoint, CGRect, CGSize, sls_window_disable_shadow, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGContext, CGContextAddPath, CGContextClearRect, CGContextDrawPath, CGContextFlush,
    CGContextSetLineWidth, CGContextSetRGBFillColor, CGContextSetRGBStrokeColor,
    CGPathCreateWithRoundedRect, CGPathDrawingMode, CGRegionCreateEmptyRegion,
    CGSNewRegionWithRect,
};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::ffi::skylight::{
    SLSDisableUpdate, SLSNewWindowWithOpaqueShapeAndContext, SLSOrderWindow, SLSReenableUpdate,
    SLSReleaseWindow, SLSSetWindowAlpha, SLSSetWindowLevel, SLSSetWindowOpacity,
    SLSSetWindowResolution, SLSSetWindowShape, SLSSetWindowSubLevel, SLWindowContextCreate,
};
use crate::state::process_wide::CONNECTION;
use crate::support::color::RgbaColor;
use crate::support::handles::WindowId;
use crate::support::macos_version::workspace_is_macos_tahoe;
use crate::window::model::{window_level, window_sub_level};

pub(crate) struct FeedbackWindow {
    pub(crate) id: WindowId,
    pub(crate) context: *mut CGContext,
    pub(crate) fade_in_started_at: Option<Instant>,
}

impl Drop for FeedbackWindow {
    fn drop(&mut self) {
        let connection = *CONNECTION.get().unwrap();
        if self.id.0 != 0 {
            unsafe { SLSOrderWindow(connection, self.id.0, 0, 0) };
            drop(unsafe { take_create_rule_result(self.context.cast_const()) });
            unsafe { SLSReleaseWindow(connection, self.id.0) };
        } else {
            drop(unsafe { take_create_rule_result(self.context.cast_const()) });
        }
    }
}

impl FeedbackWindow {
    pub(crate) fn window_id_or_zero(feedback_window: &Option<FeedbackWindow>) -> u32 {
        feedback_window
            .as_ref()
            .map_or(0, |feedback_window| feedback_window.id.0)
    }
}

pub(crate) const INSERT_FEEDBACK_BORDER_WIDTH: f64 = 2.0;
pub(crate) const INSERT_FEEDBACK_FILL_ALPHA_AS_A_FRACTION_OF_THE_BORDER_ALPHA: f32 = 0.18;
pub(crate) const MACOS_WINDOW_CORNER_RADIUS_BEFORE_TAHOE: f64 = 10.0;
pub(crate) const MACOS_WINDOW_CORNER_RADIUS_ON_TAHOE: f64 = 16.0;
pub(crate) const FEEDBACK_WINDOW_RESOLUTION_SHARP_ON_RETINA_DISPLAYS: f64 = 2.0;
pub(crate) const FEEDBACK_WINDOW_FADE_IN_DURATION: Duration = Duration::from_millis(120);
pub(crate) const DELAY_BETWEEN_FEEDBACK_WINDOW_FADE_IN_STEPS_IN_NANOSECONDS: i64 = 16_666_667;

pub(crate) fn macos_window_corner_radius() -> f64 {
    if workspace_is_macos_tahoe() {
        MACOS_WINDOW_CORNER_RADIUS_ON_TAHOE
    } else {
        MACOS_WINDOW_CORNER_RADIUS_BEFORE_TAHOE
    }
}

pub(crate) fn feedback_window_create_transparent_above_window(
    frame: CGRect,
    window_id: WindowId,
) -> FeedbackWindow {
    let connection = *CONNECTION.get().unwrap();

    let mut frame = frame;
    let mut frame_region: *mut CFType = std::ptr::null_mut();
    unsafe { CGSNewRegionWithRect(&mut frame, &mut frame_region) };
    let empty_region = unsafe { CGRegionCreateEmptyRegion() };

    let mut tags: u64 = (1u64 << 1) | (1u64 << 9);
    let mut feedback_window_id: u32 = 0;
    unsafe {
        SLSNewWindowWithOpaqueShapeAndContext(
            connection,
            2,
            frame_region.cast_const(),
            empty_region.cast_const(),
            13,
            &mut tags,
            0.0,
            0.0,
            64,
            &mut feedback_window_id,
            std::ptr::null_mut(),
        )
    };
    drop(unsafe { take_create_rule_result(empty_region.cast_const()) });
    drop(unsafe { take_create_rule_result(frame_region.cast_const()) });

    sls_window_disable_shadow(feedback_window_id);
    unsafe {
        SLSSetWindowResolution(
            connection,
            feedback_window_id,
            FEEDBACK_WINDOW_RESOLUTION_SHARP_ON_RETINA_DISPLAYS,
        )
    };
    unsafe { SLSSetWindowOpacity(connection, feedback_window_id, false) };
    unsafe { SLSSetWindowAlpha(connection, feedback_window_id, 0.0f32) };
    unsafe { SLSSetWindowLevel(connection, feedback_window_id, window_level(window_id)) };
    unsafe { SLSSetWindowSubLevel(connection, feedback_window_id, window_sub_level(window_id)) };
    let feedback_window_context =
        unsafe { SLWindowContextCreate(connection, feedback_window_id, std::ptr::null()) };

    let window_bounds = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: frame.size,
    };
    unsafe { SLSDisableUpdate(connection) };
    CGContextClearRect(unsafe { feedback_window_context.as_ref() }, window_bounds);
    CGContextFlush(unsafe { feedback_window_context.as_ref() });
    unsafe { SLSReenableUpdate(connection) };
    unsafe { SLSOrderWindow(connection, feedback_window_id, 1, window_id.0) };

    FeedbackWindow {
        id: WindowId(feedback_window_id),
        context: feedback_window_context,
        fade_in_started_at: Some(Instant::now()),
    }
}

pub(crate) fn feedback_window_draw_ghost_of_frame(
    feedback_window: &FeedbackWindow,
    frame: CGRect,
    color: RgbaColor,
) {
    let connection = *CONNECTION.get().unwrap();

    let mut frame = frame;
    let mut frame_region: *mut CFType = std::ptr::null_mut();
    unsafe { CGSNewRegionWithRect(&mut frame, &mut frame_region) };

    let window_bounds = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: frame.size,
    };
    let border_centre_line = CGRect {
        origin: CGPoint {
            x: 0.5 * INSERT_FEEDBACK_BORDER_WIDTH,
            y: 0.5 * INSERT_FEEDBACK_BORDER_WIDTH,
        },
        size: CGSize {
            width: (frame.size.width - INSERT_FEEDBACK_BORDER_WIDTH).max(0.0),
            height: (frame.size.height - INSERT_FEEDBACK_BORDER_WIDTH).max(0.0),
        },
    };
    let corner_radius_of_the_border_centre_line_that_fits_inside_it =
        (macos_window_corner_radius() - 0.5 * INSERT_FEEDBACK_BORDER_WIDTH)
            .min(0.5 * border_centre_line.size.width)
            .min(0.5 * border_centre_line.size.height)
            .max(0.0);
    let border_path = unsafe {
        CGPathCreateWithRoundedRect(
            border_centre_line,
            corner_radius_of_the_border_centre_line_that_fits_inside_it,
            corner_radius_of_the_border_centre_line_that_fits_inside_it,
            std::ptr::null(),
        )
    };

    let feedback_window_context = unsafe { feedback_window.context.as_ref() };
    unsafe { SLSDisableUpdate(connection) };
    unsafe {
        SLSSetWindowShape(
            connection,
            feedback_window.id.0,
            0.0,
            0.0,
            frame_region.cast_const(),
        )
    };
    CGContextClearRect(feedback_window_context, window_bounds);
    CGContextSetLineWidth(feedback_window_context, INSERT_FEEDBACK_BORDER_WIDTH);
    CGContextSetRGBFillColor(
        feedback_window_context,
        color.red as f64,
        color.green as f64,
        color.blue as f64,
        (color.alpha * INSERT_FEEDBACK_FILL_ALPHA_AS_A_FRACTION_OF_THE_BORDER_ALPHA) as f64,
    );
    CGContextSetRGBStrokeColor(
        feedback_window_context,
        color.red as f64,
        color.green as f64,
        color.blue as f64,
        color.alpha as f64,
    );
    CGContextAddPath(feedback_window_context, Some(&border_path));
    CGContextDrawPath(feedback_window_context, CGPathDrawingMode::FillStroke);
    CGContextFlush(feedback_window_context);
    unsafe { SLSReenableUpdate(connection) };
    drop(border_path);
    drop(unsafe { take_create_rule_result(frame_region.cast_const()) });
}

pub(crate) fn feedback_window_is_fading_in(feedback_window: &FeedbackWindow) -> bool {
    feedback_window.fade_in_started_at.is_some()
}

pub(crate) fn feedback_window_advance_fade_in(feedback_window: &mut FeedbackWindow) {
    let Some(fade_in_started_at) = feedback_window.fade_in_started_at else {
        return;
    };

    let fraction_of_the_fade_in_elapsed = (fade_in_started_at.elapsed().as_secs_f32()
        / FEEDBACK_WINDOW_FADE_IN_DURATION.as_secs_f32())
    .min(1.0f32);
    unsafe {
        SLSSetWindowAlpha(
            *CONNECTION.get().unwrap(),
            feedback_window.id.0,
            fraction_of_the_fade_in_elapsed,
        )
    };

    if fraction_of_the_fade_in_elapsed >= 1.0f32 {
        feedback_window.fade_in_started_at = None;
    }
}

pub(crate) fn schedule_the_next_feedback_window_fade_in_step() {
    dispatch_after_on_main_queue(
        DELAY_BETWEEN_FEEDBACK_WINDOW_FADE_IN_STEPS_IN_NANOSECONDS,
        || event_loop_post(Event::InsertFeedbackFadeInStep),
    );
}
