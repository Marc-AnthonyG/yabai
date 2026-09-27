use std::sync::atomic::{Ordering, compiler_fence};

use crate::display::manager::DisplayManager;
use crate::display::spaces::{display_manager_display_is_animating, display_space_id};
use crate::layout::view::{view_flush, view_update};
use crate::mouse::drag::MouseDragState;
use crate::scripting_addition::client::{
    scripting_addition_create_space, scripting_addition_destroy_space,
    scripting_addition_move_space_after_space, scripting_addition_move_space_to_display,
};
use crate::space::focus::{space_manager_active_space, space_manager_focus_space};
use crate::space::lookup::{
    space_manager_find_first_user_space_for_display, space_manager_is_space_last_user_space,
    space_manager_mission_control_index, space_manager_prev_space,
};
use crate::space::managed_space::{space_display_id, space_is_user, space_window_list};
use crate::space::manager::{
    SpaceManager, space_manager_mark_view_invalid,
    space_manager_point_view_handles_at_rekeyed_views,
};
use crate::space::moving_windows::space_manager_move_window_list_to_space;
use crate::state::mission_control_mode::{MissionControlMode, mission_control_is_active};
use crate::support::handles::{DisplayId, SpaceId};
use crate::window::manager::WindowManager;
use crate::window::space_reconciliation::window_manager_validate_and_check_for_windows_on_space;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpaceOpError {
    Success = 0,
    MissingSrc = 1,
    MissingDst = 2,
    InvalidSrc = 3,
    InvalidDst = 4,
    InvalidType = 5,
    SameSpace = 6,
    SameDisplay = 7,
    DisplayIsAnimating = 8,
    InMissionControl = 9,
    ScriptingAddition = 10,
}

pub(crate) fn space_manager_swap_space_with_space_on_display(
    a_display_id: DisplayId,
    a_space_id: SpaceId,
    b_display_id: DisplayId,
    b_space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> SpaceOpError {
    if display_manager_display_is_animating(a_display_id) {
        return SpaceOpError::DisplayIsAnimating;
    }
    if display_manager_display_is_animating(b_display_id) {
        return SpaceOpError::DisplayIsAnimating;
    }

    let window_animation_duration = window_manager.window_animation_duration;
    window_manager.window_animation_duration = 0.0f32;
    compiler_fence(Ordering::SeqCst);

    let a_window_list = space_window_list(a_space_id, true, window_manager).unwrap_or_default();

    let b_window_list = space_window_list(b_space_id, true, window_manager).unwrap_or_default();

    let Some(mut a_view) = space_manager.view.remove(&a_space_id) else {
        compiler_fence(Ordering::SeqCst);
        window_manager.window_animation_duration = window_animation_duration;
        return SpaceOpError::InvalidSrc;
    };
    let Some(mut b_view) = space_manager.view.remove(&b_space_id) else {
        space_manager.view.add(a_space_id, a_view);
        compiler_fence(Ordering::SeqCst);
        window_manager.window_animation_duration = window_animation_duration;
        return SpaceOpError::InvalidDst;
    };

    a_view.space_id = b_space_id;
    b_view.space_id = a_space_id;

    std::mem::swap(&mut a_view.uuid, &mut b_view.uuid);

    space_manager.view.add(a_space_id, b_view);
    space_manager.view.add(b_space_id, a_view);

    space_manager_point_view_handles_at_rekeyed_views(
        window_manager,
        mouse_drag_state,
        |space_id| {
            if space_id == a_space_id {
                b_space_id
            } else if space_id == b_space_id {
                a_space_id
            } else {
                space_id
            }
        },
    );

    if !a_window_list.is_empty() {
        space_manager_move_window_list_to_space(b_space_id, &a_window_list);
    }

    if !b_window_list.is_empty() {
        space_manager_move_window_list_to_space(a_space_id, &b_window_list);
    }

    for label in space_manager.labels.iter_mut() {
        if label.space_id == a_space_id {
            label.space_id = b_space_id;
        } else if label.space_id == b_space_id {
            label.space_id = a_space_id;
        }
    }

    view_update(space_manager, b_space_id, display_manager, window_manager);
    view_update(space_manager, a_space_id, display_manager, window_manager);

    view_flush(space_manager, b_space_id, window_manager);
    view_flush(space_manager, a_space_id, window_manager);

    compiler_fence(Ordering::SeqCst);
    window_manager.window_animation_duration = window_animation_duration;
    SpaceOpError::Success
}

pub(crate) fn space_manager_swap_space_with_space(
    acting_space_id: SpaceId,
    selector_space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mission_control_mode: &mut MissionControlMode,
    mouse_drag_state: &mut MouseDragState,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }

    let acting_display_id = space_display_id(acting_space_id);
    let selector_display_id = space_display_id(selector_space_id);

    if acting_space_id == selector_space_id {
        return SpaceOpError::SameSpace;
    }
    if acting_display_id != selector_display_id {
        return space_manager_swap_space_with_space_on_display(
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

    let is_animating = display_manager_display_is_animating(acting_display_id);
    if is_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    let acting_previous_space_id = space_manager_prev_space(acting_space_id);
    let selector_previous_space_id = space_manager_prev_space(selector_space_id);

    let acting_previous_display_id = if acting_previous_space_id != SpaceId(0) {
        space_display_id(acting_previous_space_id)
    } else {
        DisplayId(0)
    };
    let selector_previous_display_id = if selector_previous_space_id != SpaceId(0) {
        space_display_id(selector_previous_space_id)
    } else {
        DisplayId(0)
    };

    let acting_space_id_is_first =
        acting_previous_space_id == SpaceId(0) || acting_previous_display_id != acting_display_id;
    let selector_space_id_is_first = selector_previous_space_id == SpaceId(0)
        || selector_previous_display_id != selector_display_id;

    let acting_mission_control_index = space_manager_mission_control_index(acting_space_id);
    let selector_mission_control_index = space_manager_mission_control_index(selector_space_id);
    let mut success = true;

    if acting_space_id_is_first
        && !selector_space_id_is_first
        && selector_mission_control_index - acting_mission_control_index == 1
    {
        success = scripting_addition_move_space_after_space(
            acting_space_id,
            selector_space_id,
            acting_space_id == space_manager_active_space(window_manager),
        );
    } else if !acting_space_id_is_first
        && selector_space_id_is_first
        && acting_mission_control_index - selector_mission_control_index == 1
    {
        success = scripting_addition_move_space_after_space(
            selector_space_id,
            acting_space_id,
            selector_space_id == space_manager_active_space(window_manager),
        );
    } else if acting_space_id_is_first && !selector_space_id_is_first {
        success = scripting_addition_move_space_after_space(
            selector_space_id,
            acting_space_id,
            false,
        );
        success &= scripting_addition_move_space_after_space(
            acting_space_id,
            selector_previous_space_id,
            acting_space_id == space_manager_active_space(window_manager),
        );
    } else if !acting_space_id_is_first && selector_space_id_is_first {
        success = scripting_addition_move_space_after_space(
            acting_space_id,
            selector_space_id,
            acting_space_id == space_manager_active_space(window_manager),
        );
        success &= scripting_addition_move_space_after_space(
            selector_space_id,
            acting_previous_space_id,
            false,
        );
    } else if !acting_space_id_is_first && !selector_space_id_is_first {
        if acting_mission_control_index > selector_mission_control_index {
            success = scripting_addition_move_space_after_space(
                selector_space_id,
                acting_space_id,
                false,
            );
            success &= scripting_addition_move_space_after_space(
                acting_space_id,
                selector_previous_space_id,
                acting_space_id == space_manager_active_space(window_manager),
            );
        } else {
            success = scripting_addition_move_space_after_space(
                acting_space_id,
                selector_space_id,
                acting_space_id == space_manager_active_space(window_manager),
            );
            success &= scripting_addition_move_space_after_space(
                selector_space_id,
                acting_previous_space_id,
                false,
            );
        }
    }

    if success {
        SpaceOpError::Success
    } else {
        SpaceOpError::ScriptingAddition
    }
}

pub(crate) fn space_manager_move_space_to_space(
    acting_space_id: SpaceId,
    selector_space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }

    let acting_display_id = space_display_id(acting_space_id);
    let selector_display_id = space_display_id(selector_space_id);

    if acting_space_id == selector_space_id {
        return SpaceOpError::SameSpace;
    }
    if acting_display_id != selector_display_id {
        return SpaceOpError::SameDisplay;
    }

    let is_animating = display_manager_display_is_animating(acting_display_id);
    if is_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    let acting_previous_space_id = space_manager_prev_space(acting_space_id);
    let selector_previous_space_id = space_manager_prev_space(selector_space_id);

    let acting_previous_display_id = if acting_previous_space_id != SpaceId(0) {
        space_display_id(acting_previous_space_id)
    } else {
        DisplayId(0)
    };
    let selector_previous_display_id = if selector_previous_space_id != SpaceId(0) {
        space_display_id(selector_previous_space_id)
    } else {
        DisplayId(0)
    };

    let acting_space_id_is_first =
        acting_previous_space_id == SpaceId(0) || acting_previous_display_id != acting_display_id;
    let selector_space_id_is_first = selector_previous_space_id == SpaceId(0)
        || selector_previous_display_id != selector_display_id;
    let mut success = true;

    if acting_space_id_is_first && !selector_space_id_is_first {
        success = scripting_addition_move_space_after_space(
            acting_space_id,
            selector_space_id,
            acting_space_id == space_manager_active_space(window_manager),
        );
    } else if !acting_space_id_is_first && selector_space_id_is_first {
        success = scripting_addition_move_space_after_space(
            acting_space_id,
            selector_space_id,
            acting_space_id == space_manager_active_space(window_manager),
        );
        success &= scripting_addition_move_space_after_space(
            selector_space_id,
            acting_space_id,
            false,
        );
    } else if !acting_space_id_is_first && !selector_space_id_is_first {
        if space_manager_mission_control_index(acting_space_id)
            > space_manager_mission_control_index(selector_space_id)
        {
            success = scripting_addition_move_space_after_space(
                acting_space_id,
                selector_previous_space_id,
                acting_space_id == space_manager_active_space(window_manager),
            );
        } else {
            success = scripting_addition_move_space_after_space(
                acting_space_id,
                selector_space_id,
                acting_space_id == space_manager_active_space(window_manager),
            );
        }
    }

    if success {
        SpaceOpError::Success
    } else {
        SpaceOpError::ScriptingAddition
    }
}

pub(crate) fn space_manager_move_space_to_display(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }
    if space_id == SpaceId(0) {
        return SpaceOpError::MissingSrc;
    }

    let source_display_id = space_display_id(space_id);
    if source_display_id == display_id {
        return SpaceOpError::InvalidDst;
    }

    let is_source_animating = display_manager_display_is_animating(source_display_id);
    if is_source_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    let last_space = space_manager_is_space_last_user_space(space_id);
    if last_space {
        return SpaceOpError::InvalidSrc;
    }

    let is_destination_animating = display_manager_display_is_animating(display_id);
    if is_destination_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    let destination_space_id = display_space_id(display_id);
    if destination_space_id == SpaceId(0) {
        return SpaceOpError::MissingDst;
    }

    let focus_space = space_id == space_manager_active_space(window_manager);

    if scripting_addition_move_space_to_display(
        space_id,
        destination_space_id,
        if focus_space {
            space_manager_prev_space(space_id)
        } else {
            SpaceId(0)
        },
        focus_space,
    ) {
        space_manager_mark_view_invalid(space_manager, space_id, display_manager, window_manager);
        if focus_space {
            space_manager_focus_space(space_id, window_manager, mission_control_mode);
        }
        return SpaceOpError::Success;
    }

    SpaceOpError::ScriptingAddition
}

pub(crate) fn space_manager_destroy_space(
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }

    if space_id == SpaceId(0) {
        return SpaceOpError::MissingSrc;
    }
    if !space_is_user(space_id) {
        return SpaceOpError::InvalidType;
    }
    if space_manager_is_space_last_user_space(space_id) {
        return SpaceOpError::InvalidSrc;
    }

    let display_id = space_display_id(space_id);
    let first_space_id = space_manager_find_first_user_space_for_display(display_id);

    let is_animating = display_manager_display_is_animating(display_id);
    if is_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    let success = scripting_addition_destroy_space(space_id);
    if !success {
        return SpaceOpError::ScriptingAddition;
    }

    if first_space_id != SpaceId(0) {
        window_manager_validate_and_check_for_windows_on_space(
            space_manager,
            window_manager,
            first_space_id,
            display_manager,
            mouse_drag_state,
        );
    }

    SpaceOpError::Success
}

pub(crate) fn space_manager_add_space(
    space_id: SpaceId,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }
    if space_id == SpaceId(0) {
        return SpaceOpError::MissingSrc;
    }

    let is_animating = display_manager_display_is_animating(space_display_id(space_id));
    if is_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    if scripting_addition_create_space(space_id) {
        SpaceOpError::Success
    } else {
        SpaceOpError::ScriptingAddition
    }
}
