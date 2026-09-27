#![allow(deprecated)]

use crate::ffi::core_foundation::{
    CFType, CGPoint, CGRect, CGSize, sls_window_disable_shadow, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGContext, CGContextAddPath, CGContextClearRect, CGContextClipToRect, CGContextFillRect,
    CGContextFlush, CGContextResetClip, CGContextSetLineWidth, CGContextSetRGBFillColor,
    CGContextSetRGBStrokeColor, CGContextStrokePath, CGPathCreateWithRoundedRect, CGRectGetMidX,
    CGRectGetMidY, CGRectInset, CGRegionCreateEmptyRegion, CGSNewRegionWithRect,
};
use crate::ffi::skylight::{
    SLSDisableUpdate, SLSNewWindowWithOpaqueShapeAndContext, SLSOrderWindow, SLSReenableUpdate,
    SLSReleaseWindow, SLSSetWindowLevel, SLSSetWindowOpacity, SLSSetWindowResolution,
    SLSSetWindowShape, SLSSetWindowSubLevel, SLWindowContextCreate,
};
use crate::notifications::window::update_window_notifications;
use crate::space::manager::SpaceManager;
use crate::state::process_wide::CONNECTION;
use crate::support::direction::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST, STACK};
use crate::support::geometry::{cgrect_clamp_x_radius, cgrect_clamp_y_radius};
use crate::support::handles::{NodeId, SpaceId, WindowId};
use crate::support::macos_version::{workspace_is_macos_sequoia, workspace_is_macos_tahoe};
use crate::window::manager::WindowManager;
use crate::window::model::{window_level, window_sub_level};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum WindowInsertionPoint {
    Focused = 0,
    First = 1,
    Last = 2,
}

pub(crate) static WINDOW_INSERTION_POINT_STR: [&str; 3] = ["focused", "first", "last"];

pub(crate) struct FeedbackWindow {
    pub(crate) id: WindowId,
    pub(crate) context: *mut CGContext,
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

pub(crate) const INSERT_FEEDBACK_WIDTH: f64 = 2.0;
pub(crate) const INSERT_FEEDBACK_RADIUS: f64 = 9.0;

pub(crate) fn insert_feedback_show(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let connection = *CONNECTION.get().unwrap();

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);

    let mut frame = CGRect {
        origin: CGPoint {
            x: node.area.x as f64,
            y: node.area.y as f64,
        },
        size: CGSize {
            width: node.area.width as f64,
            height: node.area.height as f64,
        },
    };
    let mut frame_region: *mut CFType = std::ptr::null_mut();
    unsafe { CGSNewRegionWithRect(&mut frame, &mut frame_region) };
    frame.origin.x = 0.0;
    frame.origin.y = 0.0;

    if FeedbackWindow::window_id_or_zero(&node.feedback_window) == 0 {
        let mut tags: u64 = (1u64 << 1) | (1u64 << 9);
        let empty_region = unsafe { CGRegionCreateEmptyRegion() };
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

        sls_window_disable_shadow(feedback_window_id);
        unsafe { SLSSetWindowResolution(connection, feedback_window_id, 1.0f32 as f64) };
        unsafe { SLSSetWindowOpacity(connection, feedback_window_id, false) };
        unsafe {
            SLSSetWindowLevel(
                connection,
                feedback_window_id,
                window_level(node.window_order[0]),
            )
        };
        unsafe {
            SLSSetWindowSubLevel(
                connection,
                feedback_window_id,
                window_sub_level(node.window_order[0]),
            )
        };
        let feedback_window_context =
            unsafe { SLWindowContextCreate(connection, feedback_window_id, std::ptr::null()) };
        node.feedback_window = Some(FeedbackWindow {
            id: WindowId(feedback_window_id),
            context: feedback_window_context,
        });
        let feedback_window_context = unsafe { feedback_window_context.as_ref() };
        CGContextSetLineWidth(feedback_window_context, INSERT_FEEDBACK_WIDTH);
        CGContextSetRGBFillColor(
            feedback_window_context,
            window_manager.insert_feedback_color.red as f64,
            window_manager.insert_feedback_color.green as f64,
            window_manager.insert_feedback_color.blue as f64,
            (window_manager.insert_feedback_color.alpha * 0.25f32) as f64,
        );
        CGContextSetRGBStrokeColor(
            feedback_window_context,
            window_manager.insert_feedback_color.red as f64,
            window_manager.insert_feedback_color.green as f64,
            window_manager.insert_feedback_color.blue as f64,
            window_manager.insert_feedback_color.alpha as f64,
        );
        unsafe { SLSDisableUpdate(connection) };
        CGContextClearRect(feedback_window_context, frame);
        CGContextFlush(feedback_window_context);
        unsafe { SLSReenableUpdate(connection) };
        unsafe { SLSOrderWindow(connection, feedback_window_id, 1, node.window_order[0].0) };
        window_manager
            .insert_feedback
            .add(node.window_order[0], (space_id, node_id));
        if !workspace_is_macos_sequoia() && !workspace_is_macos_tahoe() {
            update_window_notifications(window_manager, space_manager);
        }
    }

    let Some(node) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
    else {
        drop(unsafe { take_create_rule_result(frame_region.cast_const()) });
        return;
    };
    let (feedback_window_id, feedback_window_context) = match &node.feedback_window {
        Some(feedback_window) => (feedback_window.id, feedback_window.context),
        None => (WindowId(0), std::ptr::null_mut()),
    };
    let insert_direction = node.insert_direction;

    let clip_x: f64;
    let clip_y: f64;
    let clip_width: f64;
    let clip_height: f64;
    let middle_x = CGRectGetMidX(frame);
    let middle_y = CGRectGetMidY(frame);

    match insert_direction {
        DIR_NORTH => {
            clip_x = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = middle_y - 0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = INSERT_FEEDBACK_WIDTH;
            clip_height = INSERT_FEEDBACK_WIDTH;
        }
        DIR_EAST => {
            clip_x = middle_x - 0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = INSERT_FEEDBACK_WIDTH;
            clip_height = INSERT_FEEDBACK_WIDTH;
        }
        DIR_SOUTH => {
            clip_x = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = INSERT_FEEDBACK_WIDTH;
            clip_height = -middle_y + INSERT_FEEDBACK_WIDTH;
        }
        DIR_WEST => {
            clip_x = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = -middle_x + INSERT_FEEDBACK_WIDTH;
            clip_height = INSERT_FEEDBACK_WIDTH;
        }
        STACK => {
            clip_x = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = INSERT_FEEDBACK_WIDTH;
            clip_height = INSERT_FEEDBACK_WIDTH;
        }
        _ => unreachable!(),
    }

    let rect = CGRect {
        origin: CGPoint {
            x: 0.5 * INSERT_FEEDBACK_WIDTH,
            y: 0.5 * INSERT_FEEDBACK_WIDTH,
        },
        size: CGSize {
            width: frame.size.width - INSERT_FEEDBACK_WIDTH,
            height: frame.size.height - INSERT_FEEDBACK_WIDTH,
        },
    };
    let fill = CGRectInset(
        rect,
        0.5 * INSERT_FEEDBACK_WIDTH,
        0.5 * INSERT_FEEDBACK_WIDTH,
    );
    let clip = CGRect {
        origin: CGPoint {
            x: rect.origin.x + clip_x,
            y: rect.origin.y + clip_y,
        },
        size: CGSize {
            width: rect.size.width + clip_width,
            height: rect.size.height + clip_height,
        },
    };
    let path = unsafe {
        CGPathCreateWithRoundedRect(
            rect,
            cgrect_clamp_x_radius(rect, INSERT_FEEDBACK_RADIUS as f32) as f64,
            cgrect_clamp_y_radius(rect, INSERT_FEEDBACK_RADIUS as f32) as f64,
            std::ptr::null(),
        )
    };

    let feedback_window_context = unsafe { feedback_window_context.as_ref() };
    unsafe { SLSDisableUpdate(connection) };
    unsafe {
        SLSSetWindowShape(
            connection,
            feedback_window_id.0,
            0.0,
            0.0,
            frame_region.cast_const(),
        )
    };
    CGContextClearRect(feedback_window_context, frame);
    CGContextClipToRect(feedback_window_context, clip);
    CGContextFillRect(feedback_window_context, fill);
    CGContextAddPath(feedback_window_context, Some(&path));
    CGContextStrokePath(feedback_window_context);
    if let Some(feedback_window_context) = feedback_window_context {
        CGContextResetClip(feedback_window_context);
    }
    CGContextFlush(feedback_window_context);
    unsafe { SLSReenableUpdate(connection) };
    drop(path);
    drop(unsafe { take_create_rule_result(frame_region.cast_const()) });
}

pub(crate) fn insert_feedback_destroy(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let Some(node) = view.find_node_mut(node_id) else {
        return;
    };

    if FeedbackWindow::window_id_or_zero(&node.feedback_window) != 0 {
        window_manager.insert_feedback.remove(&node.window_order[0]);

        if !workspace_is_macos_sequoia() && !workspace_is_macos_tahoe() {
            update_window_notifications(window_manager, space_manager);
        }

        let Some(node) = space_manager
            .view
            .find_mut(&space_id)
            .and_then(|view| view.find_node_mut(node_id))
        else {
            return;
        };
        drop(node.feedback_window.take());
    }
}
