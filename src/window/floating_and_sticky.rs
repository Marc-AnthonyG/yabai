use crate::display::manager::DisplayManager;
use crate::mouse::drag::MouseDragState;
use crate::scripting_addition::client::set_window_sticky_through_scripting_addition;
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::manager::SpaceManager;
use crate::space::tiling::{tile_window_on_space, untile_window_from_view_of_space};
use crate::support::handles::WindowId;
use crate::window::manager::{
    WindowManager, forget_managed_window, is_window_eligible_for_management,
    record_managed_window_on_space_updating_its_shadow, should_window_be_managed,
    space_managing_window,
};
use crate::window::model::{
    WindowFlag, WindowRuleFlag, is_window_a_standard_window, is_window_at_normal_window_level,
    is_window_movable,
};
use crate::window::shadow::apply_shadow_removal_mode_to_window;

pub(crate) fn set_whether_window_floats(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    should_float: bool,
    force: bool,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if !is_window_eligible_for_management(window_id, window_manager) {
        return;
    }

    if !force {
        let Some(window) = window_manager.window.get(&window_id) else {
            return;
        };
        if !is_window_a_standard_window(window)
            || !is_window_at_normal_window_level(window)
            || !is_window_movable(window)
        {
            if !window.rule_flags.contains(WindowRuleFlag::MANAGE_FORCED_ON) {
                return;
            }
        }
    }

    if should_float {
        let view = space_managing_window(window_manager, window_id);
        if let Some(space_id) = view {
            untile_window_from_view_of_space(
                space_manager,
                space_id,
                window_id,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            forget_managed_window(window_manager, window_id);
            apply_shadow_removal_mode_to_window(window_manager, window_id);
        }
        if let Some(window) = window_manager.window.get_mut(&window_id) {
            window.flags.insert(WindowFlag::FLOATING);
        }
    } else {
        let Some(window) = window_manager.window.get_mut(&window_id) else {
            return;
        };
        window.flags.remove(WindowFlag::FLOATING);

        if !window.flags.contains(WindowFlag::STICKY) {
            if (should_window_be_managed(window_id, window_manager))
                && (space_managing_window(window_manager, window_id).is_none())
            {
                let view = tile_window_on_space(
                    space_manager,
                    window_id,
                    query_current_space_of_the_focused_display(window_manager),
                    display_manager,
                    window_manager,
                );
                record_managed_window_on_space_updating_its_shadow(
                    window_manager,
                    window_id,
                    space_manager,
                    view,
                );
            }
        }
    }
}

pub(crate) fn set_whether_window_is_sticky(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    should_sticky: bool,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if !is_window_eligible_for_management(window_id, window_manager) {
        return;
    }

    if should_sticky {
        if set_window_sticky_through_scripting_addition(window_id, true) {
            let view = space_managing_window(window_manager, window_id);
            if let Some(space_id) = view {
                untile_window_from_view_of_space(
                    space_manager,
                    space_id,
                    window_id,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
                forget_managed_window(window_manager, window_id);
                apply_shadow_removal_mode_to_window(window_manager, window_id);
            }
            if let Some(window) = window_manager.window.get_mut(&window_id) {
                window.flags.insert(WindowFlag::STICKY);
            }
        }
    } else {
        if set_window_sticky_through_scripting_addition(window_id, false) {
            let Some(window) = window_manager.window.get_mut(&window_id) else {
                return;
            };
            window.flags.remove(WindowFlag::STICKY);

            if !window.flags.contains(WindowFlag::FLOATING) {
                if (should_window_be_managed(window_id, window_manager))
                    && (space_managing_window(window_manager, window_id).is_none())
                {
                    let view = tile_window_on_space(
                        space_manager,
                        window_id,
                        query_current_space_of_the_focused_display(window_manager),
                        display_manager,
                        window_manager,
                    );
                    record_managed_window_on_space_updating_its_shadow(
                        window_manager,
                        window_id,
                        space_manager,
                        view,
                    );
                }
            }
        }
    }
}
