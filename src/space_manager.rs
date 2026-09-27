#![allow(deprecated)]

use core::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{Ordering, compiler_fence};

use objc2::ffi::objc_msgSend;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject, Sel};

use crate::display::{display_center, display_space_id, display_space_list};
use crate::display_manager::{
    DisplayManager, display_manager_active_display_id, display_manager_active_display_list,
    display_manager_cursor_display_id, display_manager_display_is_animating,
    display_manager_focus_display, display_manager_set_active_display_id,
};
use crate::ffi::carbon_process::CoreDockSendNotification;
use crate::ffi::core_foundation::{
    CFArray, CFDictionary, CFEqual, CFIndex, CFNumber, CFRetained, SendCFRetained, as_cftype,
    cfarray_borrow_value_at_index, cfarray_count, cfarray_of_cfnumbers, cfdictionary_borrow_value,
    cfnumber_read_u64_widening, k_com_apple_expose_awake, k_com_apple_showdesktop_awake, k_id64,
    k_spaces, kCFNumberSInt32Type, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGEventCreate, CGEventField, CGEventPost, CGEventSetDoubleValueField,
    CGEventSetIntegerValueField, CGPostMouseEvent, CGWarpMouseCursorPosition, kCGSessionEventTap,
};
use crate::ffi::skylight::{
    SLSCopyManagedDisplaySpaces, SLSMoveWindowsToManagedSpace, SLSSetWindowListWorkspace,
    SLSSpaceCopyName, SLSSpaceSetCompatID,
};
use crate::ffi::skylight_dynamic::sls_perform_asynchronous_bridged_window_management_operation;
use crate::globals::CONNECTION;
use crate::handles::{DisplayId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::misc::macros::{LAYER_BELOW, LAYER_NORMAL, TYPE_ABS, TYPE_REL, add_and_clamp_to_zero};
use crate::misc::response::Response;
use crate::misc::table::Table;
use crate::mission_control::{MissionControlMode, mission_control_is_active};
use crate::process_manager::ProcessManager;
use crate::sa::{
    scripting_addition_create_space, scripting_addition_destroy_space,
    scripting_addition_focus_space, scripting_addition_move_space_after_space,
    scripting_addition_move_space_to_display, scripting_addition_move_window_list_to_space,
    scripting_addition_move_window_to_space,
};
use crate::space::{space_display_id, space_is_user, space_is_visible, space_window_list};
use crate::state::MouseDragState;
use crate::view::{
    View, ViewFlag, ViewType, WindowInsertionPoint, WindowNodeChild, WindowNodeSplit,
    view_add_window_node_with_insertion_point, view_clear, view_create, view_find_window_node,
    view_flush, view_remove_window_node, view_serialize, view_update, window_node_balance,
    window_node_equalize, window_node_flush, window_node_is_intermediate, window_node_mirror,
    window_node_rotate, window_node_update,
};
use crate::window::{window_display_id, window_space, window_space_list};
use crate::window_manager::{
    WindowManager, window_manager_add_existing_application_windows, window_manager_adjust_layer,
    window_manager_focused_window, window_manager_validate_and_check_for_windows_on_space,
};
use crate::workspace::workspace_use_macos_space_workaround;

type InitWithWindowsSpaceIdFn =
    unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, u64) -> *mut AnyObject;

pub(crate) struct SpaceLabel {
    pub(crate) space_id: SpaceId,
    pub(crate) label: String,
}

pub(crate) struct SpaceManager {
    pub(crate) view: Table<SpaceId, View>,
    pub(crate) current_space_id: SpaceId,
    pub(crate) last_space_id: SpaceId,
    pub(crate) did_begin: bool,
    pub(crate) layout: ViewType,
    pub(crate) top_padding: i32,
    pub(crate) bottom_padding: i32,
    pub(crate) left_padding: i32,
    pub(crate) right_padding: i32,
    pub(crate) window_gap: i32,
    pub(crate) split_ratio: f32,
    pub(crate) split_type: WindowNodeSplit,
    pub(crate) window_placement: WindowNodeChild,
    pub(crate) window_insertion_point: WindowInsertionPoint,
    pub(crate) window_zoom_persist: bool,
    pub(crate) auto_balance: u32,
    pub(crate) labels: Vec<SpaceLabel>,
    pub(crate) skip_window_focus_animation: bool,
}

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

pub(crate) fn hash_view_key(key: &SpaceId) -> u64 {
    key.0
}

pub(crate) fn space_manager_query_space(
    response: &mut Response,
    space_id: SpaceId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    if space_manager_query_view(space_manager, space_id, display_manager, window_manager).is_none()
    {
        return false;
    }

    view_serialize(
        response,
        space_manager,
        space_id,
        flags,
        display_manager,
        window_manager,
    );
    response.write(format_args!("\n"));
    true
}

pub(crate) fn space_manager_query_spaces_for_window(
    response: &mut Response,
    window_id: WindowId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let space_list = window_space_list(window_id);
    if space_list.is_empty() {
        return false;
    }
    let space_count = space_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..space_count {
        let space_id = space_list[index as usize];
        if space_manager_query_view(space_manager, space_id, display_manager, window_manager)
            .is_none()
        {
            continue;
        }

        view_serialize(
            response,
            space_manager,
            space_id,
            flags,
            display_manager,
            window_manager,
        );
        response.write(format_args!(
            "{}",
            if index < space_count - 1 { ',' } else { ']' }
        ));
    }
    response.write(format_args!("\n"));

    true
}

pub(crate) fn space_manager_query_spaces_for_display(
    response: &mut Response,
    display_id: DisplayId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(space_list) = display_space_list(display_id) else {
        return false;
    };
    let space_count = space_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..space_count {
        let space_id = space_list[index as usize];
        if space_manager_query_view(space_manager, space_id, display_manager, window_manager)
            .is_none()
        {
            continue;
        }

        view_serialize(
            response,
            space_manager,
            space_id,
            flags,
            display_manager,
            window_manager,
        );
        response.write(format_args!(
            "{}",
            if index < space_count - 1 { ',' } else { ']' }
        ));
    }
    response.write(format_args!("\n"));

    true
}

pub(crate) fn space_manager_query_spaces_for_displays(
    response: &mut Response,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let display_list = display_manager_active_display_list();
    let display_count = display_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..display_count {
        let Some(space_list) = display_space_list(display_list[index as usize]) else {
            continue;
        };
        let space_count = space_list.len() as i32;

        for inner_index in 0..space_count {
            let space_id = space_list[inner_index as usize];
            if space_manager_query_view(space_manager, space_id, display_manager, window_manager)
                .is_none()
            {
                continue;
            }

            view_serialize(
                response,
                space_manager,
                space_id,
                flags,
                display_manager,
                window_manager,
            );
            if inner_index < space_count - 1 {
                response.write(format_args!(","));
            }
        }

        response.write(format_args!(
            "{}",
            if index < display_count - 1 { ',' } else { ']' }
        ));
    }
    response.write(format_args!("\n"));

    true
}

pub(crate) fn space_manager_query_view(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<SpaceId> {
    if space_manager.did_begin {
        return Some(space_manager_find_view(
            space_manager,
            space_id,
            display_manager,
            window_manager,
        ));
    }
    space_manager.view.find(&space_id).map(|_| space_id)
}

pub(crate) fn space_manager_find_view(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> SpaceId {
    if space_manager.view.find(&space_id).is_none() {
        view_create(space_id, display_manager, window_manager, space_manager);
    }
    space_id
}

pub(crate) fn space_manager_refresh_view(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    if view.layout == ViewType::Float {
        return;
    }

    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);
}

pub(crate) fn space_manager_mark_view_invalid(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    if view.layout == ViewType::Float {
        return;
    }

    view.clear_flag(ViewFlag::IS_VALID);
}

pub(crate) fn space_manager_untile_window(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    if view.layout == ViewType::Float {
        return;
    }

    window_manager_adjust_layer(window_id, LAYER_NORMAL, window_manager);
    let Some(node_id) = view_remove_window_node(
        space_manager,
        space_id,
        window_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    ) else {
        return;
    };

    if space_is_visible(space_id) {
        window_node_flush(space_id, node_id, window_manager, space_manager);
    } else if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }
}

pub(crate) fn space_manager_get_label_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> Option<&mut SpaceLabel> {
    space_manager
        .labels
        .iter_mut()
        .find(|space_label| space_label.space_id == space_id)
}

pub(crate) fn space_manager_get_space_for_label<'space_manager>(
    space_manager: &'space_manager mut SpaceManager,
    label: &[u8],
) -> Option<&'space_manager mut SpaceLabel> {
    space_manager
        .labels
        .iter_mut()
        .find(|space_label| space_label.label.as_bytes() == label)
}

pub(crate) fn space_manager_remove_label_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> bool {
    for index in 0..space_manager.labels.len() {
        let space_label = &space_manager.labels[index];
        if space_label.space_id == space_id {
            space_manager.labels.swap_remove(index);
            return true;
        }
    }

    false
}

pub(crate) fn space_manager_set_label_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    label: String,
) {
    space_manager_remove_label_for_space(space_manager, space_id);

    for index in 0..space_manager.labels.len() {
        let space_label = &space_manager.labels[index];
        if space_label.label == label {
            space_manager.labels.swap_remove(index);
            break;
        }
    }

    space_manager.labels.push(SpaceLabel { space_id, label });
}

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

pub(crate) fn space_manager_toggle_mission_control(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) {
    space_manager_focus_space(space_id, window_manager, mission_control_mode);
    unsafe { CoreDockSendNotification(k_com_apple_expose_awake(), 0) };
}

pub(crate) fn space_manager_toggle_show_desktop(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) {
    space_manager_focus_space(space_id, window_manager, mission_control_mode);
    unsafe { CoreDockSendNotification(k_com_apple_showdesktop_awake(), 0) };
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

pub(crate) fn space_manager_rotate_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    degrees: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewType::Bsp {
        return false;
    }

    window_node_rotate(space_id, ROOT_NODE_ID, degrees, space_manager);
    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_mirror_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    axis: WindowNodeSplit,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewType::Bsp {
        return false;
    }

    window_node_mirror(space_id, ROOT_NODE_ID, axis, space_manager);
    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_equalize_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    axis_flag: u32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewType::Bsp {
        return false;
    }

    window_node_equalize(space_id, ROOT_NODE_ID, axis_flag, space_manager);
    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_balance_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    axis_flag: u32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> bool {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    if view.layout != ViewType::Bsp {
        return false;
    }

    window_node_balance(space_id, ROOT_NODE_ID, axis_flag, space_manager);
    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);

    true
}

pub(crate) fn space_manager_tile_window_on_space_with_insertion_point(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    space_id: SpaceId,
    insertion_point: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> SpaceId {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return space_id;
    };
    if view.layout == ViewType::Float {
        return space_id;
    }

    window_manager_adjust_layer(window_id, LAYER_BELOW, window_manager);
    let node_id = view_add_window_node_with_insertion_point(
        space_manager,
        space_id,
        window_id,
        insertion_point,
        display_manager,
        window_manager,
    );
    debug_assert!(node_id.is_some());

    if space_is_visible(space_id) {
        if let Some(node_id) = node_id {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        }
    } else if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }

    space_id
}

pub(crate) fn space_manager_tile_window_on_space(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> SpaceId {
    space_manager_tile_window_on_space_with_insertion_point(
        space_manager,
        window_id,
        space_id,
        WindowId(0),
        display_manager,
        window_manager,
    )
}

pub(crate) fn space_manager_toggle_window_split(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let space_id = space_manager_find_view(
        space_manager,
        window_space(window_id),
        display_manager,
        window_manager,
    );
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    if view.layout != ViewType::Bsp {
        return;
    }

    let node_id = view_find_window_node(space_manager, space_id, window_id);
    if let Some(node_id) = node_id
        && window_node_is_intermediate(space_id, node_id, space_manager)
    {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let Some(parent_node_id) = view.node(node_id).parent else {
            return;
        };
        let parent_split = view.node(parent_node_id).split;
        view.node_mut(parent_node_id).split = if parent_split == WindowNodeSplit::Y {
            WindowNodeSplit::X
        } else {
            WindowNodeSplit::Y
        };

        let auto_balance = view.auto_balance;
        if auto_balance != WindowNodeSplit::None as u32 {
            window_node_balance(space_id, ROOT_NODE_ID, auto_balance, space_manager);
            view_update(space_manager, space_id, display_manager, window_manager);
            view_flush(space_manager, space_id, window_manager);
        } else {
            window_node_update(space_manager, space_id, parent_node_id, window_manager);
            if space_is_visible(space_id) {
                window_node_flush(space_id, parent_node_id, window_manager, space_manager);
            } else if let Some(view) = space_manager.view.find_mut(&space_id) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
        }
    }
}

pub(crate) fn space_manager_mission_control_index(space_id: SpaceId) -> i32 {
    let mut desktop_count: i32 = 1;

    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(*CONNECTION.get().unwrap()))
    }) else {
        return 0;
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    'out: {
        for index in 0..display_spaces_count {
            let display_ref: &CFDictionary =
                unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex) }
                    .unwrap();
            let spaces_ref: &CFArray =
                unsafe { cfdictionary_borrow_value(display_ref, k_spaces()) }.unwrap();
            let spaces_count = cfarray_count(spaces_ref) as i32;

            for inner_index in 0..spaces_count {
                let space_ref: &CFDictionary =
                    unsafe { cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex) }
                        .unwrap();
                let space_id_ref: &CFNumber =
                    unsafe { cfdictionary_borrow_value(space_ref, k_id64()) }.unwrap();
                let result = cfnumber_read_u64_widening(space_id_ref);
                if space_id.0 == result {
                    break 'out;
                }

                desktop_count += 1;
            }
        }

        desktop_count = 0;
    }

    desktop_count
}

pub(crate) fn space_manager_mission_control_space(desktop_id: i32) -> SpaceId {
    let mut result: u64;
    let mut desktop_count: i32 = 1;

    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(*CONNECTION.get().unwrap()))
    }) else {
        return SpaceId(0);
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    'out: {
        for index in 0..display_spaces_count {
            let display_ref: &CFDictionary =
                unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex) }
                    .unwrap();
            let spaces_ref: &CFArray =
                unsafe { cfdictionary_borrow_value(display_ref, k_spaces()) }.unwrap();
            let spaces_count = cfarray_count(spaces_ref) as i32;

            for inner_index in 0..spaces_count {
                let space_ref: &CFDictionary =
                    unsafe { cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex) }
                        .unwrap();
                let space_id_ref: &CFNumber =
                    unsafe { cfdictionary_borrow_value(space_ref, k_id64()) }.unwrap();
                result = cfnumber_read_u64_widening(space_id_ref);
                if desktop_count == desktop_id {
                    break 'out;
                }

                desktop_count += 1;
            }
        }

        result = 0;
    }

    SpaceId(result)
}

pub(crate) fn space_manager_cursor_space() -> SpaceId {
    let display_id = display_manager_cursor_display_id();
    display_space_id(display_id)
}

pub(crate) fn space_manager_prev_space(space_id: SpaceId) -> SpaceId {
    let mut previous_space_id: u64 = 0;
    let mut next_space_id: u64;

    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(*CONNECTION.get().unwrap()))
    }) else {
        return SpaceId(0);
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    'out: {
        for index in 0..display_spaces_count {
            let display_ref: &CFDictionary =
                unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex) }
                    .unwrap();
            let spaces_ref: &CFArray =
                unsafe { cfdictionary_borrow_value(display_ref, k_spaces()) }.unwrap();
            let spaces_count = cfarray_count(spaces_ref) as i32;

            for inner_index in 0..spaces_count {
                let space_ref: &CFDictionary =
                    unsafe { cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex) }
                        .unwrap();
                let space_id_ref: &CFNumber =
                    unsafe { cfdictionary_borrow_value(space_ref, k_id64()) }.unwrap();
                next_space_id = cfnumber_read_u64_widening(space_id_ref);
                if next_space_id == space_id.0 {
                    break 'out;
                }

                previous_space_id = next_space_id;
            }
        }
    }

    if previous_space_id != space_id.0 {
        SpaceId(previous_space_id)
    } else {
        SpaceId(0)
    }
}

pub(crate) fn space_manager_next_space(space_id: SpaceId) -> SpaceId {
    let mut next_space_id: u64 = 0;
    let mut found_space_id = false;

    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(*CONNECTION.get().unwrap()))
    }) else {
        return SpaceId(0);
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    'out: {
        for index in 0..display_spaces_count {
            let display_ref: &CFDictionary =
                unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex) }
                    .unwrap();
            let spaces_ref: &CFArray =
                unsafe { cfdictionary_borrow_value(display_ref, k_spaces()) }.unwrap();
            let spaces_count = cfarray_count(spaces_ref) as i32;

            for inner_index in 0..spaces_count {
                let space_ref: &CFDictionary =
                    unsafe { cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex) }
                        .unwrap();
                let space_id_ref: &CFNumber =
                    unsafe { cfdictionary_borrow_value(space_ref, k_id64()) }.unwrap();
                next_space_id = cfnumber_read_u64_widening(space_id_ref);
                if found_space_id {
                    break 'out;
                }

                found_space_id = next_space_id == space_id.0;
            }
        }
    }

    if next_space_id != space_id.0 {
        SpaceId(next_space_id)
    } else {
        SpaceId(0)
    }
}

pub(crate) fn space_manager_first_space() -> SpaceId {
    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(*CONNECTION.get().unwrap()))
    }) else {
        return SpaceId(0);
    };

    if cfarray_count(&display_spaces_ref) <= 0 {
        return SpaceId(0);
    }
    let display_ref: &CFDictionary =
        unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, 0) }.unwrap();
    let spaces_ref: &CFArray =
        unsafe { cfdictionary_borrow_value(display_ref, k_spaces()) }.unwrap();

    if cfarray_count(spaces_ref) <= 0 {
        return SpaceId(0);
    }
    let space_ref: &CFDictionary = unsafe { cfarray_borrow_value_at_index(spaces_ref, 0) }.unwrap();
    let space_id_ref: &CFNumber =
        unsafe { cfdictionary_borrow_value(space_ref, k_id64()) }.unwrap();
    let space_id = cfnumber_read_u64_widening(space_id_ref);

    SpaceId(space_id)
}

pub(crate) fn space_manager_last_space() -> SpaceId {
    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(*CONNECTION.get().unwrap()))
    }) else {
        return SpaceId(0);
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    if display_spaces_count <= 0 {
        return SpaceId(0);
    }
    let display_ref: &CFDictionary = unsafe {
        cfarray_borrow_value_at_index(&display_spaces_ref, (display_spaces_count - 1) as CFIndex)
    }
    .unwrap();
    let spaces_ref: &CFArray =
        unsafe { cfdictionary_borrow_value(display_ref, k_spaces()) }.unwrap();
    let spaces_count = cfarray_count(spaces_ref) as i32;

    if spaces_count <= 0 {
        return SpaceId(0);
    }
    let space_ref: &CFDictionary =
        unsafe { cfarray_borrow_value_at_index(spaces_ref, (spaces_count - 1) as CFIndex) }
            .unwrap();
    let space_id_ref: &CFNumber =
        unsafe { cfdictionary_borrow_value(space_ref, k_id64()) }.unwrap();
    let space_id = cfnumber_read_u64_widening(space_id_ref);

    SpaceId(space_id)
}

pub(crate) fn space_manager_active_space(window_manager: &mut WindowManager) -> SpaceId {
    let mut display_id = DisplayId(0);
    let window = window_manager_focused_window(window_manager);

    if let Some(window_id) = window {
        display_id = window_display_id(window_id);
    }
    if display_id == DisplayId(0) {
        display_id = display_manager_active_display_id();
    }
    if display_id == DisplayId(0) {
        return SpaceId(0);
    }

    display_space_id(display_id)
}

pub(crate) fn space_manager_move_window_list_to_space(space_id: SpaceId, window_list: &[WindowId]) {
    let connection_id = *CONNECTION.get().unwrap();
    let mut window_id_list: Vec<u32> = window_list.iter().map(|window_id| window_id.0).collect();

    if let Some(perform_operation) = sls_perform_asynchronous_bridged_window_management_operation()
    {
        let window_list_ref = cfarray_of_cfnumbers(&window_id_list, kCFNumberSInt32Type);
        let Some(class) = AnyClass::get(c"SLSBridgedMoveWindowsToManagedSpaceOperation") else {
            return;
        };
        let selector = Sel::register(c"initWithWindows:spaceID:");
        let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
        let init_with_windows_space_id: InitWithWindowsSpaceIdFn = unsafe {
            core::mem::transmute::<*const c_void, InitWithWindowsSpaceIdFn>(
                objc_msgSend as *const c_void,
            )
        };
        let operation = unsafe {
            init_with_windows_space_id(
                allocated,
                selector,
                CFRetained::as_ptr(&window_list_ref)
                    .as_ptr()
                    .cast::<AnyObject>(),
                space_id.0,
            )
        };
        unsafe { perform_operation(operation.cast::<c_void>()) };
        let _: () = unsafe { msg_send![operation, release] };
        drop(window_list_ref);
    } else if !workspace_use_macos_space_workaround() {
        let window_list_ref = cfarray_of_cfnumbers(&window_id_list, kCFNumberSInt32Type);
        unsafe { SLSMoveWindowsToManagedSpace(connection_id, &*window_list_ref, space_id.0) };
        drop(window_list_ref);
    } else if !scripting_addition_move_window_list_to_space(space_id, window_list) {
        unsafe { SLSSpaceSetCompatID(connection_id, space_id.0, 0x79616265) };
        unsafe {
            SLSSetWindowListWorkspace(
                connection_id,
                window_id_list.as_mut_ptr(),
                window_id_list.len() as i32,
                0x79616265,
            )
        };
        unsafe { SLSSpaceSetCompatID(connection_id, space_id.0, 0x0) };
    }
}

pub(crate) fn space_manager_move_window_to_space(space_id: SpaceId, window_id: WindowId) {
    let connection_id = *CONNECTION.get().unwrap();
    let mut window_id_value: u32 = window_id.0;

    if let Some(perform_operation) = sls_perform_asynchronous_bridged_window_management_operation()
    {
        let window_list_ref = cfarray_of_cfnumbers(&[window_id_value], kCFNumberSInt32Type);
        let Some(class) = AnyClass::get(c"SLSBridgedMoveWindowsToManagedSpaceOperation") else {
            return;
        };
        let selector = Sel::register(c"initWithWindows:spaceID:");
        let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
        let init_with_windows_space_id: InitWithWindowsSpaceIdFn = unsafe {
            core::mem::transmute::<*const c_void, InitWithWindowsSpaceIdFn>(
                objc_msgSend as *const c_void,
            )
        };
        let operation = unsafe {
            init_with_windows_space_id(
                allocated,
                selector,
                CFRetained::as_ptr(&window_list_ref)
                    .as_ptr()
                    .cast::<AnyObject>(),
                space_id.0,
            )
        };
        unsafe { perform_operation(operation.cast::<c_void>()) };
        let _: () = unsafe { msg_send![operation, release] };
        drop(window_list_ref);
    } else if !workspace_use_macos_space_workaround() {
        let window_list_ref = cfarray_of_cfnumbers(&[window_id_value], kCFNumberSInt32Type);
        unsafe { SLSMoveWindowsToManagedSpace(connection_id, &*window_list_ref, space_id.0) };
        drop(window_list_ref);
    } else if !scripting_addition_move_window_to_space(space_id, window_id) {
        unsafe { SLSSpaceSetCompatID(connection_id, space_id.0, 0x79616265) };
        unsafe { SLSSetWindowListWorkspace(connection_id, &mut window_id_value, 1, 0x79616265) };
        unsafe { SLSSpaceSetCompatID(connection_id, space_id.0, 0x0) };
    }
}

pub(crate) fn space_manager_find_first_user_space_for_display(display_id: DisplayId) -> SpaceId {
    let Some(space_list) = display_space_list(display_id) else {
        return SpaceId(0);
    };
    let count = space_list.len() as i32;

    for index in 0..count {
        let space_id = space_list[index as usize];

        if space_is_user(space_id) {
            return space_id;
        }
    }

    SpaceId(0)
}

pub(crate) fn space_manager_is_space_last_user_space(space_id: SpaceId) -> bool {
    let mut result = true;

    let Some(space_list) = display_space_list(space_display_id(space_id)) else {
        return true;
    };
    let count = space_list.len() as i32;

    for index in 0..count {
        let current_space_id = space_list[index as usize];
        if space_id == current_space_id {
            continue;
        }

        if space_is_user(current_space_id) {
            result = false;
            break;
        }
    }

    result
}

pub(crate) fn space_manager_point_view_handles_at_rekeyed_views(
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
    space_id_of_view_after_rekeying: impl Fn(SpaceId) -> SpaceId,
) {
    for managed_view_space_id in window_manager.managed_window.values_mut() {
        *managed_view_space_id = space_id_of_view_after_rekeying(*managed_view_space_id);
    }

    for (feedback_node_space_id, _) in window_manager.insert_feedback.values_mut() {
        *feedback_node_space_id = space_id_of_view_after_rekeying(*feedback_node_space_id);
    }

    if let Some((feedback_node_space_id, _)) = mouse_drag_state.feedback_node.as_mut() {
        *feedback_node_space_id = space_id_of_view_after_rekeying(*feedback_node_space_id);
    }
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

pub(crate) fn space_manager_focus_space_using_gesture(
    new_display_id: DisplayId,
    new_space_id: SpaceId,
    window_manager: &mut WindowManager,
) -> bool {
    let current_index = space_manager_mission_control_index(display_space_id(new_display_id));
    let new_index = space_manager_mission_control_index(new_space_id);

    let count = (new_index - current_index).abs();
    if count == 0 {
        display_manager_focus_display(new_display_id, new_space_id, window_manager);
        return true;
    }

    let point = display_center(new_display_id);
    let current_display_id = display_manager_cursor_display_id();

    let focus_display = current_display_id != new_display_id;
    if focus_display {
        CGWarpMouseCursorPosition(point);
    }

    //
    // NOTE(asmvik): MacOS does not have an API that allows for space activation.
    // However, we can synthesize a sequence of high velocity gestures to skip the
    // animation instead.
    //
    // :Attribution
    // https://github.com/jurplel/InstantSpaceSwitcher
    // https://github.com/thenickdude/wacom-driver-fix/blob/bdfda9a788934c88d09d31ea6a42664b9ba1471e/Readme.md
    // Technique first observed in practice, and reverse-engineered from, BetterTouchTool.
    //

    let Some(event_dock_control) = CGEventCreate(None) else {
        return false;
    };

    let sign: f32 = (if (new_index - current_index) > 0 {
        1.0f64
    } else {
        -1.0f64
    }) as f32;
    CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGSEventTypeField            */ CGEventField(55), /* kCGSEventDockControl       */ 30);
    CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGEventGestureHIDType        */ CGEventField(110), /* kIOHIDEventTypeDockSwipe   */ 23);
    CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGEventGestureSwipeMotion    */ CGEventField(123), /* kCGGestureMotionHorizontal */ 1);
    CGEventSetDoubleValueField(Some(&event_dock_control), /* kCGEventGestureSwipeProgress  */ CGEventField(124), sign as f64);
    CGEventSetDoubleValueField(Some(&event_dock_control), /* kCGEventGestureSwipeVelocityX */ CGEventField(129), sign as f64 * 9999.0f64);

    for _ in 0..count {
        CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGEventGesturePhase */ CGEventField(132), /* kCGSGesturePhaseBegan */ 1);
        CGEventPost(kCGSessionEventTap, Some(&event_dock_control));
        CGEventSetIntegerValueField(Some(&event_dock_control), /* kCGEventGesturePhase */ CGEventField(132), /* kCGSGesturePhaseEnded */ 4);
        CGEventPost(kCGSessionEventTap, Some(&event_dock_control));
    }
    drop(event_dock_control);

    if focus_display {
        display_manager_set_active_display_id(new_display_id);
        if space_manager_active_space(window_manager) != new_space_id {
            unsafe { CGPostMouseEvent(point, 0, 1, 1) };
            unsafe { CGPostMouseEvent(point, 0, 1, 0) };
        }
    }

    true
}

pub(crate) fn space_manager_focus_space(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }

    let current_space_id = space_manager_active_space(window_manager);
    if current_space_id == space_id {
        return SpaceOpError::SameSpace;
    }

    let current_display_id = space_display_id(current_space_id);
    let new_display_id = space_display_id(space_id);
    let focus_display = current_display_id != new_display_id;

    let is_animating = display_manager_display_is_animating(new_display_id);
    if is_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    if scripting_addition_focus_space(space_id) {
        if focus_display {
            display_manager_focus_display(new_display_id, space_id, window_manager);
        }
    } else {
        space_manager_focus_space_using_gesture(new_display_id, space_id, window_manager);
    }

    SpaceOpError::Success
}

pub(crate) fn space_manager_switch_space(
    space_id: SpaceId,
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

    let current_space_id = space_manager_active_space(window_manager);
    if current_space_id == space_id {
        return SpaceOpError::SameSpace;
    }

    let current_display_id = space_display_id(current_space_id);
    let display_id = space_display_id(space_id);

    let is_source_animating = display_manager_display_is_animating(current_display_id);
    if is_source_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    let is_destination_animating = display_manager_display_is_animating(display_id);
    if is_destination_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    if current_display_id != display_id {
        space_manager_swap_space_with_space_on_display(
            current_display_id,
            current_space_id,
            display_id,
            space_id,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
        display_manager_focus_display(current_display_id, current_space_id, window_manager);
        return SpaceOpError::Success;
    }

    if scripting_addition_focus_space(space_id) {
        SpaceOpError::Success
    } else {
        SpaceOpError::ScriptingAddition
    }
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

pub(crate) fn space_manager_is_window_on_space(space_id: SpaceId, window_id: WindowId) -> bool {
    let space_list = window_space_list(window_id);
    if space_list.is_empty() {
        return false;
    }
    let space_count = space_list.len() as i32;

    for index in 0..space_count {
        if space_id == space_list[index as usize] {
            return true;
        }
    }

    false
}

pub(crate) fn space_manager_mark_spaces_invalid_for_display(
    space_manager: &mut SpaceManager,
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let Some(space_list) = display_space_list(display_id) else {
        return;
    };
    let space_count = space_list.len() as i32;

    let space_id = display_space_id(display_id);
    for index in 0..space_count {
        if space_list[index as usize] == space_id {
            space_manager_refresh_view(space_manager, space_id, display_manager, window_manager);
        } else {
            space_manager_mark_view_invalid(
                space_manager,
                space_list[index as usize],
                display_manager,
                window_manager,
            );
        }
    }
}

pub(crate) fn space_manager_mark_spaces_invalid(
    space_manager: &mut SpaceManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let display_list = display_manager_active_display_list();
    let display_count = display_list.len() as i32;

    for index in 0..display_count {
        space_manager_mark_spaces_invalid_for_display(
            space_manager,
            display_list[index as usize],
            display_manager,
            window_manager,
        );
    }
}

pub(crate) fn space_manager_refresh_application_windows(
    space_manager: &mut SpaceManager,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> bool {
    let mut refresh_count = window_manager.applications_to_refresh.len() as i32;
    if refresh_count == 0 {
        return false;
    }
    let window_count = window_manager.window.len();
    let mut index: i32 = 0;
    while index < refresh_count {
        let process_id = window_manager.applications_to_refresh[index as usize];
        let application_name = window_manager
            .application
            .find(&process_id)
            .map(|application| Arc::clone(&application.name));
        crate::debug!(
            "{}: {} has windows that are not yet resolved\n",
            "space_manager_refresh_application_windows",
            application_name.as_deref().unwrap_or("(null)")
        );
        let result = window_manager_add_existing_application_windows(
            space_manager,
            window_manager,
            process_id,
            index,
            process_manager,
            display_manager,
            mouse_drag_state,
            mission_control_mode,
        );
        if result {
            refresh_count -= 1;
            index -= 1;
        }
        index += 1;
    }
    window_count != window_manager.window.len()
}

pub(crate) fn space_manager_handle_display_add(
    space_manager: &mut SpaceManager,
    display_id: DisplayId,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some(space_list) = display_space_list(display_id) else {
        return;
    };
    let space_count = space_list.len() as i32;

    let mut view_list: Vec<Option<SpaceId>> = space_manager
        .view
        .keys_in_bucket_order()
        .into_iter()
        .map(Some)
        .collect();
    let list_count = view_list.len() as i32;

    for index in 0..space_count {
        let space_id = space_list[index as usize];
        let Some(uuid) = (unsafe {
            take_create_rule_result(SLSSpaceCopyName(*CONNECTION.get().unwrap(), space_id.0))
        }) else {
            continue;
        };

        for inner_index in 0..list_count {
            let Some(view_space_id) = view_list[inner_index as usize] else {
                continue;
            };
            let Some(view_uuid) = space_manager
                .view
                .find(&view_space_id)
                .and_then(|view| view.uuid.as_ref())
            else {
                continue;
            };

            if CFEqual(Some(as_cftype(view_uuid.as_ref())), Some(as_cftype(&*uuid))) {
                view_list[inner_index as usize] = None;

                let Some(mut view) = space_manager.view.remove(&view_space_id) else {
                    break;
                };
                drop(view.uuid.take());

                if let Some(label) = space_manager_get_label_for_space(space_manager, view.space_id)
                {
                    label.space_id = space_id;
                }

                view.space_id = space_id;
                view.uuid = Some(SendCFRetained(uuid.clone()));

                let view_is_kept_by_the_table = space_manager.view.find(&space_id).is_none();
                space_manager.view.add(space_id, view);
                if view_is_kept_by_the_table {
                    space_manager_point_view_handles_at_rekeyed_views(
                        window_manager,
                        mouse_drag_state,
                        |handle_space_id| {
                            if handle_space_id == view_space_id {
                                space_id
                            } else {
                                handle_space_id
                            }
                        },
                    );
                }
                break;
            }
        }

        drop(uuid);
    }

    space_manager.current_space_id = space_manager_active_space(window_manager);
    space_manager.last_space_id = space_manager.current_space_id;
}

pub(crate) fn space_manager_begin(
    space_manager: &mut SpaceManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.layout = ViewType::Float;
    space_manager.split_ratio = 0.5f32;
    space_manager.auto_balance = WindowNodeSplit::None as u32;
    space_manager.split_type = WindowNodeSplit::Auto;
    space_manager.window_placement = WindowNodeChild::Second;
    space_manager.window_insertion_point = WindowInsertionPoint::Focused;
    space_manager.window_zoom_persist = true;
    space_manager.labels = Vec::new();
    space_manager.skip_window_focus_animation = false;
    space_manager.view = Table::new(23, hash_view_key);

    let display_list = display_manager_active_display_list();
    let display_count = display_list.len() as i32;

    for index in 0..display_count {
        let Some(space_list) = display_space_list(display_list[index as usize]) else {
            continue;
        };
        let space_count = space_list.len() as i32;

        for inner_index in 0..space_count {
            view_create(
                space_list[inner_index as usize],
                display_manager,
                window_manager,
                space_manager,
            );
        }
    }

    space_manager.current_space_id = space_manager_active_space(window_manager);
    space_manager.last_space_id = space_manager.current_space_id;
    space_manager.did_begin = true;
}
