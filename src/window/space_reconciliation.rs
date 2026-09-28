use crate::display::identity::query_displays_active_for_drawing;
use crate::display::manager::DisplayManager;
use crate::display::spaces::{query_current_space_of_display, query_spaces_of_display};
use crate::layout::group::{
    RejoinedGroup, add_window_to_view_tree_rejoining_its_remembered_group,
    put_the_rejoined_groups_back_in_order_and_forget_them,
};
use crate::layout::settings::{ViewFlag, ViewLayout};
use crate::layout::tree::{
    collect_windows_of_view_in_tree_order, move_windows_below_node_into_their_areas,
    remove_window_from_view_tree,
};
use crate::layout::view::has_view_windows_awaiting_their_areas;
use crate::mouse::drag::MouseDragState;
use crate::scripting_addition::client::order_window_relative_to_other_window_through_scripting_addition;
use crate::space::managed_space::{
    is_space_visible_on_its_display, is_user_space, query_windows_on_space,
};
use crate::space::manager::{
    SpaceManager, find_or_create_view_for_space, mark_view_areas_out_of_date,
    recompute_view_areas_and_move_its_windows_into_them,
};
use crate::support::handles::{DisplayId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::support::layer::{LAYER_BELOW, LAYER_NORMAL};
use crate::window::layer::set_window_layer_unless_explicitly_set;
use crate::window::manager::{
    WindowManager, forget_managed_window, record_managed_window_on_space_updating_its_shadow,
    should_window_be_managed, space_managing_window, tracked_window_with_id,
};
use crate::window::shadow::apply_shadow_removal_mode_to_window;

pub(crate) fn untile_windows_no_longer_on_space(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_list: &[WindowId],
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let view_window_list = collect_windows_of_view_in_tree_order(space_manager, space_id);

    for index in 0..view_window_list.len() {
        let mut found = false;

        for inner_index in 0..window_list.len() {
            if view_window_list[index] == window_list[inner_index] {
                found = true;
                break;
            }
        }

        if !found {
            let Some(window) = tracked_window_with_id(window_manager, view_window_list[index])
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

            remove_window_from_view_tree(
                space_manager,
                space_id,
                window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            set_window_layer_unless_explicitly_set(window, LAYER_NORMAL, window_manager);
            forget_managed_window(window_manager, window);
            apply_shadow_removal_mode_to_window(window_manager, window);

            if let Some(view) = space_manager.view.find_mut(&space_id) {
                view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
            }
        }
    }
}

pub(crate) fn tile_manageable_windows_found_on_space(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_list: &[WindowId],
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    for index in 0..window_list.len() {
        let window = tracked_window_with_id(window_manager, window_list[index]);
        let Some(window) = window else {
            continue;
        };
        if !should_window_be_managed(window, window_manager) {
            continue;
        }

        let existing_view = space_managing_window(window_manager, window);
        let existing_view_layout = existing_view
            .and_then(|existing_space_id| space_manager.view.find(&existing_space_id))
            .map(|view| view.layout);
        if let Some(existing_space_id) = existing_view
            && existing_view_layout != Some(ViewLayout::Float)
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

            remove_window_from_view_tree(
                space_manager,
                existing_space_id,
                window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            set_window_layer_unless_explicitly_set(window, LAYER_NORMAL, window_manager);
            forget_managed_window(window_manager, window);
            apply_shadow_removal_mode_to_window(window_manager, window);
            if let Some(view) = space_manager.view.find_mut(&existing_space_id) {
                view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
            }
        }

        if existing_view.is_none()
            || (existing_view_layout != Some(ViewLayout::Float) && existing_view != Some(space_id))
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

            add_window_to_view_tree_rejoining_its_remembered_group(
                space_manager,
                space_id,
                window,
                display_manager,
                window_manager,
            );
            set_window_layer_unless_explicitly_set(window, LAYER_BELOW, window_manager);
            record_managed_window_on_space_updating_its_shadow(
                window_manager,
                window,
                space_manager,
                space_id,
            );
            if let Some(view) = space_manager.view.find_mut(&space_id) {
                view.set_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
            }
        }
    }

    bring_the_front_window_of_each_rejoined_group_above_the_others(
        put_the_rejoined_groups_back_in_order_and_forget_them(
            space_manager,
            space_id,
            window_manager,
        ),
    );
}

fn bring_the_front_window_of_each_rejoined_group_above_the_others(
    rejoined_groups: Vec<RejoinedGroup>,
) {
    for rejoined_group in rejoined_groups {
        for other_member in rejoined_group.other_members {
            order_window_relative_to_other_window_through_scripting_addition(
                rejoined_group.front_window,
                1,
                other_member,
            );
        }
    }
}

pub(crate) fn reconcile_space_view_with_windows_on_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let view =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    if space_manager
        .view
        .find(&view)
        .is_none_or(|view| view.layout == ViewLayout::Float)
    {
        return;
    }

    let window_list = query_windows_on_space(space_id, false, window_manager).unwrap_or_default();
    untile_windows_no_longer_on_space(
        window_manager,
        space_manager,
        view,
        &window_list,
        display_manager,
        mouse_drag_state,
    );
    tile_manageable_windows_found_on_space(
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

    if is_space_visible_on_its_display(view)
        && has_view_windows_awaiting_their_areas(space_manager, view)
    {
        move_windows_below_node_into_their_areas(view, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&view) {
            view.clear_flag(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
        }
    }
}

pub(crate) fn reconcile_every_view_after_mission_control_changes(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let display_list = query_displays_active_for_drawing();

    let animation_duration = window_manager.window_animation_duration;
    window_manager.window_animation_duration = 0.0f32;

    for index in 0..display_list.len() {
        let display_id = display_list[index];

        let Some(space_list) = query_spaces_of_display(display_id) else {
            continue;
        };

        let space_id = query_current_space_of_display(display_id);
        for inner_index in 0..space_list.len() {
            if space_list[inner_index] == space_id {
                reconcile_space_view_with_windows_on_space(
                    space_manager,
                    window_manager,
                    space_id,
                    display_manager,
                    mouse_drag_state,
                );
            } else {
                mark_view_areas_out_of_date(
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

pub(crate) fn reconcile_views_after_display_added_or_removed(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some(space_list) = query_spaces_of_display(display_id) else {
        return;
    };

    for index in 0..space_list.len() {
        if is_user_space(space_list[index]) {
            let window_list = query_windows_on_space(space_list[index], false, window_manager);
            if let Some(window_list) = window_list {
                let view = find_or_create_view_for_space(
                    space_manager,
                    space_list[index],
                    display_manager,
                    window_manager,
                );
                if space_manager
                    .view
                    .find(&view)
                    .is_some_and(|view| view.layout != ViewLayout::Float)
                {
                    tile_manageable_windows_found_on_space(
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

    let space_id = query_current_space_of_display(display_id);
    for index in 0..space_list.len() {
        if space_list[index] == space_id {
            recompute_view_areas_and_move_its_windows_into_them(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            );
        } else {
            mark_view_areas_out_of_date(
                space_manager,
                space_list[index],
                display_manager,
                window_manager,
            );
        }
    }
}
