use crate::display::identity::display_manager_active_display_list;
use crate::display::manager::DisplayManager;
use crate::display::spaces::{display_space_id, display_space_list};
use crate::handles::{DisplayId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::layout::settings::{ViewFlag, ViewType};
use crate::layout::tree::{
    view_add_window_node, view_find_window_list, view_remove_window_node, window_node_flush,
};
use crate::layout::view::view_is_dirty;
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::{space_is_user, space_is_visible, space_window_list};
use crate::space::manager::{
    SpaceManager, space_manager_find_view, space_manager_mark_view_invalid,
    space_manager_refresh_view,
};
use crate::support::layer::{LAYER_BELOW, LAYER_NORMAL};
use crate::window::layer::window_manager_adjust_layer;
use crate::window::manager::{
    WindowManager, window_manager_add_managed_window, window_manager_find_managed_window,
    window_manager_find_window, window_manager_remove_managed_window,
    window_manager_should_manage_window,
};
use crate::window::shadow::window_manager_purify_window;

pub(crate) fn window_manager_validate_windows_on_space(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_list: &[WindowId],
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let view_window_list = view_find_window_list(space_manager, space_id);

    for index in 0..view_window_list.len() {
        let mut found = false;

        for inner_index in 0..window_list.len() {
            if view_window_list[index] == window_list[inner_index] {
                found = true;
                break;
            }
        }

        if !found {
            let Some(window) = window_manager_find_window(window_manager, view_window_list[index])
            else {
                continue;
            };

            //
            // @cleanup
            //
            // :AXBatching
            //
            // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
            // making sure that each window is only moved and resized a single time, when the final layout has been computed.
            // This is necessary to make sure that we do not call the AX API for each modification to the tree.
            //

            view_remove_window_node(
                space_manager,
                space_id,
                window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_adjust_layer(window, LAYER_NORMAL, window_manager);
            window_manager_remove_managed_window(window_manager, window);
            window_manager_purify_window(window_manager, window);

            if let Some(view) = space_manager.view.find_mut(&space_id) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
        }
    }
}

pub(crate) fn window_manager_check_for_windows_on_space(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_list: &[WindowId],
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    for index in 0..window_list.len() {
        let window = window_manager_find_window(window_manager, window_list[index]);
        let Some(window) = window else {
            continue;
        };
        if !window_manager_should_manage_window(window, window_manager) {
            continue;
        }

        let existing_view = window_manager_find_managed_window(window_manager, window);
        let existing_view_layout = existing_view
            .and_then(|existing_space_id| space_manager.view.find(&existing_space_id))
            .map(|view| view.layout);
        if let Some(existing_space_id) = existing_view
            && existing_view_layout != Some(ViewType::Float)
            && existing_space_id != space_id
        {
            //
            // @cleanup
            //
            // :AXBatching
            //
            // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
            // making sure that each window is only moved and resized a single time, when the final layout has been computed.
            // This is necessary to make sure that we do not call the AX API for each modification to the tree.
            //

            view_remove_window_node(
                space_manager,
                existing_space_id,
                window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_adjust_layer(window, LAYER_NORMAL, window_manager);
            window_manager_remove_managed_window(window_manager, window);
            window_manager_purify_window(window_manager, window);
            if let Some(view) = space_manager.view.find_mut(&existing_space_id) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
        }

        if existing_view.is_none()
            || (existing_view_layout != Some(ViewType::Float) && existing_view != Some(space_id))
        {
            //
            // @cleanup
            //
            // :AXBatching
            //
            // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
            // making sure that each window is only moved and resized a single time, when the final layout has been computed.
            // This is necessary to make sure that we do not call the AX API for each modification to the tree.
            //

            view_add_window_node(
                space_manager,
                space_id,
                window,
                display_manager,
                window_manager,
            );
            window_manager_adjust_layer(window, LAYER_BELOW, window_manager);
            window_manager_add_managed_window(window_manager, window, space_manager, space_id);
            if let Some(view) = space_manager.view.find_mut(&space_id) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
        }
    }
}

pub(crate) fn window_manager_validate_and_check_for_windows_on_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let view = space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    if space_manager
        .view
        .find(&view)
        .is_none_or(|view| view.layout == ViewType::Float)
    {
        return;
    }

    let window_list = space_window_list(space_id, false, window_manager).unwrap_or_default();
    window_manager_validate_windows_on_space(
        window_manager,
        space_manager,
        view,
        &window_list,
        display_manager,
        mouse_drag_state,
    );
    window_manager_check_for_windows_on_space(
        window_manager,
        space_manager,
        view,
        &window_list,
        display_manager,
        mouse_drag_state,
    );

    //
    // @cleanup
    //
    // :AXBatching
    //
    // NOTE(asmvik): Flush previously batched operations if the view is marked as dirty.
    // This is necessary to make sure that we do not call the AX API for each modification to the tree.
    //

    if space_is_visible(view) && view_is_dirty(space_manager, view) {
        window_node_flush(view, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&view) {
            view.clear_flag(ViewFlag::IS_DIRTY);
        }
    }
}

pub(crate) fn window_manager_correct_for_mission_control_changes(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let display_list = display_manager_active_display_list();

    let animation_duration = window_manager.window_animation_duration;
    window_manager.window_animation_duration = 0.0f32;

    for index in 0..display_list.len() {
        let display_id = display_list[index];

        let Some(space_list) = display_space_list(display_id) else {
            continue;
        };

        let space_id = display_space_id(display_id);
        for inner_index in 0..space_list.len() {
            if space_list[inner_index] == space_id {
                window_manager_validate_and_check_for_windows_on_space(
                    space_manager,
                    window_manager,
                    space_id,
                    display_manager,
                    mouse_drag_state,
                );
            } else {
                space_manager_mark_view_invalid(
                    space_manager,
                    space_list[inner_index],
                    display_manager,
                    window_manager,
                );
            }
        }
    }

    window_manager.window_animation_duration = animation_duration;
}

pub(crate) fn window_manager_handle_display_add_and_remove(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some(space_list) = display_space_list(display_id) else {
        return;
    };

    for index in 0..space_list.len() {
        if space_is_user(space_list[index]) {
            let window_list = space_window_list(space_list[index], false, window_manager);
            if let Some(window_list) = window_list {
                let view = space_manager_find_view(
                    space_manager,
                    space_list[index],
                    display_manager,
                    window_manager,
                );
                if space_manager
                    .view
                    .find(&view)
                    .is_some_and(|view| view.layout != ViewType::Float)
                {
                    window_manager_check_for_windows_on_space(
                        window_manager,
                        space_manager,
                        view,
                        &window_list,
                        display_manager,
                        mouse_drag_state,
                    );
                }
            }
            break;
        }
    }

    let space_id = display_space_id(display_id);
    for index in 0..space_list.len() {
        if space_list[index] == space_id {
            space_manager_refresh_view(space_manager, space_id, display_manager, window_manager);
        } else {
            space_manager_mark_view_invalid(
                space_manager,
                space_list[index],
                display_manager,
                window_manager,
            );
        }
    }
}
