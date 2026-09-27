#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::{NonNull, null_mut};
use std::sync::atomic::Ordering;

use crate::display_manager::DisplayManager;
use crate::event_loop::{Event, event_loop_post};
use crate::ffi::carbon_core::read_os_timer;
use crate::ffi::core_foundation::{
    CFMachPortCreateRunLoopSource, CFMachPortInvalidate, CFRetained, CFRunLoopAddSource,
    CFRunLoopGetMain, CFRunLoopRemoveSource, CGPoint, CGRect, CGSize, SendCFRetained,
    kCFRunLoopCommonModes,
};
use crate::ffi::core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventGetFlags, CGEventGetIntegerValueField, CGEventMask,
    CGEventTapCreate, CGEventTapEnable, CGEventTapIsEnabled, CGEventTapPostEvent, CGEventTapProxy,
    CGEventType, CGRectContainsPoint, kCGEventTapOptionDefault, kCGHIDEventTap,
    kCGHeadInsertEventTap,
};
use crate::globals::{LAST_GESTURE_TIME, MOUSE_TAP_STATE, MouseTapState, PENDING_GESTURE};
use crate::handles::{NodeId, SpaceId, WindowId};
use crate::misc::helpers::triangle_contains_point;
use crate::misc::macros::{LAYER_BELOW, ResizeHandle};
use crate::sa::scripting_addition_order_window;
use crate::space_manager::{
    SpaceManager, space_manager_tile_window_on_space, space_manager_untile_window,
};
use crate::state::MouseDragState;
use crate::view::{
    NODE_MAX_WINDOW_COUNT, ViewType, WindowCapture, WindowNodeChild, WindowNodeSplit,
    view_add_window_node_with_insertion_point, view_find_window_node, view_remove_window_node,
    view_stack_window_node, window_node_capture_windows, window_node_contains_window,
    window_node_flush, window_node_swap_window_list,
};
use crate::window_manager::{
    WindowManager, WindowOpError, window_manager_add_managed_window, window_manager_adjust_layer,
    window_manager_animate_window, window_manager_animate_window_list, window_manager_find_window,
    window_manager_purify_window, window_manager_remove_managed_window,
    window_manager_resize_window_relative,
};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum MouseDropAction {
    None = 0,
    Stack = 1,
    Swap = 2,
    WarpTop = 3,
    WarpRight = 4,
    WarpBottom = 5,
    WarpLeft = 6,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct MouseMod(pub u8);

impl MouseMod {
    pub(crate) const ALT: MouseMod = MouseMod(0x02);
    pub(crate) const SHIFT: MouseMod = MouseMod(0x04);
    pub(crate) const CMD: MouseMod = MouseMod(0x08);
    pub(crate) const CTRL: MouseMod = MouseMod(0x10);
    pub(crate) const FN: MouseMod = MouseMod(0x20);
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub(crate) enum MouseMode {
    #[default]
    None = 0,
    Move = 1,
    Resize = 2,
    Swap = 3,
    Stack = 4,
}

impl MouseMode {
    pub(crate) fn from_discriminant(discriminant: u8) -> MouseMode {
        match discriminant {
            1 => MouseMode::Move,
            2 => MouseMode::Resize,
            3 => MouseMode::Swap,
            4 => MouseMode::Stack,
            _ => MouseMode::None,
        }
    }
}

#[derive(Default)]
pub(crate) struct MouseWindowInfo {
    pub delta_x: f32,
    pub delta_y: f32,
    pub delta_width: f32,
    pub delta_height: f32,
    pub changed_x: bool,
    pub changed_y: bool,
    pub changed_width: bool,
    pub changed_height: bool,
    pub changed_position: bool,
    pub changed_size: bool,
}

pub(crate) const MOUSE_EVENT_MASK_FFM: u32 = (1 << CGEventType::MouseMoved.0)
    | (1 << CGEventType::LeftMouseDown.0)
    | (1 << CGEventType::LeftMouseUp.0)
    | (1 << CGEventType::LeftMouseDragged.0)
    | (1 << CGEventType::RightMouseDown.0)
    | (1 << CGEventType::RightMouseUp.0)
    | (1 << CGEventType::RightMouseDragged.0)
    | (1 << /* kCGSEventDockControl */ 30);

pub(crate) const MOUSE_EVENT_MASK: u32 = (1 << CGEventType::LeftMouseDown.0)
    | (1 << CGEventType::LeftMouseUp.0)
    | (1 << CGEventType::LeftMouseDragged.0)
    | (1 << CGEventType::RightMouseDown.0)
    | (1 << CGEventType::RightMouseUp.0)
    | (1 << CGEventType::RightMouseDragged.0)
    | (1 << /* kCGSEventDockControl */ 30);

pub(crate) static MOUSE_MOD_STR: [Option<&str>; 33] = [
    None,
    Some("none"),
    Some("alt"),
    None,
    Some("shift"),
    None,
    None,
    None,
    Some("cmd"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("ctrl"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("fn"),
];

pub(crate) static MOUSE_MODE_STR: [&str; 5] = ["none", "move", "resize", "swap", "stack"];

pub(crate) fn mouse_mod_from_cgflags(core_graphics_event_flags: u32) -> MouseMod {
    let mut flags: u8 = 0;

    if (core_graphics_event_flags as u64 & CGEventFlags::MaskAlternate.0) == CGEventFlags::MaskAlternate.0 {
        flags |= MouseMod::ALT.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskShift.0) == CGEventFlags::MaskShift.0 {
        flags |= MouseMod::SHIFT.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskCommand.0) == CGEventFlags::MaskCommand.0 {
        flags |= MouseMod::CMD.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskControl.0) == CGEventFlags::MaskControl.0 {
        flags |= MouseMod::CTRL.0;
    }
    if (core_graphics_event_flags as u64 & CGEventFlags::MaskSecondaryFn.0) == CGEventFlags::MaskSecondaryFn.0 {
        flags |= MouseMod::FN.0;
    }

    MouseMod(flags)
}

pub(crate) unsafe extern "C-unwind" fn mouse_handler(
    proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    context: *mut c_void,
) -> *mut CGEvent {
    let mouse_state = unsafe { &*(context as *const MouseTapState) };

    match event_type {
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
            let handle = mouse_state.handle.load(Ordering::Relaxed);
            if let Some(handle) = unsafe { handle.as_ref() } {
                CGEventTapEnable(handle, true);
            }
        }
        CGEventType::LeftMouseDown | CGEventType::RightMouseDown => {
            let event_modifier =
                mouse_mod_from_cgflags(CGEventGetFlags(Some(unsafe { event.as_ref() })).0 as u32);
            event_loop_post(Event::MouseDown {
                event: SendCFRetained(unsafe { CFRetained::retain(event) }),
                event_modifier,
            });

            if event_modifier.0 == mouse_state.modifier.load(Ordering::Relaxed) {
                mouse_state
                    .consume_mouse_click
                    .store(true, Ordering::Relaxed);
                let consumed_event = CFRetained::into_raw(unsafe { CFRetained::retain(event) });
                let previous_consumed_event = mouse_state
                    .consumed_event
                    .swap(consumed_event.as_ptr(), Ordering::Relaxed);
                if let Some(previous_consumed_event) = NonNull::new(previous_consumed_event) {
                    drop(unsafe { CFRetained::from_raw(previous_consumed_event) });
                }
                return null_mut();
            }
        }
        CGEventType::LeftMouseUp | CGEventType::RightMouseUp => {
            event_loop_post(Event::MouseUp {
                event: SendCFRetained(unsafe { CFRetained::retain(event) }),
            });

            if mouse_state.consume_mouse_click.load(Ordering::Relaxed) {
                let consumed_event = mouse_state
                    .consumed_event
                    .swap(null_mut(), Ordering::Relaxed);
                if !mouse_state.drag_detected.load(Ordering::Relaxed) {
                    unsafe { CGEventTapPostEvent(proxy, consumed_event.as_ref()) };
                    unsafe { CGEventTapPostEvent(proxy, Some(event.as_ref())) };
                }

                mouse_state.drag_detected.store(false, Ordering::Relaxed);
                mouse_state
                    .consume_mouse_click
                    .store(false, Ordering::Relaxed);
                if let Some(consumed_event) = NonNull::new(consumed_event) {
                    drop(unsafe { CFRetained::from_raw(consumed_event) });
                }
                return null_mut();
            }
        }
        CGEventType::LeftMouseDragged | CGEventType::RightMouseDragged => {
            mouse_state.drag_detected.store(true, Ordering::Relaxed);
            event_loop_post(Event::MouseDragged {
                event: SendCFRetained(unsafe { CFRetained::retain(event) }),
            });
        }
        CGEventType::MouseMoved => {
            let event_modifier =
                mouse_mod_from_cgflags(CGEventGetFlags(Some(unsafe { event.as_ref() })).0 as u32);
            if event_modifier.0 == mouse_state.modifier.load(Ordering::Relaxed) {
                return event.as_ptr();
            }

            event_loop_post(Event::MouseMoved {
                event: SendCFRetained(unsafe { CFRetained::retain(event) }),
                event_modifier,
            });
        }
        CGEventType(/* kCGSEventDockControl */ 30) => {
            let gesture_type = CGEventGetIntegerValueField(
                Some(unsafe { event.as_ref() }),
                CGEventField(/* kCGEventGestureHIDType */ 110),
            ) as i32;
            if gesture_type == /* kIOHIDEventTypeDockSwipe */ 23 {
                let motion = CGEventGetIntegerValueField(
                    Some(unsafe { event.as_ref() }),
                    CGEventField(/* kCGEventGestureSwipeMotion */ 123),
                ) as i32;
                if motion == /* kCGGestureMotionHorizontal */ 1 {
                    let phase = CGEventGetIntegerValueField(
                        Some(unsafe { event.as_ref() }),
                        CGEventField(/* kCGEventGesturePhase */ 132),
                    ) as i32;
                    if phase == /* kCGSGesturePhaseBegan */ 1 {
                        PENDING_GESTURE.store(true, Ordering::Release);
                    } else if phase == /* kCGSGesturePhaseEnded */ 4
                        || phase == /* kCGSGesturePhaseCancelled */ 8
                    {
                        PENDING_GESTURE.store(false, Ordering::Release);
                        LAST_GESTURE_TIME.store(read_os_timer(), Ordering::Release);
                    }
                }
            }
        }
        _ => {}
    }

    event.as_ptr()
}

pub(crate) fn mouse_window_info_populate(
    mouse_drag_state: &mut MouseDragState,
    info: &mut MouseWindowInfo,
    window_manager: &mut WindowManager,
) {
    let Some(frame) = mouse_drag_state
        .window_id
        .and_then(|window_id| window_manager.window.find(&window_id))
        .map(|window| window.frame)
    else {
        return;
    };

    info.delta_x = (frame.origin.x - mouse_drag_state.window_frame.origin.x) as f32;
    info.delta_y = (frame.origin.y - mouse_drag_state.window_frame.origin.y) as f32;
    info.delta_width = (frame.size.width - mouse_drag_state.window_frame.size.width) as f32;
    info.delta_height = (frame.size.height - mouse_drag_state.window_frame.size.height) as f32;

    info.changed_x = info.delta_x != 0.0f32;
    info.changed_y = info.delta_y != 0.0f32;
    info.changed_width = info.delta_width != 0.0f32;
    info.changed_height = info.delta_height != 0.0f32;

    info.changed_position = info.changed_x || info.changed_y;
    info.changed_size = info.changed_width || info.changed_height;
}

pub(crate) fn mouse_determine_drop_action(
    source_space_id: SpaceId,
    source_node_id: NodeId,
    destination_window_id: WindowId,
    point: CGPoint,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> MouseDropAction {
    let Some(destination_window_frame) = window_manager
        .window
        .find(&destination_window_id)
        .map(|window| window.frame)
    else {
        return MouseDropAction::None;
    };
    let point_relative_to_frame_origin = CGPoint {
        x: point.x - destination_window_frame.origin.x,
        y: point.y - destination_window_frame.origin.y,
    };
    let center_rect = CGRect {
        origin: CGPoint {
            x: 0.25f32 as f64 * destination_window_frame.size.width,
            y: 0.25f32 as f64 * destination_window_frame.size.height,
        },
        size: CGSize {
            width: 0.50f32 as f64 * destination_window_frame.size.width,
            height: 0.50f32 as f64 * destination_window_frame.size.height,
        },
    };
    let top_triangle: [CGPoint; 3] = [
        CGPoint {
            x: 0.0f32 as f64,
            y: 0.0f32 as f64,
        },
        CGPoint {
            x: 0.5f32 as f64 * destination_window_frame.size.width,
            y: 0.5f32 as f64 * destination_window_frame.size.height,
        },
        CGPoint {
            x: destination_window_frame.size.width,
            y: 0.0f32 as f64,
        },
    ];
    let right_triangle: [CGPoint; 3] = [
        CGPoint {
            x: destination_window_frame.size.width,
            y: 0.0f32 as f64,
        },
        CGPoint {
            x: 0.5f32 as f64 * destination_window_frame.size.width,
            y: 0.5f32 as f64 * destination_window_frame.size.height,
        },
        CGPoint {
            x: destination_window_frame.size.width,
            y: destination_window_frame.size.height,
        },
    ];
    let bottom_triangle: [CGPoint; 3] = [
        CGPoint {
            x: destination_window_frame.size.width,
            y: destination_window_frame.size.height,
        },
        CGPoint {
            x: 0.5f32 as f64 * destination_window_frame.size.width,
            y: 0.5f32 as f64 * destination_window_frame.size.height,
        },
        CGPoint {
            x: 0.0f32 as f64,
            y: destination_window_frame.size.height,
        },
    ];
    let left_triangle: [CGPoint; 3] = [
        CGPoint {
            x: 0.0f32 as f64,
            y: destination_window_frame.size.height,
        },
        CGPoint {
            x: 0.5f32 as f64 * destination_window_frame.size.width,
            y: 0.5f32 as f64 * destination_window_frame.size.height,
        },
        CGPoint {
            x: 0.0f32 as f64,
            y: 0.0f32 as f64,
        },
    ];

    let source_node_window_count = space_manager
        .view
        .find(&source_space_id)
        .and_then(|view| view.find_node(source_node_id))
        .map(|node| node.window_count);

    if (CGRectContainsPoint(center_rect, point_relative_to_frame_origin))
        && (source_node_window_count == Some(1))
    {
        return if MOUSE_TAP_STATE.drop_action.load(Ordering::Relaxed) == MouseMode::Stack as u8 {
            MouseDropAction::Stack
        } else {
            MouseDropAction::Swap
        };
    } else if triangle_contains_point(&top_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpTop;
    } else if triangle_contains_point(&right_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpRight;
    } else if triangle_contains_point(&bottom_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpBottom;
    } else if triangle_contains_point(&left_triangle, point_relative_to_frame_origin) {
        return MouseDropAction::WarpLeft;
    }

    MouseDropAction::None
}

pub(crate) fn mouse_drop_action_stack(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    source_space_id: SpaceId,
    source_window_id: WindowId,
    destination_space_id: SpaceId,
    destination_window_id: WindowId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    space_manager_untile_window(
        space_manager,
        source_space_id,
        source_window_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    );
    window_manager_remove_managed_window(window_manager, source_window_id);

    let destination_node =
        view_find_window_node(space_manager, destination_space_id, destination_window_id);
    let Some(destination_node) = destination_node else {
        return;
    };
    let Some(destination_node_window_count) = space_manager
        .view
        .find(&destination_space_id)
        .and_then(|view| view.find_node(destination_node))
        .map(|node| node.window_count)
    else {
        return;
    };
    if destination_node_window_count + 1 < NODE_MAX_WINDOW_COUNT as i32 {
        view_stack_window_node(
            destination_space_id,
            destination_node,
            source_window_id,
            space_manager,
        );
        window_manager_add_managed_window(
            window_manager,
            source_window_id,
            space_manager,
            destination_space_id,
        );
        window_manager_adjust_layer(source_window_id, LAYER_BELOW, window_manager);

        let Some(view) = space_manager.view.find(&destination_space_id) else {
            return;
        };
        let Some(node) = view.find_node(destination_node) else {
            return;
        };
        scripting_addition_order_window(source_window_id, 1, node.window_order[1]);

        let area = match node.zoom.and_then(|zoom| view.find_node(zoom)) {
            Some(zoom) => zoom.area,
            None => node.area,
        };
        window_manager_animate_window(
            WindowCapture {
                window_id: source_window_id,
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height,
            },
            window_manager,
        );
    }
}

pub(crate) fn mouse_drop_action_swap(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    source_space_id: SpaceId,
    source_node_id: NodeId,
    source_window_id: WindowId,
    destination_space_id: SpaceId,
    destination_node_id: NodeId,
    destination_window_id: WindowId,
) {
    let source_view_insertion_point = space_manager
        .view
        .find(&source_space_id)
        .map_or(WindowId(0), |view| view.insertion_point);
    let destination_view_insertion_point = space_manager
        .view
        .find(&destination_space_id)
        .map_or(WindowId(0), |view| view.insertion_point);
    if window_node_contains_window(
        source_space_id,
        source_node_id,
        source_view_insertion_point,
        space_manager,
    ) {
        if let Some(source_view) = space_manager.view.find_mut(&source_space_id) {
            source_view.insertion_point = destination_window_id;
        }
    } else if window_node_contains_window(
        destination_space_id,
        destination_node_id,
        destination_view_insertion_point,
        space_manager,
    ) {
        if let Some(destination_view) = space_manager.view.find_mut(&destination_space_id) {
            destination_view.insertion_point = source_window_id;
        }
    }

    window_node_swap_window_list(
        source_space_id,
        source_node_id,
        destination_space_id,
        destination_node_id,
        space_manager,
    );

    if source_space_id != destination_space_id {
        let source_node_window_list: Vec<WindowId> = space_manager
            .view
            .find(&source_space_id)
            .and_then(|view| view.find_node(source_node_id))
            .map_or(Vec::new(), |node| {
                node.window_list[..node.window_count as usize].to_vec()
            });
        for index in 0..source_node_window_list.len() {
            window_manager_remove_managed_window(window_manager, source_node_window_list[index]);
            if let Some(window) =
                window_manager_find_window(window_manager, source_node_window_list[index])
            {
                window_manager_add_managed_window(
                    window_manager,
                    window,
                    space_manager,
                    source_space_id,
                );
            }
        }

        let destination_node_window_list: Vec<WindowId> = space_manager
            .view
            .find(&destination_space_id)
            .and_then(|view| view.find_node(destination_node_id))
            .map_or(Vec::new(), |node| {
                node.window_list[..node.window_count as usize].to_vec()
            });
        for index in 0..destination_node_window_list.len() {
            window_manager_remove_managed_window(
                window_manager,
                destination_node_window_list[index],
            );
            if let Some(window) =
                window_manager_find_window(window_manager, destination_node_window_list[index])
            {
                window_manager_add_managed_window(
                    window_manager,
                    window,
                    space_manager,
                    destination_space_id,
                );
            }
        }
    }

    let mut window_list: Vec<WindowCapture> = Vec::new();
    window_node_capture_windows(
        source_space_id,
        source_node_id,
        &mut window_list,
        window_manager,
        space_manager,
    );
    window_node_capture_windows(
        destination_space_id,
        destination_node_id,
        &mut window_list,
        window_manager,
        space_manager,
    );
    window_manager_animate_window_list(&window_list, window_manager);
}

pub(crate) fn mouse_drop_action_warp(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    source_space_id: SpaceId,
    source_node_id: NodeId,
    source_window_id: WindowId,
    destination_space_id: SpaceId,
    destination_node_id: NodeId,
    destination_window_id: WindowId,
    split: WindowNodeSplit,
    child: WindowNodeChild,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let source_node = space_manager
        .view
        .find(&source_space_id)
        .and_then(|view| view.find_node(source_node_id))
        .map(|node| (node.parent, node.window_count));
    let destination_node_parent = space_manager
        .view
        .find(&destination_space_id)
        .and_then(|view| view.find_node(destination_node_id))
        .and_then(|node| node.parent);
    let Some((source_node_parent, source_node_window_count)) = source_node else {
        return;
    };

    if (source_node_parent.is_some() && destination_node_parent.is_some())
        && ((source_space_id, source_node_parent)
            == (destination_space_id, destination_node_parent))
        && (source_node_window_count == 1)
    {
        let Some(destination_node_parent) = destination_node_parent else {
            return;
        };
        let Some(destination_node_parent) = space_manager
            .view
            .find_mut(&destination_space_id)
            .and_then(|view| view.find_node_mut(destination_node_parent))
        else {
            return;
        };
        if destination_node_parent.split == split {
            mouse_drop_action_swap(
                window_manager,
                space_manager,
                source_space_id,
                source_node_id,
                source_window_id,
                destination_space_id,
                destination_node_id,
                destination_window_id,
            );
            return;
        } else {
            destination_node_parent.split = split;
            destination_node_parent.child = child;
        }
    } else if let Some(destination_node) = space_manager
        .view
        .find_mut(&destination_space_id)
        .and_then(|view| view.find_node_mut(destination_node_id))
    {
        destination_node.split = split;
        destination_node.child = child;
    }

    let source_node_remove = view_remove_window_node(
        space_manager,
        source_space_id,
        source_window_id,
        display_manager,
        window_manager,
        mouse_drag_state,
    );
    window_manager_remove_managed_window(window_manager, source_window_id);
    window_manager_purify_window(window_manager, source_window_id);

    let source_node_add = view_add_window_node_with_insertion_point(
        space_manager,
        destination_space_id,
        source_window_id,
        destination_window_id,
        display_manager,
        window_manager,
    );
    window_manager_add_managed_window(
        window_manager,
        source_window_id,
        space_manager,
        destination_space_id,
    );

    let mut window_list: Vec<WindowCapture> = Vec::new();

    if let Some(source_node_remove) = source_node_remove {
        window_node_capture_windows(
            source_space_id,
            source_node_remove,
            &mut window_list,
            window_manager,
            space_manager,
        );
    }

    if let Some(source_node_add) = source_node_add {
        let source_node_add_parent = space_manager
            .view
            .find(&destination_space_id)
            .and_then(|view| view.find_node(source_node_add))
            .and_then(|node| node.parent);
        let source_node_remove = source_node_remove.map(|node_id| (source_space_id, node_id));
        if source_node_remove != Some((destination_space_id, source_node_add))
            && source_node_remove
                != source_node_add_parent.map(|node_id| (destination_space_id, node_id))
        {
            window_node_capture_windows(
                destination_space_id,
                source_node_add,
                &mut window_list,
                window_manager,
                space_manager,
            );
        }
    }

    window_manager_animate_window_list(&window_list, window_manager);
}

pub(crate) fn mouse_drop_no_target(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    source_space_id: SpaceId,
    destination_space_id: SpaceId,
    window_id: WindowId,
    node_id: NodeId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if source_space_id == destination_space_id {
        window_node_flush(source_space_id, node_id, window_manager, space_manager);
    } else {
        space_manager_untile_window(
            space_manager,
            source_space_id,
            window_id,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        window_manager_remove_managed_window(window_manager, window_id);
        window_manager_purify_window(window_manager, window_id);

        let view = space_manager_tile_window_on_space(
            space_manager,
            window_id,
            destination_space_id,
            display_manager,
            window_manager,
        );
        window_manager_add_managed_window(window_manager, window_id, space_manager, view);
    }
}

pub(crate) fn mouse_drop_try_adjust_bsp_grid(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    info: &MouseWindowInfo,
    display_manager: &mut DisplayManager,
) {
    let mut success = true;

    'end: {
        let view_layout = space_manager.view.find(&space_id).map(|view| view.layout);
        if view_layout != Some(ViewType::Bsp) {
            success = false;
            break 'end;
        }

        if info.changed_position {
            let mut direction: u8 = 0;
            if info.changed_x {
                direction |= ResizeHandle::LEFT.0;
            }
            if info.changed_y {
                direction |= ResizeHandle::TOP.0;
            }
            if window_manager_resize_window_relative(
                window_manager,
                window_id,
                direction as i32,
                info.delta_x,
                info.delta_y,
                true,
                display_manager,
                space_manager,
            ) == WindowOpError::InvalidDstNode
            {
                success = false;
            }
        }

        if info.changed_size {
            let mut direction: u8 = 0;
            if info.changed_width && !info.changed_x {
                direction |= ResizeHandle::RIGHT.0;
            }
            if info.changed_height && !info.changed_y {
                direction |= ResizeHandle::BOTTOM.0;
            }
            if window_manager_resize_window_relative(
                window_manager,
                window_id,
                direction as i32,
                info.delta_width,
                info.delta_height,
                true,
                display_manager,
                space_manager,
            ) == WindowOpError::InvalidDstNode
            {
                success = false;
            }
        }
    }

    if !success {
        let node = view_find_window_node(space_manager, space_id, window_id);
        if let Some(node) = node {
            window_node_flush(space_id, node, window_manager, space_manager);
        }
    }
}

pub(crate) fn mouse_state_init() {
    MOUSE_TAP_STATE
        .modifier
        .store(MouseMod::FN.0, Ordering::Relaxed);
    MOUSE_TAP_STATE
        .action1
        .store(MouseMode::Move as u8, Ordering::Relaxed);
    MOUSE_TAP_STATE
        .action2
        .store(MouseMode::Resize as u8, Ordering::Relaxed);
    MOUSE_TAP_STATE
        .drop_action
        .store(MouseMode::Swap as u8, Ordering::Relaxed);
}

pub(crate) fn mouse_handler_begin(mask: u32) -> bool {
    let mouse_state = &MOUSE_TAP_STATE;

    if !mouse_state.handle.load(Ordering::Relaxed).is_null() {
        return true;
    }

    let handle = unsafe {
        CGEventTapCreate(
            kCGHIDEventTap,
            kCGHeadInsertEventTap,
            kCGEventTapOptionDefault,
            mask as CGEventMask,
            Some(mouse_handler),
            mouse_state as *const MouseTapState as *mut c_void,
        )
    };
    let Some(handle) = handle else {
        return false;
    };
    let handle = CFRetained::into_raw(handle);
    mouse_state.handle.store(handle.as_ptr(), Ordering::Release);

    if !CGEventTapIsEnabled(unsafe { handle.as_ref() }) {
        CFMachPortInvalidate(unsafe { handle.as_ref() });
        drop(unsafe { CFRetained::from_raw(handle) });
        mouse_state.handle.store(null_mut(), Ordering::Release);
        return false;
    }

    let runloop_source = CFMachPortCreateRunLoopSource(None, Some(unsafe { handle.as_ref() }), 0)
        .map_or(null_mut(), |runloop_source| {
            CFRetained::into_raw(runloop_source).as_ptr()
        });
    mouse_state
        .runloop_source
        .store(runloop_source, Ordering::Relaxed);
    if let Some(main_run_loop) = CFRunLoopGetMain() {
        CFRunLoopAddSource(&main_run_loop, unsafe { runloop_source.as_ref() }, unsafe {
            kCFRunLoopCommonModes
        });
    }

    true
}

pub(crate) fn mouse_handler_end() {
    let mouse_state = &MOUSE_TAP_STATE;

    let Some(handle) = NonNull::new(mouse_state.handle.load(Ordering::Relaxed)) else {
        return;
    };

    CGEventTapEnable(unsafe { handle.as_ref() }, false);
    CFMachPortInvalidate(unsafe { handle.as_ref() });
    let runloop_source = mouse_state.runloop_source.load(Ordering::Relaxed);
    if let Some(main_run_loop) = CFRunLoopGetMain() {
        CFRunLoopRemoveSource(&main_run_loop, unsafe { runloop_source.as_ref() }, unsafe {
            kCFRunLoopCommonModes
        });
    }
    if let Some(runloop_source) = NonNull::new(runloop_source) {
        drop(unsafe { CFRetained::from_raw(runloop_source) });
    }
    drop(unsafe { CFRetained::from_raw(handle) });
    mouse_state.handle.store(null_mut(), Ordering::Release);
}
