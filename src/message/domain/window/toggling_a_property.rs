use crate::command::window::WindowToggleableProperty;
use crate::display::manager::DisplayManager;
use crate::message::domain::window::operation_failures::fail_unless_the_window_moved_in_the_tree;
use crate::mouse::drag::MouseDragState;
use crate::space::manager::SpaceManager;
use crate::space::tiling::toggle_split_direction_of_the_parent_of_window_leaf;
use crate::support::handles::WindowId;
use crate::window::floating_and_sticky::{set_whether_window_floats, set_whether_window_is_sticky};
use crate::window::focus::toggle_application_expose_for_window;
use crate::window::fullscreen::{
    toggle_managed_window_zoom_fullscreen, toggle_managed_window_zoom_parent,
    toggle_window_native_fullscreen, toggle_window_picture_in_picture,
    toggle_window_windowed_fullscreen,
};
use crate::window::group::toggle_group_of_window;
use crate::window::manager::WindowManager;
use crate::window::model::WindowFlag;
use crate::window::shadow::toggle_window_shadow;

pub(crate) fn toggle_property_of_window(
    property: WindowToggleableProperty,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> Result<(), String> {
    match property {
        WindowToggleableProperty::Float => {
            if let Some(is_floating) =
                does_window_have_flag(window_manager, window_id, WindowFlag::FLOATING)
            {
                set_whether_window_floats(
                    space_manager,
                    window_manager,
                    window_id,
                    !is_floating,
                    false,
                    display_manager,
                    mouse_drag_state,
                );
            }
        }
        WindowToggleableProperty::Sticky => {
            if let Some(is_sticky) =
                does_window_have_flag(window_manager, window_id, WindowFlag::STICKY)
            {
                set_whether_window_is_sticky(
                    space_manager,
                    window_manager,
                    window_id,
                    !is_sticky,
                    display_manager,
                    mouse_drag_state,
                );
            }
        }
        WindowToggleableProperty::Pip => toggle_window_picture_in_picture(
            space_manager,
            window_id,
            display_manager,
            window_manager,
        ),
        WindowToggleableProperty::Shadow => toggle_window_shadow(window_id, window_manager),
        WindowToggleableProperty::Split => toggle_split_direction_of_the_parent_of_window_leaf(
            space_manager,
            window_id,
            display_manager,
            window_manager,
        ),
        WindowToggleableProperty::ZoomParent => {
            toggle_managed_window_zoom_parent(window_manager, window_id, space_manager)
        }
        WindowToggleableProperty::ZoomFullscreen => {
            toggle_managed_window_zoom_fullscreen(window_manager, window_id, space_manager)
        }
        WindowToggleableProperty::WindowedFullscreen => {
            toggle_window_windowed_fullscreen(window_id, display_manager, window_manager)
        }
        WindowToggleableProperty::NativeFullscreen => {
            toggle_window_native_fullscreen(window_id, window_manager)
        }
        WindowToggleableProperty::Expose => {
            toggle_application_expose_for_window(window_id, window_manager)
        }
        WindowToggleableProperty::Group => {
            return fail_unless_the_window_moved_in_the_tree(
                toggle_group_of_window(
                    space_manager,
                    window_id,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                ),
                "group",
            );
        }
    }
    Ok(())
}

fn does_window_have_flag(
    window_manager: &WindowManager,
    window_id: WindowId,
    flag: WindowFlag,
) -> Option<bool> {
    window_manager
        .window
        .get(&window_id)
        .map(|window| window.flags.contains(flag))
}
