#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;

use crate::display::manager::DisplayManager;
use crate::ffi::accessibility::{
    AXUIElementSetAttributeValue, AXValueCreate, AXValueType, kAXPositionAttribute,
    kAXSizeAttribute, with_enhanced_user_interface_disabled,
};
use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize, as_cftype};
use crate::layout::settings::ViewFlag;
use crate::layout::tree::{
    ancestor_whose_split_borders_node_in_direction, leaf_holding_window,
    move_windows_below_node_into_their_areas, recompute_areas_below_node_redrawing_insert_feedback,
};
use crate::layout::view::{
    move_view_windows_into_their_areas_or_defer_until_space_is_visible,
    recompute_view_areas_from_display_bounds_and_padding,
};
use crate::space::managed_space::is_space_visible_on_its_display;
use crate::space::manager::SpaceManager;
use crate::support::direction::{DIRECTION_EAST, DIRECTION_NORTH, DIRECTION_SOUTH, DIRECTION_WEST};
use crate::support::handles::{NodeId, WindowId};
use crate::support::resize_handle::ResizeHandle;
use crate::support::type_of_change::{CHANGE_TYPE_ABSOLUTE, CHANGE_TYPE_RELATIVE};
use crate::window::animation::{
    WindowWithTargetFrame, move_window_to_its_target_frame_animating_if_enabled,
};
use crate::window::manager::{WindowManager, WindowOperationOutcome, space_managing_window};
use crate::window::model::read_window_frame_through_accessibility;

pub(crate) fn adjust_split_ratio_of_managed_window_parent_node(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    type_of_change: i32,
    ratio: f32,
    space_manager: &mut SpaceManager,
) -> WindowOperationOutcome {
    let Some(space_id) = space_managing_window(window_manager, window_id) else {
        return WindowOperationOutcome::InvalidSourceView;
    };

    let node_id = leaf_holding_window(space_manager, space_id, window_id);
    let parent_node_id = node_id.and_then(|node_id| {
        space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .and_then(|node| node.parent)
    });
    let Some(parent_node_id) = parent_node_id else {
        return WindowOperationOutcome::InvalidSourceNode;
    };

    match type_of_change {
        CHANGE_TYPE_RELATIVE => {
            if let Some(parent_node) =
                space_manager.find_node_mut_in_view_of_space(space_id, parent_node_id)
            {
                parent_node.ratio = (parent_node.ratio + ratio).clamp(0.1, 0.9);
            }
        }
        CHANGE_TYPE_ABSOLUTE => {
            if let Some(parent_node) =
                space_manager.find_node_mut_in_view_of_space(space_id, parent_node_id)
            {
                parent_node.ratio = ratio.clamp(0.1, 0.9);
            }
        }
        _ => {}
    }

    recompute_areas_below_node_redrawing_insert_feedback(
        space_manager,
        space_id,
        parent_node_id,
        window_manager,
    );

    if is_space_visible_on_its_display(space_id) {
        move_windows_below_node_into_their_areas(
            space_id,
            parent_node_id,
            window_manager,
            space_manager,
        );
    } else if let Some(view) = space_manager.view.get_mut(&space_id) {
        view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
    }

    WindowOperationOutcome::Success
}

pub(crate) fn move_floating_window_by_offset_or_to_position(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    type_of_change: i32,
    delta_x: f32,
    delta_y: f32,
) -> WindowOperationOutcome {
    let view = space_managing_window(window_manager, window_id);
    if view.is_some() {
        return WindowOperationOutcome::InvalidSourceView;
    }

    let Some(window) = window_manager.window.get(&window_id) else {
        return WindowOperationOutcome::Success;
    };
    let window_frame = window.frame;

    let mut delta_x = delta_x;
    let mut delta_y = delta_y;

    if type_of_change == CHANGE_TYPE_RELATIVE {
        delta_x = (delta_x as f64 + window_frame.origin.x) as f32;
        delta_y = (delta_y as f64 + window_frame.origin.y) as f32;
    }

    move_window_to_its_target_frame_animating_if_enabled(
        WindowWithTargetFrame {
            window_id,
            x: delta_x,
            y: delta_y,
            width: window_frame.size.width as f32,
            height: window_frame.size.height as f32,
        },
        window_manager,
    );
    WindowOperationOutcome::Success
}

pub(crate) fn resize_floating_window_by_dragging_edges(
    window_id: WindowId,
    frame: CGRect,
    direction: i32,
    delta_x: f32,
    delta_y: f32,
    animate: bool,
    window_manager: &mut WindowManager,
) {
    let x_modifier: i32 = if direction & ResizeHandle::LEFT.0 as i32 != 0 {
        -1
    } else if direction & ResizeHandle::RIGHT.0 as i32 != 0 {
        1
    } else {
        0
    };
    let y_modifier: i32 = if direction & ResizeHandle::TOP.0 as i32 != 0 {
        -1
    } else if direction & ResizeHandle::BOTTOM.0 as i32 != 0 {
        1
    } else {
        0
    };

    let frame_width = (frame.size.width + (delta_x * x_modifier as f32) as f64).max(1.0) as f32;
    let frame_height = (frame.size.height + (delta_y * y_modifier as f32) as f64).max(1.0) as f32;
    let frame_x = if direction & ResizeHandle::LEFT.0 as i32 != 0 {
        (frame.origin.x + frame.size.width - frame_width as f64) as f32
    } else {
        frame.origin.x as f32
    };
    let frame_y = if direction & ResizeHandle::TOP.0 as i32 != 0 {
        (frame.origin.y + frame.size.height - frame_height as f64) as f32
    } else {
        frame.origin.y as f32
    };

    if animate {
        move_window_to_its_target_frame_animating_if_enabled(
            WindowWithTargetFrame {
                window_id,
                x: frame_x,
                y: frame_y,
                width: frame_width,
                height: frame_height,
            },
            window_manager,
        );
    } else {
        let Some(application_process_id) = window_manager
            .window
            .get(&window_id)
            .and_then(|window| window.application)
        else {
            return;
        };
        let Some(application) = window_manager.application.get(&application_process_id) else {
            return;
        };
        let application_element_ref = application.element_ref;

        with_enhanced_user_interface_disabled(unsafe { &*application_element_ref }, || {
            move_window_through_accessibility(window_id, frame_x, frame_y, window_manager);
            resize_window_through_accessibility(
                window_id,
                frame_width,
                frame_height,
                window_manager,
            );
        });
    }
}

pub(crate) fn resize_window_by_dragging_edges_or_to_absolute_size(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    direction: i32,
    delta_x: f32,
    delta_y: f32,
    animate: bool,
    display_manager: &mut DisplayManager,
    space_manager: &mut SpaceManager,
) -> WindowOperationOutcome {
    let view = space_managing_window(window_manager, window_id);
    if let Some(space_id) = view {
        if direction == ResizeHandle::ABSOLUTE.0 as i32 {
            return WindowOperationOutcome::InvalidOperation;
        }

        let Some(node_id) = leaf_holding_window(space_manager, space_id, window_id) else {
            return WindowOperationOutcome::InvalidSourceNode;
        };

        let mut x_fence: Option<NodeId> = None;
        let mut y_fence: Option<NodeId> = None;

        if direction & ResizeHandle::TOP.0 as i32 != 0 {
            x_fence = ancestor_whose_split_borders_node_in_direction(
                space_id,
                node_id,
                DIRECTION_NORTH,
                space_manager,
            );
        }
        if direction & ResizeHandle::BOTTOM.0 as i32 != 0 {
            x_fence = ancestor_whose_split_borders_node_in_direction(
                space_id,
                node_id,
                DIRECTION_SOUTH,
                space_manager,
            );
        }
        if direction & ResizeHandle::LEFT.0 as i32 != 0 {
            y_fence = ancestor_whose_split_borders_node_in_direction(
                space_id,
                node_id,
                DIRECTION_WEST,
                space_manager,
            );
        }
        if direction & ResizeHandle::RIGHT.0 as i32 != 0 {
            y_fence = ancestor_whose_split_borders_node_in_direction(
                space_id,
                node_id,
                DIRECTION_EAST,
                space_manager,
            );
        }
        if x_fence.is_none() && y_fence.is_none() {
            return WindowOperationOutcome::InvalidDestinationNode;
        }

        if let Some(y_fence) = y_fence
            && let Some(y_fence_node) =
                space_manager.find_node_mut_in_view_of_space(space_id, y_fence)
        {
            let scaled_ratio = y_fence_node.ratio + delta_x / y_fence_node.area.width;
            y_fence_node.ratio = scaled_ratio.clamp(0.1, 0.9);
        }

        if let Some(x_fence) = x_fence
            && let Some(x_fence_node) =
                space_manager.find_node_mut_in_view_of_space(space_id, x_fence)
        {
            let scaled_ratio = x_fence_node.ratio + delta_y / x_fence_node.area.height;
            x_fence_node.ratio = scaled_ratio.clamp(0.1, 0.9);
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
    } else {
        if direction == ResizeHandle::ABSOLUTE.0 as i32 {
            if animate {
                let Some(window) = window_manager.window.get(&window_id) else {
                    return WindowOperationOutcome::Success;
                };
                let window_frame = window.frame;
                move_window_to_its_target_frame_animating_if_enabled(
                    WindowWithTargetFrame {
                        window_id,
                        x: window_frame.origin.x as f32,
                        y: window_frame.origin.y as f32,
                        width: delta_x,
                        height: delta_y,
                    },
                    window_manager,
                );
            } else {
                let Some(application_process_id) = window_manager
                    .window
                    .get(&window_id)
                    .and_then(|window| window.application)
                else {
                    return WindowOperationOutcome::Success;
                };
                let Some(application) = window_manager.application.get(&application_process_id)
                else {
                    return WindowOperationOutcome::Success;
                };
                let application_element_ref = application.element_ref;

                with_enhanced_user_interface_disabled(unsafe { &*application_element_ref }, || {
                    resize_window_through_accessibility(
                        window_id,
                        delta_x,
                        delta_y,
                        window_manager,
                    );
                });
            }
        } else {
            let Some(window) = window_manager.window.get(&window_id) else {
                return WindowOperationOutcome::Success;
            };
            let frame = read_window_frame_through_accessibility(window);
            resize_floating_window_by_dragging_edges(
                window_id,
                frame,
                direction,
                delta_x,
                delta_y,
                animate,
                window_manager,
            );
        }
    }

    WindowOperationOutcome::Success
}

pub(crate) fn move_window_through_accessibility(
    window_id: WindowId,
    x: f32,
    y: f32,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.get(&window_id) else {
        return;
    };
    let window_element_ref = window.element_ref;

    let mut position = CGPoint::new(x as f64, y as f64);
    let position_ref = unsafe {
        AXValueCreate(
            AXValueType::CGPoint,
            NonNull::from(&mut position).cast::<c_void>(),
        )
    };
    let Some(position_ref) = position_ref else {
        return;
    };

    unsafe {
        AXUIElementSetAttributeValue(
            &*window_element_ref,
            kAXPositionAttribute(),
            as_cftype(&*position_ref),
        )
    };
    drop(position_ref);
}

pub(crate) fn resize_window_through_accessibility(
    window_id: WindowId,
    width: f32,
    height: f32,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.get(&window_id) else {
        return;
    };
    let window_element_ref = window.element_ref;

    let mut size = CGSize::new(width as f64, height as f64);
    let size_ref = unsafe {
        AXValueCreate(
            AXValueType::CGSize,
            NonNull::from(&mut size).cast::<c_void>(),
        )
    };
    let Some(size_ref) = size_ref else {
        return;
    };

    unsafe {
        AXUIElementSetAttributeValue(
            &*window_element_ref,
            kAXSizeAttribute(),
            as_cftype(&*size_ref),
        )
    };
    drop(size_ref);
}

pub(crate) fn move_and_resize_window_through_accessibility(
    window_id: WindowId,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    window_manager: &mut WindowManager,
) {
    //
    // NOTE(asmvik): Attempting to check the window frame cache to prevent unnecessary movement and resize calls to the AX API
    // is not reliable because it is possible to perform operations that should be applied, at a higher rate than the AX API events
    // are received, causing our cache to become out of date and incorrectly guard against some changes that **should** be applied.
    // This causes the window layout to **not** be modified the way we expect.
    //
    // A possible solution is to use the faster CG window notifications, as they are **a lot** more responsive, and can be used to
    // track changes to the window frame in real-time without delay.
    //

    let Some(window) = window_manager.window.get(&window_id) else {
        return;
    };
    let window_element_ref = window.element_ref;
    let Some(application) = window
        .application
        .and_then(|application_process_id| window_manager.application.get(&application_process_id))
    else {
        return;
    };
    let application_element_ref = application.element_ref;

    with_enhanced_user_interface_disabled(unsafe { &*application_element_ref }, || {
        let mut position = CGPoint::new(x as f64, y as f64);
        let position_ref = unsafe {
            AXValueCreate(
                AXValueType::CGPoint,
                NonNull::from(&mut position).cast::<c_void>(),
            )
        };

        let mut size = CGSize::new(width as f64, height as f64);
        let size_ref = unsafe {
            AXValueCreate(
                AXValueType::CGSize,
                NonNull::from(&mut size).cast::<c_void>(),
            )
        };

        // NOTE(asmvik): Due to macOS constraints (visible screen-area), we might need to resize the window *before* moving it.
        if let Some(size_ref) = &size_ref {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window_element_ref,
                    kAXSizeAttribute(),
                    as_cftype(&**size_ref),
                )
            };
        }

        if let Some(position_ref) = position_ref {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window_element_ref,
                    kAXPositionAttribute(),
                    as_cftype(&*position_ref),
                )
            };
            drop(position_ref);
        }

        // NOTE(asmvik): Due to macOS constraints (visible screen-area), we might need to resize the window *after* moving it.
        if let Some(size_ref) = size_ref {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window_element_ref,
                    kAXSizeAttribute(),
                    as_cftype(&*size_ref),
                )
            };
            drop(size_ref);
        }
    });
}
