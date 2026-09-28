use crate::command::config::SettingsASpaceCanOverride;
use crate::display::manager::DisplayManager;
use crate::layout::settings::{ViewFlag, ViewLayout};
use crate::layout::view::{
    View, clear_view_tree_unmanaging_every_window,
    move_view_windows_into_their_areas_or_defer_until_space_is_visible,
    recompute_view_areas_from_display_bounds_and_padding,
};
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::is_user_space;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::space::view_settings::{
    set_global_auto_balance_applying_it_to_views_without_their_own,
    set_global_bottom_padding_applying_it_to_views_without_their_own,
    set_global_layout_applying_it_to_views_without_their_own,
    set_global_left_padding_applying_it_to_views_without_their_own,
    set_global_right_padding_applying_it_to_views_without_their_own,
    set_global_split_type_applying_it_to_views_without_their_own,
    set_global_top_padding_applying_it_to_views_without_their_own,
    set_global_window_gap_applying_it_to_views_without_their_own,
};
use crate::support::handles::SpaceId;
use crate::window::manager::WindowManager;
use crate::window::space_reconciliation::reconcile_space_view_with_windows_on_space;

pub(crate) fn override_the_settings_of_space(
    space_id: SpaceId,
    settings: &SettingsASpaceCanOverride,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> Result<(), String> {
    if settings.layout.is_some() && !is_user_space(space_id) {
        return Err(String::from(
            "cannot set layout for a macOS fullscreen space!",
        ));
    }

    let view_space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    if let Some(layout) = settings.layout {
        override_the_layout_of_space_rebuilding_its_tree(
            space_id,
            view_space_id,
            layout,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    }
    if let Some(view) = space_manager.view.get_mut(&view_space_id) {
        override_the_split_padding_gap_and_balance_of_view(view, settings);
    }
    if changes_the_area_windows_get(settings) {
        recompute_view_areas_from_display_bounds_and_padding(
            space_manager,
            view_space_id,
            display_manager,
            window_manager,
        );
        move_view_windows_into_their_areas_or_defer_until_space_is_visible(
            space_manager,
            view_space_id,
            window_manager,
        );
    }
    Ok(())
}

fn override_the_layout_of_space_rebuilding_its_tree(
    space_id: SpaceId,
    view_space_id: SpaceId,
    layout: ViewLayout,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if let Some(view) = space_manager.view.get_mut(&view_space_id) {
        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_LAYOUT);
        view.layout = layout;
    }
    clear_view_tree_unmanaging_every_window(
        space_manager,
        view_space_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    );
    if layout != ViewLayout::Float {
        reconcile_space_view_with_windows_on_space(
            space_manager,
            window_manager,
            space_id,
            display_manager,
            mouse_drag_state,
        );
    }
}

fn override_the_split_padding_gap_and_balance_of_view(
    view: &mut View,
    settings: &SettingsASpaceCanOverride,
) {
    if let Some(split_type) = settings.split_type {
        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_SPLIT_TYPE);
        view.split_type = split_type;
    }
    if let Some(top_padding) = settings.top_padding {
        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_TOP_PADDING);
        view.top_padding = points_as_a_view_stores_them(top_padding);
    }
    if let Some(bottom_padding) = settings.bottom_padding {
        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_BOTTOM_PADDING);
        view.bottom_padding = points_as_a_view_stores_them(bottom_padding);
    }
    if let Some(left_padding) = settings.left_padding {
        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_LEFT_PADDING);
        view.left_padding = points_as_a_view_stores_them(left_padding);
    }
    if let Some(right_padding) = settings.right_padding {
        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_RIGHT_PADDING);
        view.right_padding = points_as_a_view_stores_them(right_padding);
    }
    if let Some(window_gap) = settings.window_gap {
        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_WINDOW_GAP);
        view.window_gap = points_as_a_view_stores_them(window_gap);
    }
    if let Some(auto_balance) = settings.auto_balance {
        view.flags.insert(ViewFlag::OVERRIDES_GLOBAL_AUTO_BALANCE);
        view.auto_balance = auto_balance.split_axes_it_balances();
    }
}

fn changes_the_area_windows_get(settings: &SettingsASpaceCanOverride) -> bool {
    [
        settings.top_padding,
        settings.bottom_padding,
        settings.left_padding,
        settings.right_padding,
        settings.window_gap,
    ]
    .iter()
    .any(Option::is_some)
}

pub(crate) fn change_the_global_settings_a_space_can_override(
    settings: &SettingsASpaceCanOverride,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if let Some(layout) = settings.layout {
        set_global_layout_applying_it_to_views_without_their_own(
            space_manager,
            layout,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
    }
    if let Some(split_type) = settings.split_type {
        set_global_split_type_applying_it_to_views_without_their_own(space_manager, split_type);
    }
    if let Some(top_padding) = settings.top_padding {
        set_global_top_padding_applying_it_to_views_without_their_own(
            space_manager,
            points_as_a_view_stores_them(top_padding),
            display_manager,
            window_manager,
        );
    }
    if let Some(bottom_padding) = settings.bottom_padding {
        set_global_bottom_padding_applying_it_to_views_without_their_own(
            space_manager,
            points_as_a_view_stores_them(bottom_padding),
            display_manager,
            window_manager,
        );
    }
    if let Some(left_padding) = settings.left_padding {
        set_global_left_padding_applying_it_to_views_without_their_own(
            space_manager,
            points_as_a_view_stores_them(left_padding),
            display_manager,
            window_manager,
        );
    }
    if let Some(right_padding) = settings.right_padding {
        set_global_right_padding_applying_it_to_views_without_their_own(
            space_manager,
            points_as_a_view_stores_them(right_padding),
            display_manager,
            window_manager,
        );
    }
    if let Some(window_gap) = settings.window_gap {
        set_global_window_gap_applying_it_to_views_without_their_own(
            space_manager,
            points_as_a_view_stores_them(window_gap),
            display_manager,
            window_manager,
        );
    }
    if let Some(auto_balance) = settings.auto_balance {
        set_global_auto_balance_applying_it_to_views_without_their_own(
            space_manager,
            auto_balance.split_axes_it_balances(),
        );
    }
}

fn points_as_a_view_stores_them(points: u32) -> i32 {
    i32::try_from(points).unwrap_or(i32::MAX)
}
