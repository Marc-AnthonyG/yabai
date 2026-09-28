#![allow(deprecated)]

use crate::ffi::core_foundation::{
    CFType, CGPoint, CGRect, CGSize, create_cfarray_of_window_ids,
    disable_window_shadow_through_skylight, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGAffineTransformIdentity, CGContext, CGContextAddPath, CGContextClearRect, CGContextDrawPath,
    CGContextFlush, CGContextSetRGBFillColor, CGContextSetTextMatrix, CGContextSetTextPosition,
    CGPathCreateWithRoundedRect, CGPathDrawingMode, CGRegionCreateEmptyRegion,
    CGSNewRegionWithRect,
};
use crate::ffi::core_text::{
    CTFont, CTLineTruncationType, create_font_of_family_and_style,
    create_line_of_text_coloured_by_the_context_fill, typographic_bounds_of_line,
};
use crate::ffi::skylight::{
    SLSDisableUpdate, SLSMoveWindowsToManagedSpace, SLSNewWindowWithOpaqueShapeAndContext,
    SLSOrderWindow, SLSReenableUpdate, SLSReleaseWindow, SLSSetWindowAlpha, SLSSetWindowLevel,
    SLSSetWindowOpacity, SLSSetWindowResolution, SLSSetWindowShape, SLSSetWindowSubLevel,
    SLWindowContextCreate,
};
use crate::layout::group_header_style::GroupHeaderStyle;
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::color::RgbaColor;
use crate::support::handles::{SpaceId, WindowId};
use crate::window::model::{
    query_window_level_from_window_server, query_window_sub_level_from_window_server,
};

const GROUP_HEADER_WINDOW_RESOLUTION_SHARP_ON_RETINA_DISPLAYS: f64 = 2.0;
const SPACE_BETWEEN_TABS: f64 = 4.0;
const SPACE_BETWEEN_THE_HEADER_AND_THE_WINDOWS_BELOW: f64 = 4.0;
const SPACE_BETWEEN_THE_BACKGROUND_AND_THE_TABS_INSIDE_IT: f64 = 3.0;
const TEXT_INSET_FROM_THE_EDGES_OF_A_TAB: f64 = 8.0;
const LARGEST_BACKGROUND_CORNER_RADIUS: f64 = 16.0;
const LARGEST_TAB_CORNER_RADIUS: f64 = 8.0;
const TEXT_THAT_ENDS_A_TRUNCATED_TITLE: &str = "\u{2026}";

pub(crate) struct GroupHeaderTab {
    pub(crate) window_id: WindowId,
    pub(crate) title: String,
    pub(crate) is_the_front_window: bool,
}

pub(crate) struct GroupHeaderWindow {
    pub(crate) id: WindowId,
    context: *mut CGContext,
    pub(crate) frame: CGRect,
    pub(crate) tab_frames: Vec<(WindowId, CGRect)>,
}

impl Drop for GroupHeaderWindow {
    fn drop(&mut self) {
        let connection = *SKYLIGHT_CONNECTION_ID.get().unwrap();
        unsafe { SLSOrderWindow(connection, self.id.0, 0, 0) };
        drop(unsafe { take_create_rule_result(self.context.cast_const()) });
        unsafe { SLSReleaseWindow(connection, self.id.0) };
    }
}

pub(crate) fn create_group_header_window_on_space(
    frame: CGRect,
    space_id: SpaceId,
) -> GroupHeaderWindow {
    let connection = *SKYLIGHT_CONNECTION_ID.get().unwrap();

    let mut frame_for_the_region = frame;
    let mut frame_region: *mut CFType = std::ptr::null_mut();
    unsafe { CGSNewRegionWithRect(&mut frame_for_the_region, &mut frame_region) };
    let empty_region = unsafe { CGRegionCreateEmptyRegion() };

    let mut tags: u64 = (1u64 << 1) | (1u64 << 9);
    let mut header_window_id: u32 = 0;
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
            &mut header_window_id,
            std::ptr::null_mut(),
        )
    };
    drop(unsafe { take_create_rule_result(empty_region.cast_const()) });
    drop(unsafe { take_create_rule_result(frame_region.cast_const()) });

    disable_window_shadow_through_skylight(header_window_id);
    unsafe {
        SLSSetWindowResolution(
            connection,
            header_window_id,
            GROUP_HEADER_WINDOW_RESOLUTION_SHARP_ON_RETINA_DISPLAYS,
        )
    };
    unsafe { SLSSetWindowOpacity(connection, header_window_id, false) };
    unsafe { SLSSetWindowAlpha(connection, header_window_id, 1.0f32) };

    let window_list = create_cfarray_of_window_ids(&[header_window_id]);
    unsafe { SLSMoveWindowsToManagedSpace(connection, &*window_list, space_id.0) };

    let context = unsafe { SLWindowContextCreate(connection, header_window_id, std::ptr::null()) };

    GroupHeaderWindow {
        id: WindowId(header_window_id),
        context,
        frame,
        tab_frames: Vec::new(),
    }
}

pub(crate) fn draw_tabs_in_group_header_window(
    header_window: &mut GroupHeaderWindow,
    frame: CGRect,
    tabs: &[GroupHeaderTab],
    style: &GroupHeaderStyle,
) {
    let connection = *SKYLIGHT_CONNECTION_ID.get().unwrap();

    let mut frame_for_the_region = frame;
    let mut frame_region: *mut CFType = std::ptr::null_mut();
    unsafe { CGSNewRegionWithRect(&mut frame_for_the_region, &mut frame_region) };

    let context = unsafe { header_window.context.as_ref() };
    let font = create_font_of_family_and_style(
        &style.font_family,
        &style.font_style,
        style.font_size as f64,
    );
    let background_frame = frame_of_the_background_inside_a_header(frame.size);
    let tab_frames_inside_the_window = frames_of_tabs_inside_a_header(frame.size, tabs.len());

    unsafe { SLSDisableUpdate(connection) };
    unsafe {
        SLSSetWindowShape(
            connection,
            header_window.id.0,
            0.0,
            0.0,
            frame_region.cast_const(),
        )
    };
    CGContextClearRect(
        context,
        CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: frame.size,
        },
    );
    CGContextSetTextMatrix(context, unsafe { CGAffineTransformIdentity });
    fill_rounded_rectangle(
        context,
        background_frame,
        style.background_color,
        LARGEST_BACKGROUND_CORNER_RADIUS,
    );
    for (tab, tab_frame) in tabs.iter().zip(&tab_frames_inside_the_window) {
        let (tab_color, text_color) = if tab.is_the_front_window {
            (style.active_color, style.active_text_color)
        } else {
            (style.inactive_color, style.inactive_text_color)
        };
        fill_rounded_rectangle(context, *tab_frame, tab_color, LARGEST_TAB_CORNER_RADIUS);
        draw_title_inside_tab(context, &tab.title, &font, *tab_frame, text_color);
    }
    CGContextFlush(context);
    unsafe { SLSReenableUpdate(connection) };
    drop(unsafe { take_create_rule_result(frame_region.cast_const()) });

    header_window.frame = frame;
    header_window.tab_frames = tabs
        .iter()
        .zip(&tab_frames_inside_the_window)
        .map(|(tab, tab_frame)| (tab.window_id, screen_frame_of_tab(frame, *tab_frame)))
        .collect();
}

pub(crate) fn order_group_header_window_right_above_window(
    header_window: &GroupHeaderWindow,
    window_id: WindowId,
) {
    let connection = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    unsafe {
        SLSSetWindowLevel(
            connection,
            header_window.id.0,
            query_window_level_from_window_server(window_id),
        )
    };
    unsafe {
        SLSSetWindowSubLevel(
            connection,
            header_window.id.0,
            query_window_sub_level_from_window_server(window_id),
        )
    };
    unsafe { SLSOrderWindow(connection, header_window.id.0, 1, window_id.0) };
}

pub(crate) fn hide_group_header_window(header_window: &GroupHeaderWindow) {
    let connection = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    unsafe { SLSOrderWindow(connection, header_window.id.0, 0, 0) };
}

fn frame_of_the_background_inside_a_header(header_size: CGSize) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: 0.0,
            y: SPACE_BETWEEN_THE_HEADER_AND_THE_WINDOWS_BELOW,
        },
        size: CGSize {
            width: header_size.width,
            height: (header_size.height - SPACE_BETWEEN_THE_HEADER_AND_THE_WINDOWS_BELOW).max(0.0),
        },
    }
}

fn frames_of_tabs_inside_a_header(header_size: CGSize, tab_count: usize) -> Vec<CGRect> {
    if tab_count == 0 {
        return Vec::new();
    }
    let background_frame = frame_of_the_background_inside_a_header(header_size);
    let inset = SPACE_BETWEEN_THE_BACKGROUND_AND_THE_TABS_INSIDE_IT;
    let tab_height = (background_frame.size.height - 2.0 * inset).max(0.0);
    let tab_width =
        ((background_frame.size.width - 2.0 * inset - SPACE_BETWEEN_TABS * (tab_count - 1) as f64)
            / tab_count as f64)
            .max(0.0);
    (0..tab_count)
        .map(|index| CGRect {
            origin: CGPoint {
                x: inset + index as f64 * (tab_width + SPACE_BETWEEN_TABS),
                y: background_frame.origin.y + inset,
            },
            size: CGSize {
                width: tab_width,
                height: tab_height,
            },
        })
        .collect()
}

fn screen_frame_of_tab(header_frame: CGRect, tab_frame_inside_the_window: CGRect) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: header_frame.origin.x + tab_frame_inside_the_window.origin.x,
            y: header_frame.origin.y + header_frame.size.height
                - tab_frame_inside_the_window.origin.y
                - tab_frame_inside_the_window.size.height,
        },
        size: tab_frame_inside_the_window.size,
    }
}

fn fill_rounded_rectangle(
    context: Option<&CGContext>,
    frame: CGRect,
    color: RgbaColor,
    largest_corner_radius: f64,
) {
    if color.alpha <= 0.0 {
        return;
    }
    let corner_radius = largest_corner_radius
        .min(0.5 * frame.size.width)
        .min(0.5 * frame.size.height)
        .max(0.0);
    let tab_path = unsafe {
        CGPathCreateWithRoundedRect(frame, corner_radius, corner_radius, std::ptr::null())
    };
    CGContextSetRGBFillColor(
        context,
        color.red as f64,
        color.green as f64,
        color.blue as f64,
        color.alpha as f64,
    );
    CGContextAddPath(context, Some(&tab_path));
    CGContextDrawPath(context, CGPathDrawingMode::Fill);
}

fn draw_title_inside_tab(
    context: Option<&CGContext>,
    title: &str,
    font: &CTFont,
    tab_frame: CGRect,
    text_color: RgbaColor,
) {
    let width_left_for_the_title = tab_frame.size.width - 2.0 * TEXT_INSET_FROM_THE_EDGES_OF_A_TAB;
    if width_left_for_the_title <= 0.0 {
        return;
    }
    let Some(title_line) = create_line_of_text_coloured_by_the_context_fill(title, font) else {
        return;
    };
    let Some(truncation_token) =
        create_line_of_text_coloured_by_the_context_fill(TEXT_THAT_ENDS_A_TRUNCATED_TITLE, font)
    else {
        return;
    };
    let Some(fitting_title_line) = (unsafe {
        title_line.truncated_line(
            width_left_for_the_title,
            CTLineTruncationType::End,
            Some(&truncation_token),
        )
    }) else {
        return;
    };

    let bounds = typographic_bounds_of_line(&fitting_title_line);
    let baseline_y = tab_frame.origin.y
        + 0.5 * (tab_frame.size.height - (bounds.ascent + bounds.descent))
        + bounds.descent;
    CGContextSetRGBFillColor(
        context,
        text_color.red as f64,
        text_color.green as f64,
        text_color.blue as f64,
        text_color.alpha as f64,
    );
    CGContextSetTextPosition(
        context,
        tab_frame.origin.x + TEXT_INSET_FROM_THE_EDGES_OF_A_TAB,
        baseline_y,
    );
    if let Some(context) = context {
        unsafe { fitting_title_line.draw(context) };
    }
}

#[cfg(test)]
mod tests {
    use super::{frames_of_tabs_inside_a_header, screen_frame_of_tab};
    use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};

    fn rectangle(x: f64, y: f64, width: f64, height: f64) -> CGRect {
        CGRect {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        }
    }

    fn corners(rectangle: CGRect) -> (f64, f64, f64, f64) {
        (
            rectangle.origin.x,
            rectangle.origin.y,
            rectangle.size.width,
            rectangle.size.height,
        )
    }

    #[test]
    fn tabs_share_the_background_width_equally_inset_from_its_edges_with_a_gap_between_them() {
        let tab_frames = frames_of_tabs_inside_a_header(
            CGSize {
                width: 314.0,
                height: 30.0,
            },
            3,
        );

        assert_eq!(
            tab_frames.iter().copied().map(corners).collect::<Vec<_>>(),
            vec![
                (3.0, 7.0, 100.0, 20.0),
                (107.0, 7.0, 100.0, 20.0),
                (211.0, 7.0, 100.0, 20.0),
            ]
        );
    }

    #[test]
    fn a_header_without_windows_has_no_tabs() {
        assert!(
            frames_of_tabs_inside_a_header(
                CGSize {
                    width: 300.0,
                    height: 24.0
                },
                0
            )
            .is_empty()
        );
    }

    #[test]
    fn a_tab_drawn_from_the_bottom_of_the_header_sits_at_its_top_on_screen() {
        let header_frame_on_screen = rectangle(100.0, 40.0, 308.0, 24.0);
        let tab_frame_inside_the_window = rectangle(104.0, 4.0, 100.0, 20.0);

        assert_eq!(
            corners(screen_frame_of_tab(
                header_frame_on_screen,
                tab_frame_inside_the_window
            )),
            (204.0, 40.0, 100.0, 20.0)
        );
    }
}
