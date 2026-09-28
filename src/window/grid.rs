use crate::display::bounds::query_bounds_of_display_left_for_windows;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::layout::settings::{ViewFlag, effective_window_gap_of_view};
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::support::handles::WindowId;
use crate::window::animation::{
    WindowWithTargetFrame, move_window_to_its_target_frame_animating_if_enabled,
};
use crate::window::manager::{WindowManager, WindowOperationOutcome, space_managing_window};
use crate::window::model::query_display_holding_window;

pub(crate) fn place_floating_window_on_display_grid(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    rows: u32,
    columns: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    display_manager: &mut DisplayManager,
) -> WindowOperationOutcome {
    let mut x = x;
    let mut y = y;
    let mut width = width;
    let mut height = height;

    let view = space_managing_window(window_manager, window_id);
    if view.is_some() {
        return WindowOperationOutcome::InvalidSourceView;
    }

    let display_id = query_display_holding_window(window_id);
    if display_id.0 == 0 {
        return WindowOperationOutcome::InvalidSourceView;
    }

    if x >= columns {
        x = columns.wrapping_sub(1);
    }
    if y >= rows {
        y = rows.wrapping_sub(1);
    }
    if width == 0 {
        width = 1;
    }
    if height == 0 {
        height = 1;
    }
    if width > columns.wrapping_sub(x) {
        width = columns.wrapping_sub(x);
    }
    if height > rows.wrapping_sub(y) {
        height = rows.wrapping_sub(y);
    }

    let mut bounds = query_bounds_of_display_left_for_windows(display_id, false, display_manager);
    let display_view = find_or_create_view_for_space(
        space_manager,
        query_current_space_of_display(display_id),
        display_manager,
        window_manager,
    );

    if let Some(view) = space_manager.view.find(&display_view) {
        let enable_gap = view.has_flag(ViewFlag::WINDOW_GAP_IS_ENABLED);

        if view.has_flag(ViewFlag::PADDING_IS_ENABLED) {
            bounds.origin.x += view.left_padding as f64;
            bounds.size.width -= (view.left_padding + view.right_padding) as f64;
            bounds.origin.y += view.top_padding as f64;
            bounds.size.height -= (view.top_padding + view.bottom_padding) as f64;
        }

        if enable_gap {
            let gap = effective_window_gap_of_view(space_manager, display_view);

            if x > 0 {
                bounds.origin.x += gap as f64;
                bounds.size.width -= gap as f64;
            }

            if y > 0 {
                bounds.origin.y += gap as f64;
                bounds.size.height -= gap as f64;
            }

            if columns > x.wrapping_add(width) {
                bounds.size.width -= gap as f64;
            }
            if rows > y.wrapping_add(height) {
                bounds.size.height -= gap as f64;
            }
        }
    }

    let column_width = (bounds.size.width / columns as f64) as f32;
    let row_height = (bounds.size.height / rows as f64) as f32;
    let frame_x = (bounds.origin.x + bounds.size.width
        - (column_width * columns.wrapping_sub(x) as f32) as f64) as f32;
    let frame_y = (bounds.origin.y + bounds.size.height
        - (row_height * rows.wrapping_sub(y) as f32) as f64) as f32;
    let frame_width = column_width * width as f32;
    let frame_height = row_height * height as f32;

    move_window_to_its_target_frame_animating_if_enabled(
        WindowWithTargetFrame {
            window_id,
            x: frame_x,
            y: frame_y,
            width: frame_width,
            height: frame_height,
        },
        window_manager,
    );
    WindowOperationOutcome::Success
}
