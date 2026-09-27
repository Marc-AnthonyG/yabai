use crate::display::bounds::display_bounds_constrained;
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_id;
use crate::layout::settings::{ViewFlag, window_node_get_gap};
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::support::handles::WindowId;
use crate::window::animation::{WindowCapture, window_manager_animate_window};
use crate::window::manager::{WindowManager, WindowOpError, window_manager_find_managed_window};
use crate::window::model::window_display_id;

pub(crate) fn window_manager_apply_grid(
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
) -> WindowOpError {
    let mut x = x;
    let mut y = y;
    let mut width = width;
    let mut height = height;

    let view = window_manager_find_managed_window(window_manager, window_id);
    if view.is_some() {
        return WindowOpError::InvalidSrcView;
    }

    let display_id = window_display_id(window_id);
    if display_id.0 == 0 {
        return WindowOpError::InvalidSrcView;
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

    let mut bounds = display_bounds_constrained(display_id, false, display_manager);
    let display_view = space_manager_find_view(
        space_manager,
        display_space_id(display_id),
        display_manager,
        window_manager,
    );

    if let Some(view) = space_manager.view.find(&display_view) {
        let enable_gap = view.check_flag(ViewFlag::ENABLE_GAP);

        if view.check_flag(ViewFlag::ENABLE_PADDING) {
            bounds.origin.x += view.left_padding as f64;
            bounds.size.width -= (view.left_padding + view.right_padding) as f64;
            bounds.origin.y += view.top_padding as f64;
            bounds.size.height -= (view.top_padding + view.bottom_padding) as f64;
        }

        if enable_gap {
            let gap = window_node_get_gap(space_manager, display_view);

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

    window_manager_animate_window(
        WindowCapture {
            window_id,
            x: frame_x,
            y: frame_y,
            width: frame_width,
            height: frame_height,
        },
        window_manager,
    );
    WindowOpError::Success
}
