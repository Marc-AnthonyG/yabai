use crate::display::manager::DisplayManager;
use crate::layout::group::remember_the_groups_of_view_as_it_leaves_bsp;
use crate::layout::settings::{ViewFlag, ViewLayout};
use crate::layout::tree::WindowNodeSplit;
use crate::layout::view::{
    clear_view_tree_unmanaging_every_window,
    move_view_windows_into_their_areas_or_defer_until_space_is_visible,
    recompute_view_areas_from_display_bounds_and_padding,
};
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::is_user_space;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::support::arithmetic::add_and_clamp_to_zero;
use crate::support::handles::SpaceId;
use crate::support::type_of_change::{CHANGE_TYPE_ABSOLUTE, CHANGE_TYPE_RELATIVE};
use crate::window::focus_follows_mouse::schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles;
use crate::window::manager::WindowManager;
use crate::window::space_reconciliation::reconcile_space_view_with_windows_on_space;

pub(crate) fn set_layout_of_space_retiling_its_windows(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    view_layout: ViewLayout,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    retile_view_in_a_different_layout_remembering_its_groups(
        space_manager,
        space_id,
        view_layout,
        display_manager,
        window_manager,
        mouse_drag_state,
    );
}

fn retile_view_in_a_different_layout_remembering_its_groups(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    view_layout: ViewLayout,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some(previous_layout) = space_manager.view.get(&space_id).map(|view| view.layout) else {
        return;
    };
    if previous_layout == view_layout {
        return;
    }
    if previous_layout == ViewLayout::BinarySpacePartitioning {
        remember_the_groups_of_view_as_it_leaves_bsp(space_manager, space_id);
    }

    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return;
    };
    view.layout = view_layout;
    clear_view_tree_unmanaging_every_window(
        space_manager,
        space_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    );

    if view_layout != ViewLayout::Float {
        reconcile_space_view_with_windows_on_space(
            space_manager,
            window_manager,
            space_id,
            display_manager,
            mouse_drag_state,
        );
    }
    schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles(window_manager);
}

pub(crate) fn set_window_gap_of_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    type_of_change: i32,
    gap: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return false;
    };
    if view.layout == ViewLayout::Float {
        return false;
    }

    if type_of_change == CHANGE_TYPE_ABSOLUTE {
        view.window_gap = gap;
    } else if type_of_change == CHANGE_TYPE_RELATIVE {
        view.window_gap = add_and_clamp_to_zero(view.window_gap, gap);
    }

    recompute_view_areas_from_display_bounds_and_padding(
        space_manager,
        space_id,
        display_manager,
        window_manager,
    );
    move_view_windows_into_their_areas_or_defer_until_space_is_visible(
        space_manager,
        space_id,
        window_manager,
    );

    true
}

pub(crate) fn toggle_window_gap_of_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return false;
    };
    if view.layout == ViewLayout::Float {
        return false;
    }

    if view.has_flag(ViewFlag::WINDOW_GAP_IS_ENABLED) {
        view.clear_flag(ViewFlag::WINDOW_GAP_IS_ENABLED);
    } else {
        view.set_flag(ViewFlag::WINDOW_GAP_IS_ENABLED);
    }

    recompute_view_areas_from_display_bounds_and_padding(
        space_manager,
        space_id,
        display_manager,
        window_manager,
    );
    move_view_windows_into_their_areas_or_defer_until_space_is_visible(
        space_manager,
        space_id,
        window_manager,
    );

    true
}

pub(crate) fn set_global_layout_applying_it_to_views_without_their_own(
    space_manager: &mut SpaceManager,
    layout: ViewLayout,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    space_manager.layout = layout;
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        let Some(view) = space_manager.view.get(&space_id) else {
            continue;
        };
        if !view.has_flag(ViewFlag::OVERRIDES_GLOBAL_LAYOUT) && is_user_space(space_id) {
            retile_view_in_a_different_layout_remembering_its_groups(
                space_manager,
                space_id,
                layout,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
        }
    }
}

pub(crate) fn set_global_window_gap_applying_it_to_views_without_their_own(
    space_manager: &mut SpaceManager,
    window_gap: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.window_gap = window_gap;
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            continue;
        };
        if !view.has_flag(ViewFlag::OVERRIDES_GLOBAL_WINDOW_GAP) {
            view.window_gap = window_gap;
            recompute_view_areas_from_display_bounds_and_padding(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            );
            move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                space_manager,
                space_id,
                window_manager,
            );
        }
    }
}

pub(crate) fn set_global_top_padding_applying_it_to_views_without_their_own(
    space_manager: &mut SpaceManager,
    top_padding: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.top_padding = top_padding;
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            continue;
        };
        if !view.has_flag(ViewFlag::OVERRIDES_GLOBAL_TOP_PADDING) {
            view.top_padding = top_padding;
            recompute_view_areas_from_display_bounds_and_padding(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            );
            move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                space_manager,
                space_id,
                window_manager,
            );
        }
    }
}

pub(crate) fn set_global_bottom_padding_applying_it_to_views_without_their_own(
    space_manager: &mut SpaceManager,
    bottom_padding: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.bottom_padding = bottom_padding;
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            continue;
        };
        if !view.has_flag(ViewFlag::OVERRIDES_GLOBAL_BOTTOM_PADDING) {
            view.bottom_padding = bottom_padding;
            recompute_view_areas_from_display_bounds_and_padding(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            );
            move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                space_manager,
                space_id,
                window_manager,
            );
        }
    }
}

pub(crate) fn set_global_left_padding_applying_it_to_views_without_their_own(
    space_manager: &mut SpaceManager,
    left_padding: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.left_padding = left_padding;
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            continue;
        };
        if !view.has_flag(ViewFlag::OVERRIDES_GLOBAL_LEFT_PADDING) {
            view.left_padding = left_padding;
            recompute_view_areas_from_display_bounds_and_padding(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            );
            move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                space_manager,
                space_id,
                window_manager,
            );
        }
    }
}

pub(crate) fn set_global_right_padding_applying_it_to_views_without_their_own(
    space_manager: &mut SpaceManager,
    right_padding: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.right_padding = right_padding;
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            continue;
        };
        if !view.has_flag(ViewFlag::OVERRIDES_GLOBAL_RIGHT_PADDING) {
            view.right_padding = right_padding;
            recompute_view_areas_from_display_bounds_and_padding(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            );
            move_view_windows_into_their_areas_or_defer_until_space_is_visible(
                space_manager,
                space_id,
                window_manager,
            );
        }
    }
}

pub(crate) fn set_global_split_type_applying_it_to_views_without_their_own(
    space_manager: &mut SpaceManager,
    split_type: WindowNodeSplit,
) {
    space_manager.split_type = split_type;
    for view in space_manager.view.values_mut() {
        if !view.has_flag(ViewFlag::OVERRIDES_GLOBAL_SPLIT_TYPE) {
            view.split_type = split_type;
        }
    }
}

pub(crate) fn set_global_auto_balance_applying_it_to_views_without_their_own(
    space_manager: &mut SpaceManager,
    auto_balance: u32,
) {
    space_manager.auto_balance = auto_balance;
    for view in space_manager.view.values_mut() {
        if !view.has_flag(ViewFlag::OVERRIDES_GLOBAL_AUTO_BALANCE) {
            view.auto_balance = auto_balance;
        }
    }
}

pub(crate) fn set_padding_of_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    type_of_change: i32,
    top: i32,
    bottom: i32,
    left: i32,
    right: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return false;
    };
    if view.layout == ViewLayout::Float {
        return false;
    }

    if type_of_change == CHANGE_TYPE_ABSOLUTE {
        view.top_padding = top;
        view.bottom_padding = bottom;
        view.left_padding = left;
        view.right_padding = right;
    } else if type_of_change == CHANGE_TYPE_RELATIVE {
        view.top_padding = add_and_clamp_to_zero(view.top_padding, top);
        view.bottom_padding = add_and_clamp_to_zero(view.bottom_padding, bottom);
        view.left_padding = add_and_clamp_to_zero(view.left_padding, left);
        view.right_padding = add_and_clamp_to_zero(view.right_padding, right);
    }

    recompute_view_areas_from_display_bounds_and_padding(
        space_manager,
        space_id,
        display_manager,
        window_manager,
    );
    move_view_windows_into_their_areas_or_defer_until_space_is_visible(
        space_manager,
        space_id,
        window_manager,
    );

    true
}

pub(crate) fn toggle_padding_of_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return false;
    };
    if view.layout == ViewLayout::Float {
        return false;
    }

    if view.has_flag(ViewFlag::PADDING_IS_ENABLED) {
        view.clear_flag(ViewFlag::PADDING_IS_ENABLED);
    } else {
        view.set_flag(ViewFlag::PADDING_IS_ENABLED);
    }

    recompute_view_areas_from_display_bounds_and_padding(
        space_manager,
        space_id,
        display_manager,
        window_manager,
    );
    move_view_windows_into_their_areas_or_defer_until_space_is_visible(
        space_manager,
        space_id,
        window_manager,
    );

    true
}
