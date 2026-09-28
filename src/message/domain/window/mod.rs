pub mod operation_failures;
pub mod toggling_a_property;

use crate::command::selectors::WindowSelector;
use crate::command::window::{WindowAction, WindowCommand};
use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::message::domain::window::operation_failures::{
    fail_unless_the_split_ratio_changed, fail_unless_the_window_moved_in_the_tree,
    fail_unless_the_window_was_deminimized, fail_unless_the_window_was_minimized,
    fail_unless_the_window_was_resized,
};
use crate::message::domain::window::toggling_a_property::toggle_property_of_window;
use crate::message::selector_resolution::{
    resolve_display_selector, resolve_space_selector, resolve_window_selector,
    resolve_window_selector_or_the_focused_window_if_there_is_one,
};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::scripting_addition::client::order_window_relative_to_other_window_through_scripting_addition;
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::managed_space::is_native_fullscreen_space;
use crate::space::manager::SpaceManager;
use crate::support::handles::{SpaceId, WindowId};
use crate::window::focus::focus_and_raise_tracked_window;
use crate::window::frame::{
    adjust_split_ratio_of_managed_window_parent_node,
    move_floating_window_by_offset_or_to_position,
    resize_window_by_dragging_edges_or_to_absolute_size,
};
use crate::window::grid::place_floating_window_on_display_grid;
use crate::window::layer::set_window_layer_for_it_and_its_child_windows;
use crate::window::manager::{WindowManager, WindowOperationOutcome};
use crate::window::minimize_and_close::{
    close_window_by_pressing_its_close_button, deminimize_window_through_accessibility,
    minimize_window_through_accessibility,
};
use crate::window::opacity::apply_opacity_to_window_through_scripting_addition;
use crate::window::send_to_space::send_window_to_space;
use crate::window::tree_placement::{
    stack_second_window_onto_the_node_of_first_window, swap_managed_windows,
    toggle_insertion_point_at_window_in_direction,
    warp_first_window_into_the_node_of_second_window,
};

const ORDER_ABOVE_THE_OTHER_WINDOW: i32 = 1;
const ORDER_BELOW_THE_OTHER_WINDOW: i32 = -1;

pub(crate) fn run_window_command(
    command: WindowCommand,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> Result<String, String> {
    let acting_window_id = resolve_window_selector_or_the_focused_window_if_there_is_one(
        command.window.as_ref(),
        display_manager,
        window_manager,
        space_manager,
    )?;
    match command.action {
        WindowAction::Focus { target } => {
            let window_id = resolve_the_target_or_the_acting_window(
                target.as_ref(),
                acting_window_id,
                display_manager,
                window_manager,
                space_manager,
            )?;
            focus_and_raise_tracked_window(window_manager, window_id);
        }
        WindowAction::Close { target } => {
            let window_id = resolve_the_target_or_the_acting_window(
                target.as_ref(),
                acting_window_id,
                display_manager,
                window_manager,
                space_manager,
            )?;
            if !close_window_by_pressing_its_close_button(window_id, window_manager) {
                return Err(format!("could not close window with id '{}'.", window_id.0));
            }
        }
        WindowAction::Minimize { target } => {
            let window_id = resolve_the_target_or_the_acting_window(
                target.as_ref(),
                acting_window_id,
                display_manager,
                window_manager,
                space_manager,
            )?;
            fail_unless_the_window_was_minimized(
                minimize_window_through_accessibility(window_id, window_manager),
                window_id,
            )?;
        }
        WindowAction::Deminimize { target } => {
            let window_id = resolve_window_selector(
                &target,
                acting_window_id,
                display_manager,
                window_manager,
                space_manager,
            )?;
            fail_unless_the_window_was_deminimized(
                deminimize_window_through_accessibility(window_id, window_manager),
                window_id,
            )?;
        }
        WindowAction::SendToDisplay { display } => {
            let display_id = resolve_display_selector(
                &display,
                query_display_showing_the_active_menu_bar(),
                display_manager,
            )?;
            send_window_to_space_unless_it_is_a_native_fullscreen_space(
                the_acting_window(acting_window_id)?,
                query_current_space_of_display(display_id),
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            )?;
        }
        WindowAction::SendToSpace { space } => {
            let space_id = resolve_space_selector(
                &space,
                query_current_space_of_the_focused_display(window_manager),
                space_manager,
            )?;
            send_window_to_space_unless_it_is_a_native_fullscreen_space(
                the_acting_window(acting_window_id)?,
                space_id,
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            )?;
        }
        WindowAction::Swap { target } => {
            let (acting_window_id, target_window_id) = resolve_the_acting_window_and_the_target(
                acting_window_id,
                &target,
                display_manager,
                window_manager,
                space_manager,
            )?;
            fail_unless_the_window_moved_in_the_tree(
                swap_managed_windows(
                    space_manager,
                    window_manager,
                    acting_window_id,
                    target_window_id,
                    display_manager,
                ),
                "swap",
            )?;
        }
        WindowAction::Warp { target } => {
            let (acting_window_id, target_window_id) = resolve_the_acting_window_and_the_target(
                acting_window_id,
                &target,
                display_manager,
                window_manager,
                space_manager,
            )?;
            fail_unless_the_window_moved_in_the_tree(
                warp_first_window_into_the_node_of_second_window(
                    space_manager,
                    window_manager,
                    acting_window_id,
                    target_window_id,
                    process_manager,
                    display_manager,
                    mouse_drag_state,
                ),
                "warp",
            )?;
        }
        WindowAction::Stack { target } => {
            let (acting_window_id, target_window_id) = resolve_the_acting_window_and_the_target(
                acting_window_id,
                &target,
                display_manager,
                window_manager,
                space_manager,
            )?;
            fail_unless_the_window_moved_in_the_tree(
                stack_second_window_onto_the_node_of_first_window(
                    space_manager,
                    window_manager,
                    acting_window_id,
                    target_window_id,
                    display_manager,
                    mouse_drag_state,
                ),
                "stack",
            )?;
        }
        WindowAction::Insert { direction } => fail_unless_the_window_moved_in_the_tree(
            toggle_insertion_point_at_window_in_direction(
                space_manager,
                the_acting_window(acting_window_id)?,
                direction.insert_direction_of_a_tree_node(),
                display_manager,
                window_manager,
            ),
            "insert",
        )?,
        WindowAction::Grid { placement } => {
            if place_floating_window_on_display_grid(
                space_manager,
                window_manager,
                the_acting_window(acting_window_id)?,
                placement.rows,
                placement.columns,
                placement.column,
                placement.row,
                placement.width,
                placement.height,
                display_manager,
            ) == WindowOperationOutcome::InvalidSourceView
            {
                return Err(String::from(
                    "cannot apply grid layout to a managed window.",
                ));
            }
        }
        WindowAction::Move { change, x, y } => {
            if move_floating_window_by_offset_or_to_position(
                window_manager,
                the_acting_window(acting_window_id)?,
                change,
                x,
                y,
            ) == WindowOperationOutcome::InvalidSourceView
            {
                return Err(String::from("cannot move a managed window."));
            }
        }
        WindowAction::Resize {
            handle,
            width,
            height,
        } => fail_unless_the_window_was_resized(
            resize_window_by_dragging_edges_or_to_absolute_size(
                window_manager,
                the_acting_window(acting_window_id)?,
                i32::from(handle.resize_handle().0),
                width,
                height,
                true,
                display_manager,
                space_manager,
            ),
        )?,
        WindowAction::Ratio { change, ratio } => {
            fail_unless_the_split_ratio_changed(adjust_split_ratio_of_managed_window_parent_node(
                window_manager,
                the_acting_window(acting_window_id)?,
                change,
                ratio,
                space_manager,
            ))?
        }
        WindowAction::Toggle { property } => toggle_property_of_window(
            property,
            the_acting_window(acting_window_id)?,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        )?,
        WindowAction::SubLayer { layer } => {
            let window_id = the_acting_window(acting_window_id)?;
            if !set_window_layer_for_it_and_its_child_windows(
                window_id,
                layer.layer(),
                window_manager,
            ) {
                return Err(format!(
                    "could not change sub-layer of window with id '{}' due to an error with the scripting-addition.",
                    window_id.0
                ));
            }
        }
        WindowAction::Opacity { opacity } => set_opacity_of_window(
            the_acting_window(acting_window_id)?,
            opacity,
            window_manager,
        )?,
        WindowAction::Raise { above } => order_window_above_or_below_another_window(
            the_acting_window(acting_window_id)?,
            ORDER_ABOVE_THE_OTHER_WINDOW,
            above.as_ref(),
            display_manager,
            window_manager,
            space_manager,
        )?,
        WindowAction::Lower { below } => order_window_above_or_below_another_window(
            the_acting_window(acting_window_id)?,
            ORDER_BELOW_THE_OTHER_WINDOW,
            below.as_ref(),
            display_manager,
            window_manager,
            space_manager,
        )?,
    }
    Ok(String::new())
}

fn the_acting_window(acting_window_id: Option<WindowId>) -> Result<WindowId, String> {
    acting_window_id.ok_or_else(|| String::from("could not locate the window to act on."))
}

fn resolve_the_target_or_the_acting_window(
    target: Option<&WindowSelector>,
    acting_window_id: Option<WindowId>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<WindowId, String> {
    match target {
        Some(target) => resolve_window_selector(
            target,
            acting_window_id,
            display_manager,
            window_manager,
            space_manager,
        ),
        None => the_acting_window(acting_window_id),
    }
}

fn resolve_the_acting_window_and_the_target(
    acting_window_id: Option<WindowId>,
    target: &WindowSelector,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<(WindowId, WindowId), String> {
    let acting_window_id = the_acting_window(acting_window_id)?;
    let target_window_id = resolve_window_selector(
        target,
        Some(acting_window_id),
        display_manager,
        window_manager,
        space_manager,
    )?;
    Ok((acting_window_id, target_window_id))
}

fn send_window_to_space_unless_it_is_a_native_fullscreen_space(
    window_id: WindowId,
    space_id: SpaceId,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> Result<(), String> {
    if is_native_fullscreen_space(space_id) {
        return Err(String::from(
            "can not move window to a macOS fullscreen space.",
        ));
    }
    send_window_to_space(
        space_manager,
        window_manager,
        window_id,
        space_id,
        false,
        process_manager,
        display_manager,
        mouse_drag_state,
    );
    Ok(())
}

fn set_opacity_of_window(
    window_id: WindowId,
    opacity: f32,
    window_manager: &mut WindowManager,
) -> Result<(), String> {
    if !apply_opacity_to_window_through_scripting_addition(window_manager, window_id, opacity) {
        return Err(format!(
            "could not change opacity of window with id '{}' due to an error with the scripting-addition.",
            window_id.0
        ));
    }
    if let Some(window) = window_manager.window.get_mut(&window_id) {
        window.opacity = opacity;
    }
    Ok(())
}

fn order_window_above_or_below_another_window(
    window_id: WindowId,
    order: i32,
    other_window: Option<&WindowSelector>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<(), String> {
    let other_window_id = match other_window {
        Some(other_window) => resolve_window_selector(
            other_window,
            Some(window_id),
            display_manager,
            window_manager,
            space_manager,
        )?,
        None => WindowId(0),
    };
    if !order_window_relative_to_other_window_through_scripting_addition(
        window_id,
        order,
        other_window_id,
    ) {
        let operation = if order == ORDER_ABOVE_THE_OTHER_WINDOW {
            "raise"
        } else {
            "lower"
        };
        return Err(format!(
            "could not {operation} window with id '{}' due to an error with the scripting-addition.",
            window_id.0
        ));
    }
    Ok(())
}
