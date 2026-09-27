use crate::display::manager::DisplayManager;
use crate::mouse::drag::MouseDragState;
use crate::scripting_addition::client::scripting_addition_set_sticky;
use crate::space::focus::space_manager_active_space;
use crate::space::manager::SpaceManager;
use crate::space::tiling::{space_manager_tile_window_on_space, space_manager_untile_window};
use crate::support::handles::WindowId;
use crate::window::manager::{
    WindowManager, window_manager_add_managed_window, window_manager_find_managed_window,
    window_manager_is_window_eligible, window_manager_remove_managed_window,
    window_manager_should_manage_window,
};
use crate::window::model::{
    WindowFlag, WindowRuleFlag, window_can_move, window_check_flag, window_check_rule_flag,
    window_clear_flag, window_is_standard, window_level_is_standard, window_set_flag,
};
use crate::window::shadow::window_manager_purify_window;

pub(crate) fn window_manager_make_window_floating(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    should_float: bool,
    force: bool,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if !window_manager_is_window_eligible(window_id, window_manager) {
        return;
    }

    if !force {
        let Some(window) = window_manager.window.find(&window_id) else {
            return;
        };
        if !window_is_standard(window)
            || !window_level_is_standard(window)
            || !window_can_move(window)
        {
            if !window_check_rule_flag(window, WindowRuleFlag::MANAGED) {
                return;
            }
        }
    }

    if should_float {
        let view = window_manager_find_managed_window(window_manager, window_id);
        if let Some(space_id) = view {
            space_manager_untile_window(
                space_manager,
                space_id,
                window_id,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_remove_managed_window(window_manager, window_id);
            window_manager_purify_window(window_manager, window_id);
        }
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_flag(window, WindowFlag::FLOAT);
        }
    } else {
        let Some(window) = window_manager.window.find_mut(&window_id) else {
            return;
        };
        window_clear_flag(window, WindowFlag::FLOAT);

        if !window_check_flag(window, WindowFlag::STICKY) {
            if (window_manager_should_manage_window(window_id, window_manager))
                && (window_manager_find_managed_window(window_manager, window_id).is_none())
            {
                let view = space_manager_tile_window_on_space(
                    space_manager,
                    window_id,
                    space_manager_active_space(window_manager),
                    display_manager,
                    window_manager,
                );
                window_manager_add_managed_window(window_manager, window_id, space_manager, view);
            }
        }
    }
}

pub(crate) fn window_manager_make_window_sticky(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    should_sticky: bool,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if !window_manager_is_window_eligible(window_id, window_manager) {
        return;
    }

    if should_sticky {
        if scripting_addition_set_sticky(window_id, true) {
            let view = window_manager_find_managed_window(window_manager, window_id);
            if let Some(space_id) = view {
                space_manager_untile_window(
                    space_manager,
                    space_id,
                    window_id,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
                window_manager_remove_managed_window(window_manager, window_id);
                window_manager_purify_window(window_manager, window_id);
            }
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                window_set_flag(window, WindowFlag::STICKY);
            }
        }
    } else {
        if scripting_addition_set_sticky(window_id, false) {
            let Some(window) = window_manager.window.find_mut(&window_id) else {
                return;
            };
            window_clear_flag(window, WindowFlag::STICKY);

            if !window_check_flag(window, WindowFlag::FLOAT) {
                if (window_manager_should_manage_window(window_id, window_manager))
                    && (window_manager_find_managed_window(window_manager, window_id).is_none())
                {
                    let view = space_manager_tile_window_on_space(
                        space_manager,
                        window_id,
                        space_manager_active_space(window_manager),
                        display_manager,
                        window_manager,
                    );
                    window_manager_add_managed_window(
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
