use crate::display::manager::DisplayManager;
use crate::layout::settings::{ViewFlag, ViewType};
use crate::layout::tree::WindowNodeSplit;
use crate::layout::view::{view_clear, view_flush, view_update};
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::space_is_user;
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::support::arithmetic::add_and_clamp_to_zero;
use crate::support::handles::SpaceId;
use crate::support::type_of_change::{TYPE_ABS, TYPE_REL};
use crate::window::manager::WindowManager;
use crate::window::space_reconciliation::window_manager_validate_and_check_for_windows_on_space;

pub(crate) fn space_manager_set_layout_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    view_type: ViewType,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    view.layout = view_type;
    view_clear(
        space_manager,
        space_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    );

    if space_manager
        .view
        .find(&space_id)
        .is_some_and(|view| view.layout != ViewType::Float)
    {
        window_manager_validate_and_check_for_windows_on_space(
            space_manager,
            window_manager,
            space_id,
            display_manager,
            mouse_drag_state,
        );
    }
}

pub(crate) fn space_manager_set_gap_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    type_of_change: i32,
    gap: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return false;
    };
    if view.layout == ViewType::Float {
        return false;
    }

    if type_of_change == TYPE_ABS {
        view.window_gap = gap;
    } else if type_of_change == TYPE_REL {
        view.window_gap = add_and_clamp_to_zero(view.window_gap, gap);
    }

    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_toggle_gap_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return false;
    };
    if view.layout == ViewType::Float {
        return false;
    }

    if view.check_flag(ViewFlag::ENABLE_GAP) {
        view.clear_flag(ViewFlag::ENABLE_GAP);
    } else {
        view.set_flag(ViewFlag::ENABLE_GAP);
    }

    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_set_layout_for_all_spaces(
    space_manager: &mut SpaceManager,
    layout: ViewType,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    space_manager.layout = layout;
    for space_id in space_manager.view.keys_in_bucket_order() {
        let Some(view) = space_manager.view.find(&space_id) else {
            continue;
        };
        if !view.check_flag(ViewFlag::LAYOUT) {
            if space_is_user(space_id) {
                let Some(view) = space_manager.view.find_mut(&space_id) else {
                    continue;
                };
                view.layout = layout;
                view_clear(
                    space_manager,
                    space_id,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );

                if space_manager
                    .view
                    .find(&space_id)
                    .is_some_and(|view| view.layout != ViewType::Float)
                {
                    window_manager_validate_and_check_for_windows_on_space(
                        space_manager,
                        window_manager,
                        space_id,
                        display_manager,
                        mouse_drag_state,
                    );
                }
            }
        }
    }
}

pub(crate) fn space_manager_set_window_gap_for_all_spaces(
    space_manager: &mut SpaceManager,
    window_gap: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.window_gap = window_gap;
    for space_id in space_manager.view.keys_in_bucket_order() {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            continue;
        };
        if !view.check_flag(ViewFlag::WINDOW_GAP) {
            view.window_gap = window_gap;
            view_update(space_manager, space_id, display_manager, window_manager);
            view_flush(space_manager, space_id, window_manager);
        }
    }
}

pub(crate) fn space_manager_set_top_padding_for_all_spaces(
    space_manager: &mut SpaceManager,
    top_padding: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.top_padding = top_padding;
    for space_id in space_manager.view.keys_in_bucket_order() {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            continue;
        };
        if !view.check_flag(ViewFlag::TOP_PADDING) {
            view.top_padding = top_padding;
            view_update(space_manager, space_id, display_manager, window_manager);
            view_flush(space_manager, space_id, window_manager);
        }
    }
}

pub(crate) fn space_manager_set_bottom_padding_for_all_spaces(
    space_manager: &mut SpaceManager,
    bottom_padding: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.bottom_padding = bottom_padding;
    for space_id in space_manager.view.keys_in_bucket_order() {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            continue;
        };
        if !view.check_flag(ViewFlag::BOTTOM_PADDING) {
            view.bottom_padding = bottom_padding;
            view_update(space_manager, space_id, display_manager, window_manager);
            view_flush(space_manager, space_id, window_manager);
        }
    }
}

pub(crate) fn space_manager_set_left_padding_for_all_spaces(
    space_manager: &mut SpaceManager,
    left_padding: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.left_padding = left_padding;
    for space_id in space_manager.view.keys_in_bucket_order() {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            continue;
        };
        if !view.check_flag(ViewFlag::LEFT_PADDING) {
            view.left_padding = left_padding;
            view_update(space_manager, space_id, display_manager, window_manager);
            view_flush(space_manager, space_id, window_manager);
        }
    }
}

pub(crate) fn space_manager_set_right_padding_for_all_spaces(
    space_manager: &mut SpaceManager,
    right_padding: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.right_padding = right_padding;
    for space_id in space_manager.view.keys_in_bucket_order() {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            continue;
        };
        if !view.check_flag(ViewFlag::RIGHT_PADDING) {
            view.right_padding = right_padding;
            view_update(space_manager, space_id, display_manager, window_manager);
            view_flush(space_manager, space_id, window_manager);
        }
    }
}

pub(crate) fn space_manager_set_split_type_for_all_spaces(
    space_manager: &mut SpaceManager,
    split_type: WindowNodeSplit,
) {
    space_manager.split_type = split_type;
    for view in space_manager.view.values_mut() {
        if !view.check_flag(ViewFlag::SPLIT_TYPE) {
            view.split_type = split_type;
        }
    }
}

pub(crate) fn space_manager_set_auto_balance_for_all_spaces(
    space_manager: &mut SpaceManager,
    auto_balance: u32,
) {
    space_manager.auto_balance = auto_balance;
    for view in space_manager.view.values_mut() {
        if !view.check_flag(ViewFlag::AUTO_BALANCE) {
            view.auto_balance = auto_balance;
        }
    }
}

pub(crate) fn space_manager_set_padding_for_space(
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
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return false;
    };
    if view.layout == ViewType::Float {
        return false;
    }

    if type_of_change == TYPE_ABS {
        view.top_padding = top;
        view.bottom_padding = bottom;
        view.left_padding = left;
        view.right_padding = right;
    } else if type_of_change == TYPE_REL {
        view.top_padding = add_and_clamp_to_zero(view.top_padding, top);
        view.bottom_padding = add_and_clamp_to_zero(view.bottom_padding, bottom);
        view.left_padding = add_and_clamp_to_zero(view.left_padding, left);
        view.right_padding = add_and_clamp_to_zero(view.right_padding, right);
    }

    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_toggle_padding_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return false;
    };
    if view.layout == ViewType::Float {
        return false;
    }

    if view.check_flag(ViewFlag::ENABLE_PADDING) {
        view.clear_flag(ViewFlag::ENABLE_PADDING);
    } else {
        view.set_flag(ViewFlag::ENABLE_PADDING);
    }

    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}
