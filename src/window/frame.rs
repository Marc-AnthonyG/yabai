#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;

use crate::display::manager::DisplayManager;
use crate::ffi::accessibility::{
    AXUIElementSetAttributeValue, AXValueCreate, AXValueType, kAXPositionAttribute,
    kAXSizeAttribute, with_enhanced_user_interface_disabled,
};
use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize, as_cftype};
use crate::handles::{NodeId, WindowId};
use crate::layout::settings::ViewFlag;
use crate::layout::tree::{
    view_find_window_node, window_node_fence, window_node_flush, window_node_update,
};
use crate::layout::view::{view_flush, view_update};
use crate::space::managed_space::space_is_visible;
use crate::space::manager::SpaceManager;
use crate::support::arithmetic::{clampf_range, max};
use crate::support::direction::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST};
use crate::support::resize_handle::ResizeHandle;
use crate::support::type_of_change::{TYPE_ABS, TYPE_REL};
use crate::window::animation::{WindowCapture, window_manager_animate_window};
use crate::window::manager::{WindowManager, WindowOpError, window_manager_find_managed_window};
use crate::window::model::window_ax_frame;

pub(crate) fn window_manager_adjust_window_ratio(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    type_of_change: i32,
    ratio: f32,
    space_manager: &mut SpaceManager,
) -> WindowOpError {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return WindowOpError::InvalidSrcView;
    };

    let node_id = view_find_window_node(space_manager, space_id, window_id);
    let parent_node_id = node_id.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let Some(parent_node_id) = parent_node_id else {
        return WindowOpError::InvalidSrcNode;
    };

    match type_of_change {
        TYPE_REL => {
            if let Some(view) = space_manager.view.find_mut(&space_id)
                && let Some(parent_node) = view.find_node_mut(parent_node_id)
            {
                parent_node.ratio = clampf_range(parent_node.ratio + ratio, 0.1f32, 0.9f32);
            }
        }
        TYPE_ABS => {
            if let Some(view) = space_manager.view.find_mut(&space_id)
                && let Some(parent_node) = view.find_node_mut(parent_node_id)
            {
                parent_node.ratio = clampf_range(ratio, 0.1f32, 0.9f32);
            }
        }
        _ => {}
    }

    window_node_update(space_manager, space_id, parent_node_id, window_manager);

    if space_is_visible(space_id) {
        window_node_flush(space_id, parent_node_id, window_manager, space_manager);
    } else if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }

    WindowOpError::Success
}

pub(crate) fn window_manager_move_window_relative(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    type_of_change: i32,
    delta_x: f32,
    delta_y: f32,
) -> WindowOpError {
    let view = window_manager_find_managed_window(window_manager, window_id);
    if view.is_some() {
        return WindowOpError::InvalidSrcView;
    }

    let Some(window) = window_manager.window.find(&window_id) else {
        return WindowOpError::Success;
    };
    let window_frame = window.frame;

    let mut delta_x = delta_x;
    let mut delta_y = delta_y;

    if type_of_change == TYPE_REL {
        delta_x = (delta_x as f64 + window_frame.origin.x) as f32;
        delta_y = (delta_y as f64 + window_frame.origin.y) as f32;
    }

    window_manager_animate_window(
        WindowCapture {
            window_id,
            x: delta_x,
            y: delta_y,
            width: window_frame.size.width as f32,
            height: window_frame.size.height as f32,
        },
        window_manager,
    );
    WindowOpError::Success
}

pub(crate) fn window_manager_resize_window_relative_internal(
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

    let frame_width = max(
        1.0f64,
        frame.size.width + (delta_x * x_modifier as f32) as f64,
    ) as f32;
    let frame_height = max(
        1.0f64,
        frame.size.height + (delta_y * y_modifier as f32) as f64,
    ) as f32;
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
        window_manager_animate_window(
            WindowCapture {
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
            .find(&window_id)
            .and_then(|window| window.application)
        else {
            return;
        };
        let Some(application) = window_manager.application.find(&application_process_id) else {
            return;
        };
        let application_element_ref = application.element_ref;

        with_enhanced_user_interface_disabled(unsafe { &*application_element_ref }, || {
            window_manager_move_window(window_id, frame_x, frame_y, window_manager);
            window_manager_resize_window(window_id, frame_width, frame_height, window_manager);
        });
    }
}

pub(crate) fn window_manager_resize_window_relative(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    direction: i32,
    delta_x: f32,
    delta_y: f32,
    animate: bool,
    display_manager: &mut DisplayManager,
    space_manager: &mut SpaceManager,
) -> WindowOpError {
    let view = window_manager_find_managed_window(window_manager, window_id);
    if let Some(space_id) = view {
        if direction == ResizeHandle::ABS.0 as i32 {
            return WindowOpError::InvalidOperation;
        }

        let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
            return WindowOpError::InvalidSrcNode;
        };

        let mut x_fence: Option<NodeId> = None;
        let mut y_fence: Option<NodeId> = None;

        if direction & ResizeHandle::TOP.0 as i32 != 0 {
            x_fence = window_node_fence(space_id, node_id, DIR_NORTH, space_manager);
        }
        if direction & ResizeHandle::BOTTOM.0 as i32 != 0 {
            x_fence = window_node_fence(space_id, node_id, DIR_SOUTH, space_manager);
        }
        if direction & ResizeHandle::LEFT.0 as i32 != 0 {
            y_fence = window_node_fence(space_id, node_id, DIR_WEST, space_manager);
        }
        if direction & ResizeHandle::RIGHT.0 as i32 != 0 {
            y_fence = window_node_fence(space_id, node_id, DIR_EAST, space_manager);
        }
        if x_fence.is_none() && y_fence.is_none() {
            return WindowOpError::InvalidDstNode;
        }

        if let Some(y_fence) = y_fence
            && let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(y_fence_node) = view.find_node_mut(y_fence)
        {
            let scaled_ratio = y_fence_node.ratio + delta_x / y_fence_node.area.width;
            y_fence_node.ratio = clampf_range(scaled_ratio, 0.1f32, 0.9f32);
        }

        if let Some(x_fence) = x_fence
            && let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(x_fence_node) = view.find_node_mut(x_fence)
        {
            let scaled_ratio = x_fence_node.ratio + delta_y / x_fence_node.area.height;
            x_fence_node.ratio = clampf_range(scaled_ratio, 0.1f32, 0.9f32);
        }

        view_update(space_manager, space_id, display_manager, window_manager);
        view_flush(space_manager, space_id, window_manager);
    } else {
        if direction == ResizeHandle::ABS.0 as i32 {
            if animate {
                let Some(window) = window_manager.window.find(&window_id) else {
                    return WindowOpError::Success;
                };
                let window_frame = window.frame;
                window_manager_animate_window(
                    WindowCapture {
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
                    .find(&window_id)
                    .and_then(|window| window.application)
                else {
                    return WindowOpError::Success;
                };
                let Some(application) = window_manager.application.find(&application_process_id)
                else {
                    return WindowOpError::Success;
                };
                let application_element_ref = application.element_ref;

                with_enhanced_user_interface_disabled(unsafe { &*application_element_ref }, || {
                    window_manager_resize_window(window_id, delta_x, delta_y, window_manager);
                });
            }
        } else {
            let Some(window) = window_manager.window.find(&window_id) else {
                return WindowOpError::Success;
            };
            let frame = window_ax_frame(window);
            window_manager_resize_window_relative_internal(
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

    WindowOpError::Success
}

pub(crate) fn window_manager_move_window(
    window_id: WindowId,
    x: f32,
    y: f32,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
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

pub(crate) fn window_manager_resize_window(
    window_id: WindowId,
    width: f32,
    height: f32,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
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

pub(crate) fn window_manager_set_window_frame(
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

    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    let window_element_ref = window.element_ref;
    let Some(application) = window.application.and_then(|application_process_id| {
        window_manager.application.find(&application_process_id)
    }) else {
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
