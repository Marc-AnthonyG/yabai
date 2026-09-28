use std::sync::atomic::{Ordering, compiler_fence};

use crate::display::manager::DisplayManager;
use crate::display::spaces::{
    is_display_animating_a_space_transition, query_current_space_of_display,
};
use crate::layout::view::{
    move_view_windows_into_their_areas_or_defer_until_space_is_visible,
    recompute_view_areas_from_display_bounds_and_padding,
};
use crate::mouse::drag::MouseDragState;
use crate::scripting_addition::client::{
    create_space_on_display_of_space_through_scripting_addition,
    destroy_space_through_scripting_addition, move_space_after_space_through_scripting_addition,
    move_space_to_display_through_scripting_addition,
};
use crate::space::focus::{
    focus_space_through_the_scripting_addition_or_dock_swipes,
    query_current_space_of_the_focused_display,
};
use crate::space::lookup::{
    is_space_the_last_user_space_of_its_display, query_first_user_space_of_display,
    query_mission_control_index_of_space, query_previous_space_in_mission_control_order,
};
use crate::space::managed_space::{
    is_user_space, query_display_holding_space, query_windows_on_space,
};
use crate::space::manager::{
    SpaceManager, mark_view_areas_out_of_date, point_view_handles_at_rekeyed_views,
};
use crate::space::moving_windows::move_windows_to_space_by_whichever_mechanism_this_macos_supports;
use crate::state::mission_control_mode::{MissionControlMode, is_mission_control_active};
use crate::support::handles::{DisplayId, SpaceId};
use crate::window::manager::WindowManager;
use crate::window::space_reconciliation::reconcile_space_view_with_windows_on_space;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpaceOperationOutcome {
    Success = 0,
    MissingSource = 1,
    MissingDestination = 2,
    InvalidSource = 3,
    InvalidDestination = 4,
    NotAUserSpace = 5,
    SameSpace = 6,
    NotOnTheSameDisplay = 7,
    DisplayIsAnimating = 8,
    MissionControlIsActive = 9,
    ScriptingAdditionFailed = 10,
}

pub(crate) fn swap_spaces_across_displays_by_exchanging_their_windows(
    a_display_id: DisplayId,
    a_space_id: SpaceId,
    b_display_id: DisplayId,
    b_space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> SpaceOperationOutcome {
    if is_display_animating_a_space_transition(a_display_id) {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }
    if is_display_animating_a_space_transition(b_display_id) {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    let window_animation_duration = window_manager.window_animation_duration;
    window_manager.window_animation_duration = 0.0f32;
    compiler_fence(Ordering::SeqCst);

    let a_window_list =
        query_windows_on_space(a_space_id, true, window_manager).unwrap_or_default();

    let b_window_list =
        query_windows_on_space(b_space_id, true, window_manager).unwrap_or_default();

    let Some(mut a_view) = space_manager.view.remove(&a_space_id) else {
        compiler_fence(Ordering::SeqCst);
        window_manager.window_animation_duration = window_animation_duration;
        return SpaceOperationOutcome::InvalidSource;
    };
    let Some(mut b_view) = space_manager.view.remove(&b_space_id) else {
        space_manager.view.insert(a_space_id, a_view);
        compiler_fence(Ordering::SeqCst);
        window_manager.window_animation_duration = window_animation_duration;
        return SpaceOperationOutcome::InvalidDestination;
    };

    a_view.space_id = b_space_id;
    b_view.space_id = a_space_id;

    std::mem::swap(&mut a_view.uuid, &mut b_view.uuid);

    space_manager.view.insert(a_space_id, b_view);
    space_manager.view.insert(b_space_id, a_view);

    point_view_handles_at_rekeyed_views(window_manager, mouse_drag_state, |space_id| {
        if space_id == a_space_id {
            b_space_id
        } else if space_id == b_space_id {
            a_space_id
        } else {
            space_id
        }
    });

    if !a_window_list.is_empty() {
        move_windows_to_space_by_whichever_mechanism_this_macos_supports(
            b_space_id,
            &a_window_list,
        );
    }

    if !b_window_list.is_empty() {
        move_windows_to_space_by_whichever_mechanism_this_macos_supports(
            a_space_id,
            &b_window_list,
        );
    }

    for label in space_manager.labels.iter_mut() {
        if label.space_id == a_space_id {
            label.space_id = b_space_id;
        } else if label.space_id == b_space_id {
            label.space_id = a_space_id;
        }
    }

    recompute_view_areas_from_display_bounds_and_padding(
        space_manager,
        b_space_id,
        display_manager,
        window_manager,
    );
    recompute_view_areas_from_display_bounds_and_padding(
        space_manager,
        a_space_id,
        display_manager,
        window_manager,
    );

    move_view_windows_into_their_areas_or_defer_until_space_is_visible(
        space_manager,
        b_space_id,
        window_manager,
    );
    move_view_windows_into_their_areas_or_defer_until_space_is_visible(
        space_manager,
        a_space_id,
        window_manager,
    );

    compiler_fence(Ordering::SeqCst);
    window_manager.window_animation_duration = window_animation_duration;
    SpaceOperationOutcome::Success
}

pub(crate) fn swap_space_with_space(
    acting_space_id: SpaceId,
    selector_space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mission_control_mode: &mut MissionControlMode,
    mouse_drag_state: &mut MouseDragState,
) -> SpaceOperationOutcome {
    let is_in_mission_control = is_mission_control_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOperationOutcome::MissionControlIsActive;
    }

    let acting_display_id = query_display_holding_space(acting_space_id);
    let selector_display_id = query_display_holding_space(selector_space_id);

    if acting_space_id == selector_space_id {
        return SpaceOperationOutcome::SameSpace;
    }
    if acting_display_id != selector_display_id {
        return swap_spaces_across_displays_by_exchanging_their_windows(
            acting_display_id,
            acting_space_id,
            selector_display_id,
            selector_space_id,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    }

    let is_animating = is_display_animating_a_space_transition(acting_display_id);
    if is_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    let acting_previous_space_id = query_previous_space_in_mission_control_order(acting_space_id);
    let selector_previous_space_id =
        query_previous_space_in_mission_control_order(selector_space_id);

    let acting_previous_display_id = if acting_previous_space_id != SpaceId(0) {
        query_display_holding_space(acting_previous_space_id)
    } else {
        DisplayId(0)
    };
    let selector_previous_display_id = if selector_previous_space_id != SpaceId(0) {
        query_display_holding_space(selector_previous_space_id)
    } else {
        DisplayId(0)
    };

    let acting_space_id_is_first =
        acting_previous_space_id == SpaceId(0) || acting_previous_display_id != acting_display_id;
    let selector_space_id_is_first = selector_previous_space_id == SpaceId(0)
        || selector_previous_display_id != selector_display_id;

    let acting_mission_control_index = query_mission_control_index_of_space(acting_space_id);
    let selector_mission_control_index = query_mission_control_index_of_space(selector_space_id);
    let mut success = true;

    if acting_space_id_is_first
        && !selector_space_id_is_first
        && selector_mission_control_index - acting_mission_control_index == 1
    {
        success = move_space_after_space_through_scripting_addition(
            acting_space_id,
            selector_space_id,
            acting_space_id == query_current_space_of_the_focused_display(window_manager),
        );
    } else if !acting_space_id_is_first
        && selector_space_id_is_first
        && acting_mission_control_index - selector_mission_control_index == 1
    {
        success = move_space_after_space_through_scripting_addition(
            selector_space_id,
            acting_space_id,
            selector_space_id == query_current_space_of_the_focused_display(window_manager),
        );
    } else if acting_space_id_is_first && !selector_space_id_is_first {
        success = move_space_after_space_through_scripting_addition(
            selector_space_id,
            acting_space_id,
            false,
        );
        success &= move_space_after_space_through_scripting_addition(
            acting_space_id,
            selector_previous_space_id,
            acting_space_id == query_current_space_of_the_focused_display(window_manager),
        );
    } else if !acting_space_id_is_first && selector_space_id_is_first {
        success = move_space_after_space_through_scripting_addition(
            acting_space_id,
            selector_space_id,
            acting_space_id == query_current_space_of_the_focused_display(window_manager),
        );
        success &= move_space_after_space_through_scripting_addition(
            selector_space_id,
            acting_previous_space_id,
            false,
        );
    } else if !acting_space_id_is_first && !selector_space_id_is_first {
        if acting_mission_control_index > selector_mission_control_index {
            success = move_space_after_space_through_scripting_addition(
                selector_space_id,
                acting_space_id,
                false,
            );
            success &= move_space_after_space_through_scripting_addition(
                acting_space_id,
                selector_previous_space_id,
                acting_space_id == query_current_space_of_the_focused_display(window_manager),
            );
        } else {
            success = move_space_after_space_through_scripting_addition(
                acting_space_id,
                selector_space_id,
                acting_space_id == query_current_space_of_the_focused_display(window_manager),
            );
            success &= move_space_after_space_through_scripting_addition(
                selector_space_id,
                acting_previous_space_id,
                false,
            );
        }
    }

    if success {
        SpaceOperationOutcome::Success
    } else {
        SpaceOperationOutcome::ScriptingAdditionFailed
    }
}

pub(crate) fn move_space_to_position_of_space(
    acting_space_id: SpaceId,
    selector_space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOperationOutcome {
    let is_in_mission_control = is_mission_control_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOperationOutcome::MissionControlIsActive;
    }

    let acting_display_id = query_display_holding_space(acting_space_id);
    let selector_display_id = query_display_holding_space(selector_space_id);

    if acting_space_id == selector_space_id {
        return SpaceOperationOutcome::SameSpace;
    }
    if acting_display_id != selector_display_id {
        return SpaceOperationOutcome::NotOnTheSameDisplay;
    }

    let is_animating = is_display_animating_a_space_transition(acting_display_id);
    if is_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    let acting_previous_space_id = query_previous_space_in_mission_control_order(acting_space_id);
    let selector_previous_space_id =
        query_previous_space_in_mission_control_order(selector_space_id);

    let acting_previous_display_id = if acting_previous_space_id != SpaceId(0) {
        query_display_holding_space(acting_previous_space_id)
    } else {
        DisplayId(0)
    };
    let selector_previous_display_id = if selector_previous_space_id != SpaceId(0) {
        query_display_holding_space(selector_previous_space_id)
    } else {
        DisplayId(0)
    };

    let acting_space_id_is_first =
        acting_previous_space_id == SpaceId(0) || acting_previous_display_id != acting_display_id;
    let selector_space_id_is_first = selector_previous_space_id == SpaceId(0)
        || selector_previous_display_id != selector_display_id;
    let mut success = true;

    if acting_space_id_is_first && !selector_space_id_is_first {
        success = move_space_after_space_through_scripting_addition(
            acting_space_id,
            selector_space_id,
            acting_space_id == query_current_space_of_the_focused_display(window_manager),
        );
    } else if !acting_space_id_is_first && selector_space_id_is_first {
        success = move_space_after_space_through_scripting_addition(
            acting_space_id,
            selector_space_id,
            acting_space_id == query_current_space_of_the_focused_display(window_manager),
        );
        success &= move_space_after_space_through_scripting_addition(
            selector_space_id,
            acting_space_id,
            false,
        );
    } else if !acting_space_id_is_first && !selector_space_id_is_first {
        if query_mission_control_index_of_space(acting_space_id)
            > query_mission_control_index_of_space(selector_space_id)
        {
            success = move_space_after_space_through_scripting_addition(
                acting_space_id,
                selector_previous_space_id,
                acting_space_id == query_current_space_of_the_focused_display(window_manager),
            );
        } else {
            success = move_space_after_space_through_scripting_addition(
                acting_space_id,
                selector_space_id,
                acting_space_id == query_current_space_of_the_focused_display(window_manager),
            );
        }
    }

    if success {
        SpaceOperationOutcome::Success
    } else {
        SpaceOperationOutcome::ScriptingAdditionFailed
    }
}

pub(crate) fn send_space_to_display(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOperationOutcome {
    let is_in_mission_control = is_mission_control_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOperationOutcome::MissionControlIsActive;
    }
    if space_id == SpaceId(0) {
        return SpaceOperationOutcome::MissingSource;
    }

    let source_display_id = query_display_holding_space(space_id);
    if source_display_id == display_id {
        return SpaceOperationOutcome::InvalidDestination;
    }

    let is_source_animating = is_display_animating_a_space_transition(source_display_id);
    if is_source_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    let last_space = is_space_the_last_user_space_of_its_display(space_id);
    if last_space {
        return SpaceOperationOutcome::InvalidSource;
    }

    let is_destination_animating = is_display_animating_a_space_transition(display_id);
    if is_destination_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    let destination_space_id = query_current_space_of_display(display_id);
    if destination_space_id == SpaceId(0) {
        return SpaceOperationOutcome::MissingDestination;
    }

    let focus_space = space_id == query_current_space_of_the_focused_display(window_manager);

    if move_space_to_display_through_scripting_addition(
        space_id,
        destination_space_id,
        if focus_space {
            query_previous_space_in_mission_control_order(space_id)
        } else {
            SpaceId(0)
        },
        focus_space,
    ) {
        mark_view_areas_out_of_date(space_manager, space_id, display_manager, window_manager);
        if focus_space {
            focus_space_through_the_scripting_addition_or_dock_swipes(
                space_id,
                window_manager,
                mission_control_mode,
            );
        }
        return SpaceOperationOutcome::Success;
    }

    SpaceOperationOutcome::ScriptingAdditionFailed
}

pub(crate) fn destroy_user_space_unless_it_is_the_last_of_its_display(
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOperationOutcome {
    let is_in_mission_control = is_mission_control_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOperationOutcome::MissionControlIsActive;
    }

    if space_id == SpaceId(0) {
        return SpaceOperationOutcome::MissingSource;
    }
    if !is_user_space(space_id) {
        return SpaceOperationOutcome::NotAUserSpace;
    }
    if is_space_the_last_user_space_of_its_display(space_id) {
        return SpaceOperationOutcome::InvalidSource;
    }

    let display_id = query_display_holding_space(space_id);
    let first_space_id = query_first_user_space_of_display(display_id);

    let is_animating = is_display_animating_a_space_transition(display_id);
    if is_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    let success = destroy_space_through_scripting_addition(space_id);
    if !success {
        return SpaceOperationOutcome::ScriptingAdditionFailed;
    }

    if first_space_id != SpaceId(0) {
        reconcile_space_view_with_windows_on_space(
            space_manager,
            window_manager,
            first_space_id,
            display_manager,
            mouse_drag_state,
        );
    }

    SpaceOperationOutcome::Success
}

pub(crate) fn add_space_on_display_of_space(
    space_id: SpaceId,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOperationOutcome {
    let is_in_mission_control = is_mission_control_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOperationOutcome::MissionControlIsActive;
    }
    if space_id == SpaceId(0) {
        return SpaceOperationOutcome::MissingSource;
    }

    let is_animating =
        is_display_animating_a_space_transition(query_display_holding_space(space_id));
    if is_animating {
        return SpaceOperationOutcome::DisplayIsAnimating;
    }

    if create_space_on_display_of_space_through_scripting_addition(space_id) {
        SpaceOperationOutcome::Success
    } else {
        SpaceOperationOutcome::ScriptingAdditionFailed
    }
}
