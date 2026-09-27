#![allow(deprecated)]

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64};

use crate::display::display_bounds_constrained;
use crate::display_manager::{DisplayManager, display_manager_display_id_arrangement};
use crate::event_loop::update_window_notifications;
use crate::ffi::CFStringOwned;
use crate::ffi::core_foundation::{
    CFRetained, CFType, CGPoint, CGRect, CGSize, SendCFRetained, sls_window_disable_shadow,
    take_create_rule_result, ts_cfstring_copy,
};
use crate::ffi::core_graphics::{
    CGContext, CGContextAddPath, CGContextClearRect, CGContextClipToRect, CGContextFillRect,
    CGContextFlush, CGContextResetClip, CGContextSetLineWidth, CGContextSetRGBFillColor,
    CGContextSetRGBStrokeColor, CGContextStrokePath, CGImage, CGPathCreateWithRoundedRect,
    CGRectGetMidX, CGRectGetMidY, CGRectInset, CGRegionCreateEmptyRegion, CGSNewRegionWithRect,
};
use crate::ffi::skylight::{
    SLSDisableUpdate, SLSNewWindowWithOpaqueShapeAndContext, SLSOrderWindow, SLSReenableUpdate,
    SLSReleaseWindow, SLSSetWindowLevel, SLSSetWindowOpacity, SLSSetWindowResolution,
    SLSSetWindowShape, SLSSetWindowSubLevel, SLSSpaceCopyName, SLWindowContextCreate,
};
use crate::globals::CONNECTION;
use crate::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::misc::helpers::{
    AnimationEasingType, cgrect_clamp_x_radius, cgrect_clamp_y_radius, json_bool,
};
use crate::misc::macros::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST, STACK, in_range_ii};
use crate::misc::response::Response;
use crate::misc::table::Table;
use crate::space::{
    space_display_id, space_is_fullscreen, space_is_user, space_is_visible, space_window_list,
};
use crate::space_manager::{
    SpaceManager, space_manager_get_label_for_space, space_manager_mission_control_index,
};
use crate::state::MouseDragState;
use crate::window::{window_level, window_sub_level};
use crate::window_manager::{
    WindowManager, window_manager_animate_window_list, window_manager_find_rank_of_window_in_list,
    window_manager_find_window, window_manager_remove_managed_window,
};
use crate::workspace::{workspace_is_macos_sequoia, workspace_is_macos_tahoe};

macro_rules! space_property_list {
    ($space_property_entry:ident) => {
        $space_property_entry! {
            ("id", SPACE_PROPERTY_ID, 0x001),
            ("uuid", SPACE_PROPERTY_UUID, 0x002),
            ("index", SPACE_PROPERTY_INDEX, 0x004),
            ("label", SPACE_PROPERTY_LABEL, 0x008),
            ("type", SPACE_PROPERTY_TYPE, 0x010),
            ("display", SPACE_PROPERTY_DISPLAY, 0x020),
            ("windows", SPACE_PROPERTY_WINDOWS, 0x040),
            ("first-window", SPACE_PROPERTY_FIRST_WINDOW, 0x080),
            ("last-window", SPACE_PROPERTY_LAST_WINDOW, 0x100),
            ("has-focus", SPACE_PROPERTY_HAS_FOCUS, 0x200),
            ("is-visible", SPACE_PROPERTY_IS_VISIBLE, 0x400),
            ("is-native-fullscreen", SPACE_PROPERTY_IS_FULLSCREEN, 0x800),
        }
    };
}

macro_rules! define_space_property_list {
    ($(($name:literal, $identifier:ident, $value:literal)),* $(,)?) => {
        $(pub(crate) const $identifier: u64 = $value;)*

        pub(crate) static SPACE_PROPERTY_VAL: [u64; 12] = [$($value),*];

        pub(crate) static SPACE_PROPERTY_STR: [&str; 12] = [$($name),*];
    };
}

space_property_list!(define_space_property_list);

#[derive(Clone, Copy, Default)]
pub(crate) struct Area {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

pub(crate) struct WindowCapture {
    pub(crate) window_id: WindowId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

pub(crate) struct WindowProxy {
    pub(crate) id: AtomicU32,
    pub(crate) core_graphics_objects: Mutex<WindowProxyCoreGraphicsObjects>,
    pub(crate) target_x: AtomicU32,
    pub(crate) target_y: AtomicU32,
    pub(crate) target_width: AtomicU32,
    pub(crate) target_height: AtomicU32,
    pub(crate) frame_origin_x: AtomicU64,
    pub(crate) frame_origin_y: AtomicU64,
    pub(crate) frame_size_width: AtomicU64,
    pub(crate) frame_size_height: AtomicU64,
    pub(crate) level: AtomicI32,
    pub(crate) sub_level: AtomicI32,
}

pub(crate) struct WindowProxyCoreGraphicsObjects {
    pub(crate) context: Option<CFRetained<CGContext>>,
    pub(crate) image: Option<CFRetained<CGImage>>,
}

pub(crate) struct WindowAnimation {
    pub(crate) window_id: WindowId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) connection_id: i32,
    pub(crate) proxy: WindowProxy,
    pub(crate) skip: AtomicBool,
}

pub(crate) struct AnimationContext {
    pub(crate) animation_connection: i32,
    pub(crate) animation_easing: AnimationEasingType,
    pub(crate) animation_duration: f32,
    pub(crate) animation_clock: AtomicU64,
    pub(crate) animation_list: Vec<WindowAnimation>,
    pub(crate) animation_count: i32,
    pub(crate) window_animations_table: Arc<Mutex<Table<WindowId, (Arc<AnimationContext>, usize)>>>,
}

unsafe impl Send for AnimationContext {}
unsafe impl Sync for AnimationContext {}

#[derive(Clone, Copy)]
pub(crate) struct BalanceNode {
    pub(crate) y_count: i32,
    pub(crate) x_count: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum WindowInsertionPoint {
    Focused = 0,
    First = 1,
    Last = 2,
}

pub(crate) static WINDOW_INSERTION_POINT_STR: [&str; 3] = ["focused", "first", "last"];

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum WindowNodeChild {
    #[default]
    None = 0,
    Second = 1,
    First = 2,
}

pub(crate) static WINDOW_NODE_CHILD_STR: [&str; 3] = ["none", "second_child", "first_child"];

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub(crate) enum WindowNodeSplit {
    #[default]
    None = 0,
    Y = 1,
    X = 2,
    Auto = 3,
}

pub(crate) static WINDOW_NODE_SPLIT_STR: [&str; 4] = ["none", "vertical", "horizontal", "auto"];

pub(crate) static AUTO_BALANCE_STR: [&str; 4] = ["off", "vertical", "horizontal", "on"];

pub(crate) struct FeedbackWindow {
    pub(crate) id: WindowId,
    pub(crate) context: *mut CGContext,
}

impl Drop for FeedbackWindow {
    fn drop(&mut self) {
        let connection = *CONNECTION.get().unwrap();
        if self.id.0 != 0 {
            unsafe { SLSOrderWindow(connection, self.id.0, 0, 0) };
            drop(unsafe { take_create_rule_result(self.context.cast_const()) });
            unsafe { SLSReleaseWindow(connection, self.id.0) };
        } else {
            drop(unsafe { take_create_rule_result(self.context.cast_const()) });
        }
    }
}

impl FeedbackWindow {
    pub(crate) fn window_id_or_zero(feedback_window: &Option<FeedbackWindow>) -> u32 {
        feedback_window
            .as_ref()
            .map_or(0, |feedback_window| feedback_window.id.0)
    }
}

pub(crate) const NODE_MAX_WINDOW_COUNT: usize = 32;

#[derive(Default)]
pub(crate) struct WindowNode {
    pub(crate) area: Area,
    pub(crate) parent: Option<NodeId>,
    pub(crate) left: Option<NodeId>,
    pub(crate) right: Option<NodeId>,
    pub(crate) zoom: Option<NodeId>,
    pub(crate) window_list: [WindowId; NODE_MAX_WINDOW_COUNT],
    pub(crate) window_order: [WindowId; NODE_MAX_WINDOW_COUNT],
    pub(crate) window_count: i32,
    pub(crate) ratio: f32,
    pub(crate) split: WindowNodeSplit,
    pub(crate) child: WindowNodeChild,
    pub(crate) insert_direction: i32,
    pub(crate) feedback_window: Option<FeedbackWindow>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum ViewType {
    Default = 0,
    Bsp = 1,
    Stack = 2,
    Float = 3,
}

pub(crate) static VIEW_TYPE_STR: [&str; 4] = ["default", "bsp", "stack", "float"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViewFlag(pub u64);

impl ViewFlag {
    pub(crate) const LAYOUT: ViewFlag = ViewFlag(0x001);
    pub(crate) const TOP_PADDING: ViewFlag = ViewFlag(0x002);
    pub(crate) const BOTTOM_PADDING: ViewFlag = ViewFlag(0x004);
    pub(crate) const LEFT_PADDING: ViewFlag = ViewFlag(0x008);
    pub(crate) const RIGHT_PADDING: ViewFlag = ViewFlag(0x010);
    pub(crate) const WINDOW_GAP: ViewFlag = ViewFlag(0x020);
    pub(crate) const AUTO_BALANCE: ViewFlag = ViewFlag(0x040);
    pub(crate) const ENABLE_PADDING: ViewFlag = ViewFlag(0x080);
    pub(crate) const ENABLE_GAP: ViewFlag = ViewFlag(0x100);
    pub(crate) const IS_VALID: ViewFlag = ViewFlag(0x200);
    pub(crate) const IS_DIRTY: ViewFlag = ViewFlag(0x400);
    pub(crate) const SPLIT_TYPE: ViewFlag = ViewFlag(0x800);
}

pub(crate) struct View {
    pub(crate) uuid: Option<CFStringOwned>,
    pub(crate) space_id: SpaceId,
    pub(crate) nodes: Vec<Option<WindowNode>>,
    pub(crate) free_node_ids: Vec<NodeId>,
    pub(crate) insertion_point: WindowId,
    pub(crate) layout: ViewType,
    pub(crate) split_type: WindowNodeSplit,
    pub(crate) top_padding: i32,
    pub(crate) bottom_padding: i32,
    pub(crate) left_padding: i32,
    pub(crate) right_padding: i32,
    pub(crate) window_gap: i32,
    pub(crate) auto_balance: u32,
    pub(crate) flags: u64,
}

impl View {
    pub(crate) fn node(&self, node_id: NodeId) -> &WindowNode {
        self.nodes[node_id.0 as usize].as_ref().unwrap()
    }

    pub(crate) fn node_mut(&mut self, node_id: NodeId) -> &mut WindowNode {
        self.nodes[node_id.0 as usize].as_mut().unwrap()
    }

    pub(crate) fn find_node(&self, node_id: NodeId) -> Option<&WindowNode> {
        self.nodes
            .get(node_id.0 as usize)
            .and_then(|slot| slot.as_ref())
    }

    pub(crate) fn find_node_mut(&mut self, node_id: NodeId) -> Option<&mut WindowNode> {
        self.nodes
            .get_mut(node_id.0 as usize)
            .and_then(|slot| slot.as_mut())
    }

    pub(crate) fn allocate_node(&mut self) -> NodeId {
        match self.free_node_ids.pop() {
            Some(node_id) => {
                self.nodes[node_id.0 as usize] = Some(WindowNode::default());
                node_id
            }
            None => {
                self.nodes.push(Some(WindowNode::default()));
                NodeId((self.nodes.len() - 1) as u32)
            }
        }
    }

    pub(crate) fn check_flag(&self, flag: ViewFlag) -> bool {
        (self.flags & flag.0) != 0
    }

    pub(crate) fn clear_flag(&mut self, flag: ViewFlag) {
        self.flags &= !flag.0;
    }

    pub(crate) fn set_flag(&mut self, flag: ViewFlag) {
        self.flags |= flag.0;
    }
}

pub(crate) const INSERT_FEEDBACK_WIDTH: f64 = 2.0;
pub(crate) const INSERT_FEEDBACK_RADIUS: f64 = 9.0;

pub(crate) fn insert_feedback_show(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let connection = *CONNECTION.get().unwrap();

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);

    let mut frame = CGRect {
        origin: CGPoint {
            x: node.area.x as f64,
            y: node.area.y as f64,
        },
        size: CGSize {
            width: node.area.width as f64,
            height: node.area.height as f64,
        },
    };
    let mut frame_region: *mut CFType = std::ptr::null_mut();
    unsafe { CGSNewRegionWithRect(&mut frame, &mut frame_region) };
    frame.origin.x = 0.0;
    frame.origin.y = 0.0;

    if FeedbackWindow::window_id_or_zero(&node.feedback_window) == 0 {
        let mut tags: u64 = (1u64 << 1) | (1u64 << 9);
        let empty_region = unsafe { CGRegionCreateEmptyRegion() };
        let mut feedback_window_id: u32 = 0;
        unsafe {
            SLSNewWindowWithOpaqueShapeAndContext(
                connection,
                2,
                frame_region.cast_const(),
                empty_region.cast_const(),
                13,
                &mut tags,
                0.0,
                0.0,
                64,
                &mut feedback_window_id,
                std::ptr::null_mut(),
            )
        };
        drop(unsafe { take_create_rule_result(empty_region.cast_const()) });

        sls_window_disable_shadow(feedback_window_id);
        unsafe { SLSSetWindowResolution(connection, feedback_window_id, 1.0f32 as f64) };
        unsafe { SLSSetWindowOpacity(connection, feedback_window_id, false) };
        unsafe {
            SLSSetWindowLevel(
                connection,
                feedback_window_id,
                window_level(node.window_order[0]),
            )
        };
        unsafe {
            SLSSetWindowSubLevel(
                connection,
                feedback_window_id,
                window_sub_level(node.window_order[0]),
            )
        };
        let feedback_window_context =
            unsafe { SLWindowContextCreate(connection, feedback_window_id, std::ptr::null()) };
        node.feedback_window = Some(FeedbackWindow {
            id: WindowId(feedback_window_id),
            context: feedback_window_context,
        });
        let feedback_window_context = unsafe { feedback_window_context.as_ref() };
        CGContextSetLineWidth(feedback_window_context, INSERT_FEEDBACK_WIDTH);
        CGContextSetRGBFillColor(
            feedback_window_context,
            window_manager.insert_feedback_color.red as f64,
            window_manager.insert_feedback_color.green as f64,
            window_manager.insert_feedback_color.blue as f64,
            (window_manager.insert_feedback_color.alpha * 0.25f32) as f64,
        );
        CGContextSetRGBStrokeColor(
            feedback_window_context,
            window_manager.insert_feedback_color.red as f64,
            window_manager.insert_feedback_color.green as f64,
            window_manager.insert_feedback_color.blue as f64,
            window_manager.insert_feedback_color.alpha as f64,
        );
        unsafe { SLSDisableUpdate(connection) };
        CGContextClearRect(feedback_window_context, frame);
        CGContextFlush(feedback_window_context);
        unsafe { SLSReenableUpdate(connection) };
        unsafe { SLSOrderWindow(connection, feedback_window_id, 1, node.window_order[0].0) };
        window_manager
            .insert_feedback
            .add(node.window_order[0], (space_id, node_id));
        if !workspace_is_macos_sequoia() && !workspace_is_macos_tahoe() {
            update_window_notifications(window_manager, space_manager);
        }
    }

    let Some(node) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
    else {
        drop(unsafe { take_create_rule_result(frame_region.cast_const()) });
        return;
    };
    let (feedback_window_id, feedback_window_context) = match &node.feedback_window {
        Some(feedback_window) => (feedback_window.id, feedback_window.context),
        None => (WindowId(0), std::ptr::null_mut()),
    };
    let insert_direction = node.insert_direction;

    let clip_x: f64;
    let clip_y: f64;
    let clip_width: f64;
    let clip_height: f64;
    let middle_x = CGRectGetMidX(frame);
    let middle_y = CGRectGetMidY(frame);

    match insert_direction {
        DIR_NORTH => {
            clip_x = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = middle_y - 0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = INSERT_FEEDBACK_WIDTH;
            clip_height = INSERT_FEEDBACK_WIDTH;
        }
        DIR_EAST => {
            clip_x = middle_x - 0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = INSERT_FEEDBACK_WIDTH;
            clip_height = INSERT_FEEDBACK_WIDTH;
        }
        DIR_SOUTH => {
            clip_x = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = INSERT_FEEDBACK_WIDTH;
            clip_height = -middle_y + INSERT_FEEDBACK_WIDTH;
        }
        DIR_WEST => {
            clip_x = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = -middle_x + INSERT_FEEDBACK_WIDTH;
            clip_height = INSERT_FEEDBACK_WIDTH;
        }
        STACK => {
            clip_x = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_y = -0.5 * INSERT_FEEDBACK_WIDTH;
            clip_width = INSERT_FEEDBACK_WIDTH;
            clip_height = INSERT_FEEDBACK_WIDTH;
        }
        _ => unreachable!(),
    }

    let rect = CGRect {
        origin: CGPoint {
            x: 0.5 * INSERT_FEEDBACK_WIDTH,
            y: 0.5 * INSERT_FEEDBACK_WIDTH,
        },
        size: CGSize {
            width: frame.size.width - INSERT_FEEDBACK_WIDTH,
            height: frame.size.height - INSERT_FEEDBACK_WIDTH,
        },
    };
    let fill = CGRectInset(
        rect,
        0.5 * INSERT_FEEDBACK_WIDTH,
        0.5 * INSERT_FEEDBACK_WIDTH,
    );
    let clip = CGRect {
        origin: CGPoint {
            x: rect.origin.x + clip_x,
            y: rect.origin.y + clip_y,
        },
        size: CGSize {
            width: rect.size.width + clip_width,
            height: rect.size.height + clip_height,
        },
    };
    let path = unsafe {
        CGPathCreateWithRoundedRect(
            rect,
            cgrect_clamp_x_radius(rect, INSERT_FEEDBACK_RADIUS as f32) as f64,
            cgrect_clamp_y_radius(rect, INSERT_FEEDBACK_RADIUS as f32) as f64,
            std::ptr::null(),
        )
    };

    let feedback_window_context = unsafe { feedback_window_context.as_ref() };
    unsafe { SLSDisableUpdate(connection) };
    unsafe {
        SLSSetWindowShape(
            connection,
            feedback_window_id.0,
            0.0,
            0.0,
            frame_region.cast_const(),
        )
    };
    CGContextClearRect(feedback_window_context, frame);
    CGContextClipToRect(feedback_window_context, clip);
    CGContextFillRect(feedback_window_context, fill);
    CGContextAddPath(feedback_window_context, Some(&path));
    CGContextStrokePath(feedback_window_context);
    if let Some(feedback_window_context) = feedback_window_context {
        CGContextResetClip(feedback_window_context);
    }
    CGContextFlush(feedback_window_context);
    unsafe { SLSReenableUpdate(connection) };
    drop(path);
    drop(unsafe { take_create_rule_result(frame_region.cast_const()) });
}

pub(crate) fn insert_feedback_destroy(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let Some(node) = view.find_node_mut(node_id) else {
        return;
    };

    if FeedbackWindow::window_id_or_zero(&node.feedback_window) != 0 {
        window_manager.insert_feedback.remove(&node.window_order[0]);

        if !workspace_is_macos_sequoia() && !workspace_is_macos_tahoe() {
            update_window_notifications(window_manager, space_manager);
        }

        let Some(node) = space_manager
            .view
            .find_mut(&space_id)
            .and_then(|view| view.find_node_mut(node_id))
        else {
            return;
        };
        drop(node.feedback_window.take());
    }
}

pub(crate) fn area_from_cgrect(rect: CGRect) -> Area {
    Area {
        x: rect.origin.x as f32,
        y: rect.origin.y as f32,
        width: rect.size.width as f32,
        height: rect.size.height as f32,
    }
}

pub(crate) fn area_max_point(area: Area) -> CGPoint {
    CGPoint {
        x: (area.x + area.width - 1.0f32) as f64,
        y: (area.y + area.height - 1.0f32) as f64,
    }
}

pub(crate) fn window_node_get_child(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> WindowNodeChild {
    let child = match space_manager.view.find(&space_id) {
        Some(view) => view.node(node_id).child,
        None => WindowNodeChild::None,
    };

    if child != WindowNodeChild::None {
        child
    } else {
        space_manager.window_placement
    }
}

pub(crate) fn window_node_get_split(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
) -> WindowNodeSplit {
    let Some(view) = space_manager.view.find(&space_id) else {
        return WindowNodeSplit::None;
    };
    let view_split_type = view.split_type;
    let node = view.node(node_id);
    let node_split = node.split;
    let node_area = node.area;

    if node_split != WindowNodeSplit::None {
        return node_split;
    }

    if view_split_type != WindowNodeSplit::None {
        if view_split_type != WindowNodeSplit::Auto {
            return view_split_type;
        }
    } else if space_manager.split_type != WindowNodeSplit::Auto {
        return space_manager.split_type;
    }

    if node_area.width >= node_area.height {
        WindowNodeSplit::Y
    } else {
        WindowNodeSplit::X
    }
}

pub(crate) fn window_node_get_ratio(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> f32 {
    let ratio = match space_manager.view.find(&space_id) {
        Some(view) => view.node(node_id).ratio,
        None => return space_manager.split_ratio,
    };

    if in_range_ii(ratio, 0.1f32, 0.9f32) {
        ratio
    } else {
        space_manager.split_ratio
    }
}

pub(crate) fn window_node_get_gap(space_manager: &mut SpaceManager, space_id: SpaceId) -> i32 {
    let Some(view) = space_manager.view.find(&space_id) else {
        return 0;
    };

    if view.check_flag(ViewFlag::ENABLE_GAP) {
        view.window_gap
    } else {
        0
    }
}

pub(crate) fn area_make_pair(
    split: WindowNodeSplit,
    gap: i32,
    ratio: f32,
    parent_area: Area,
) -> (Area, Area) {
    if split == WindowNodeSplit::Y {
        let mut left_area = parent_area;
        let mut right_area = parent_area;

        let left_width = (parent_area.width - gap as f32) * ratio;
        let right_width = (parent_area.width - gap as f32) * (1.0f32 - ratio);

        left_area.width = left_width as i32 as f32;
        right_area.width = right_width as i32 as f32;
        right_area.x += ((left_width + 0.5f32) as i32 + gap) as f32;

        (left_area, right_area)
    } else {
        let mut left_area = parent_area;
        let mut right_area = parent_area;

        let left_width = (parent_area.height - gap as f32) * ratio;
        let right_width = (parent_area.height - gap as f32) * (1.0f32 - ratio);

        left_area.height = left_width as i32 as f32;
        right_area.height = right_width as i32 as f32;
        right_area.y += ((left_width + 0.5f32) as i32 + gap) as f32;

        (left_area, right_area)
    }
}

pub(crate) fn area_make_pair_for_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
) {
    let split = window_node_get_split(space_manager, space_id, node_id);
    let ratio = window_node_get_ratio(space_id, node_id, space_manager);
    let gap = window_node_get_gap(space_manager, space_id);

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node(node_id);
    let parent_area = node.area;
    let left = node.left;
    let right = node.right;

    let (left_area, right_area) = area_make_pair(split, gap, ratio, parent_area);

    if let Some(left) = left {
        view.node_mut(left).area = left_area;
    }
    if let Some(right) = right {
        view.node_mut(right).area = right_area;
    }

    let node = view.node_mut(node_id);
    node.split = split;
    node.ratio = ratio;
}

pub(crate) fn window_node_is_occupied(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    view.node(node_id).window_count != 0
}

pub(crate) fn window_node_is_intermediate(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    view.node(node_id).parent.is_some()
}

pub(crate) fn window_node_is_leaf(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    let node = view.node(node_id);

    node.left.is_none() && node.right.is_none()
}

pub(crate) fn window_node_is_left_child(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    match view.node(node_id).parent {
        Some(parent) => view.node(parent).left == Some(node_id),
        None => false,
    }
}

pub(crate) fn window_node_is_right_child(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    match view.node(node_id).parent {
        Some(parent) => view.node(parent).right == Some(node_id),
        None => false,
    }
}

pub(crate) fn window_node_equalize(
    space_id: SpaceId,
    node_id: NodeId,
    axis_flag: u32,
    space_manager: &mut SpaceManager,
) {
    let (left, right) = match space_manager.view.find(&space_id) {
        Some(view) => {
            let node = view.node(node_id);
            (node.left, node.right)
        }
        None => return,
    };

    if let Some(left) = left {
        window_node_equalize(space_id, left, axis_flag, space_manager);
    }
    if let Some(right) = right {
        window_node_equalize(space_id, right, axis_flag, space_manager);
    }

    let split_ratio = space_manager.split_ratio;
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);

    if (axis_flag & WindowNodeSplit::Y as u32) != 0 && node.split == WindowNodeSplit::Y {
        node.ratio = split_ratio;
    }

    if (axis_flag & WindowNodeSplit::X as u32) != 0 && node.split == WindowNodeSplit::X {
        node.ratio = split_ratio;
    }
}

pub(crate) fn balance_node_add(first: BalanceNode, second: BalanceNode) -> BalanceNode {
    BalanceNode {
        y_count: first.y_count + second.y_count,
        x_count: first.x_count + second.x_count,
    }
}

pub(crate) fn window_node_balance(
    space_id: SpaceId,
    node_id: NodeId,
    axis_flag: u32,
    space_manager: &mut SpaceManager,
) -> BalanceNode {
    if window_node_is_leaf(space_id, node_id, space_manager) {
        let Some(view) = space_manager.view.find(&space_id) else {
            return BalanceNode {
                y_count: 0,
                x_count: 0,
            };
        };
        let parent = view.node(node_id).parent;
        return BalanceNode {
            y_count: match parent {
                Some(parent) => (view.node(parent).split == WindowNodeSplit::Y) as i32,
                None => 0,
            },
            x_count: match parent {
                Some(parent) => (view.node(parent).split == WindowNodeSplit::X) as i32,
                None => 0,
            },
        };
    }

    let (left, right) = match space_manager.view.find(&space_id) {
        Some(view) => {
            let node = view.node(node_id);
            (node.left, node.right)
        }
        None => {
            return BalanceNode {
                y_count: 0,
                x_count: 0,
            };
        }
    };

    let left_leafs = match left {
        Some(left) => window_node_balance(space_id, left, axis_flag, space_manager),
        None => BalanceNode {
            y_count: 0,
            x_count: 0,
        },
    };
    let right_leafs = match right {
        Some(right) => window_node_balance(space_id, right, axis_flag, space_manager),
        None => BalanceNode {
            y_count: 0,
            x_count: 0,
        },
    };
    let mut total_leafs = balance_node_add(left_leafs, right_leafs);

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return total_leafs;
    };
    let node = view.node_mut(node_id);

    if (axis_flag & WindowNodeSplit::Y as u32) != 0 {
        if node.split == WindowNodeSplit::Y {
            node.ratio = left_leafs.y_count as f32 / total_leafs.y_count as f32;
            total_leafs.y_count -= 1;
        }
    }

    if (axis_flag & WindowNodeSplit::X as u32) != 0 {
        if node.split == WindowNodeSplit::X {
            node.ratio = left_leafs.x_count as f32 / total_leafs.x_count as f32;
            total_leafs.x_count -= 1;
        }
    }

    let parent = node.parent;
    if let Some(parent) = parent {
        let parent_split = view.node(parent).split;
        total_leafs.y_count += (parent_split == WindowNodeSplit::Y) as i32;
        total_leafs.x_count += (parent_split == WindowNodeSplit::X) as i32;
    }

    total_leafs
}

pub(crate) fn window_node_split(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
) {
    let window_zoom_persist = space_manager.window_zoom_persist;

    let (left, right, zoom) = {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let left = view.allocate_node();
        let right = view.allocate_node();

        let node = view.node(node_id);
        let zoom = if !window_zoom_persist {
            None
        } else if node.zoom.is_none() {
            None
        } else if node.zoom == node.parent {
            Some(node_id)
        } else {
            Some(ROOT_NODE_ID)
        };

        (left, right, zoom)
    };

    if window_node_get_child(space_id, node_id, space_manager) == WindowNodeChild::Second {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let node = view.node(node_id);
        let window_count = node.window_count;
        let window_list = node.window_list;
        let window_order = node.window_order;

        let left_node = view.node_mut(left);
        left_node.window_list[..window_count as usize]
            .copy_from_slice(&window_list[..window_count as usize]);
        left_node.window_order[..window_count as usize]
            .copy_from_slice(&window_order[..window_count as usize]);
        left_node.window_count = window_count;
        left_node.zoom = zoom;

        let right_node = view.node_mut(right);
        right_node.window_list[0] = window_id;
        right_node.window_order[0] = window_id;
        right_node.window_count = 1;
    } else {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let node = view.node(node_id);
        let window_count = node.window_count;
        let window_list = node.window_list;
        let window_order = node.window_order;

        let right_node = view.node_mut(right);
        right_node.window_list[..window_count as usize]
            .copy_from_slice(&window_list[..window_count as usize]);
        right_node.window_order[..window_count as usize]
            .copy_from_slice(&window_order[..window_count as usize]);
        right_node.window_count = window_count;
        right_node.zoom = zoom;

        let left_node = view.node_mut(left);
        left_node.window_list[0] = window_id;
        left_node.window_order[0] = window_id;
        left_node.window_count = 1;
    }

    {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        view.node_mut(left).parent = Some(node_id);
        view.node_mut(right).parent = Some(node_id);

        let node = view.node_mut(node_id);
        node.window_count = 0;
        node.left = Some(left);
        node.right = Some(right);
        node.zoom = None;
    }

    area_make_pair_for_node(space_manager, space_id, node_id);
}

pub(crate) fn window_node_update(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
) {
    if window_node_is_leaf(space_id, node_id, space_manager) {
        let insert_direction = match space_manager.view.find(&space_id) {
            Some(view) => view.node(node_id).insert_direction,
            None => return,
        };
        if insert_direction != 0 {
            insert_feedback_show(space_id, node_id, window_manager, space_manager);
        }
    } else {
        area_make_pair_for_node(space_manager, space_id, node_id);

        let (left, right) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                (node.left, node.right)
            }
            None => return,
        };

        if let Some(left) = left {
            window_node_update(space_manager, space_id, left, window_manager);
        }
        if let Some(right) = right {
            window_node_update(space_manager, space_id, right, window_manager);
        }
    }
}

pub(crate) fn window_node_destroy(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut node_ids = Vec::new();
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    window_node_collect_subtree_post_order(view, node_id, &mut node_ids);

    for node_id in node_ids {
        let window_ids = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                node.window_list[..node.window_count as usize].to_vec()
            }
            None => return,
        };

        for window_id in window_ids {
            window_manager_remove_managed_window(window_manager, window_id);
        }

        insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
        view_free_node(space_id, node_id, window_manager, space_manager, mouse_drag_state);
    }
}

pub(crate) fn window_node_clear_zoom(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) {
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);
    node.zoom = None;
    let left = node.left;
    let right = node.right;

    if !window_node_is_leaf(space_id, node_id, space_manager) {
        if let Some(left) = left {
            window_node_clear_zoom(space_id, left, space_manager);
        }
        if let Some(right) = right {
            window_node_clear_zoom(space_id, right, space_manager);
        }
    }
}

pub(crate) fn window_node_capture_windows(
    space_id: SpaceId,
    node_id: NodeId,
    window_list: &mut Vec<WindowCapture>,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    if window_node_is_leaf(space_id, node_id, space_manager) {
        let (window_count, node_window_list, area) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                let area = match node.zoom {
                    Some(zoom) => view.node(zoom).area,
                    None => node.area,
                };
                (node.window_count, node.window_list, area)
            }
            None => return,
        };

        for index in 0..window_count as usize {
            if window_manager_find_window(window_manager, node_window_list[index]).is_some() {
                window_list.push(WindowCapture {
                    window_id: node_window_list[index],
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: area.height,
                });
            }
        }
    } else {
        let (left, right) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                (node.left, node.right)
            }
            None => return,
        };

        if let Some(left) = left {
            window_node_capture_windows(space_id, left, window_list, window_manager, space_manager);
        }
        if let Some(right) = right {
            window_node_capture_windows(
                space_id,
                right,
                window_list,
                window_manager,
                space_manager,
            );
        }
    }
}

pub(crate) fn window_node_flush(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let mut window_list: Vec<WindowCapture> = Vec::new();
    window_node_capture_windows(
        space_id,
        node_id,
        &mut window_list,
        window_manager,
        space_manager,
    );
    if !window_list.is_empty() {
        window_manager_animate_window_list(&window_list, window_manager);
    }
}

pub(crate) fn window_node_contains_window(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };
    let node = view.node(node_id);

    for index in 0..node.window_count {
        if node.window_list[index as usize] == window_id {
            return true;
        }
    }

    false
}

pub(crate) fn window_node_index_of_window(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> i32 {
    let Some(view) = space_manager.view.find(&space_id) else {
        return 0;
    };
    let node = view.node(node_id);

    for index in 0..node.window_count {
        if node.window_list[index as usize] == window_id {
            return index;
        }
    }

    0
}

pub(crate) fn window_node_swap_window_list(
    a_space_id: SpaceId,
    a_node_id: NodeId,
    b_space_id: SpaceId,
    b_node_id: NodeId,
    space_manager: &mut SpaceManager,
) {
    let (a_window_list, a_window_order, a_window_count) = match space_manager.view.find(&a_space_id)
    {
        Some(view) => {
            let node = view.node(a_node_id);
            (node.window_list, node.window_order, node.window_count)
        }
        None => return,
    };
    let (b_window_list, b_window_order, b_window_count) = match space_manager.view.find(&b_space_id)
    {
        Some(view) => {
            let node = view.node(b_node_id);
            (node.window_list, node.window_order, node.window_count)
        }
        None => return,
    };

    {
        let Some(view) = space_manager.view.find_mut(&a_space_id) else {
            return;
        };
        let node = view.node_mut(a_node_id);
        node.window_list[..b_window_count as usize]
            .copy_from_slice(&b_window_list[..b_window_count as usize]);
        node.window_order[..b_window_count as usize]
            .copy_from_slice(&b_window_order[..b_window_count as usize]);
        node.window_count = b_window_count;
    }

    {
        let Some(view) = space_manager.view.find_mut(&b_space_id) else {
            return;
        };
        let node = view.node_mut(b_node_id);
        node.window_list[..a_window_count as usize]
            .copy_from_slice(&a_window_list[..a_window_count as usize]);
        node.window_order[..a_window_count as usize]
            .copy_from_slice(&a_window_order[..a_window_count as usize]);
        node.window_count = a_window_count;
    }

    if let Some(view) = space_manager.view.find_mut(&a_space_id) {
        view.node_mut(a_node_id).zoom = None;
    }
    if let Some(view) = space_manager.view.find_mut(&b_space_id) {
        view.node_mut(b_node_id).zoom = None;
    }
}

pub(crate) fn window_node_find_first_leaf(
    space_id: SpaceId,
    root_node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> NodeId {
    let mut node_id = root_node_id;
    while !window_node_is_leaf(space_id, node_id, space_manager) {
        node_id = match space_manager.view.find(&space_id) {
            Some(view) => match view.node(node_id).left {
                Some(left) => left,
                None => return node_id,
            },
            None => return node_id,
        };
    }
    node_id
}

pub(crate) fn window_node_find_last_leaf(
    space_id: SpaceId,
    root_node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> NodeId {
    let mut node_id = root_node_id;
    while !window_node_is_leaf(space_id, node_id, space_manager) {
        node_id = match space_manager.view.find(&space_id) {
            Some(view) => match view.node(node_id).right {
                Some(right) => right,
                None => return node_id,
            },
            None => return node_id,
        };
    }
    node_id
}

pub(crate) fn window_node_find_prev_leaf(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let parent = match space_manager.view.find(&space_id) {
        Some(view) => view.node(node_id).parent,
        None => return None,
    };
    let Some(parent) = parent else {
        return None;
    };

    if window_node_is_left_child(space_id, node_id, space_manager) {
        return window_node_find_prev_leaf(space_id, parent, space_manager);
    }

    let parent_left = match space_manager.view.find(&space_id) {
        Some(view) => view.node(parent).left,
        None => return None,
    };
    let Some(parent_left) = parent_left else {
        return None;
    };

    if window_node_is_leaf(space_id, parent_left, space_manager) {
        return Some(parent_left);
    }

    let parent_left_right = match space_manager.view.find(&space_id) {
        Some(view) => view.node(parent_left).right,
        None => return None,
    };
    let Some(parent_left_right) = parent_left_right else {
        return None;
    };

    Some(window_node_find_last_leaf(
        space_id,
        parent_left_right,
        space_manager,
    ))
}

pub(crate) fn window_node_find_next_leaf(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let parent = match space_manager.view.find(&space_id) {
        Some(view) => view.node(node_id).parent,
        None => return None,
    };
    let Some(parent) = parent else {
        return None;
    };

    if window_node_is_right_child(space_id, node_id, space_manager) {
        return window_node_find_next_leaf(space_id, parent, space_manager);
    }

    let parent_right = match space_manager.view.find(&space_id) {
        Some(view) => view.node(parent).right,
        None => return None,
    };
    let Some(parent_right) = parent_right else {
        return None;
    };

    if window_node_is_leaf(space_id, parent_right, space_manager) {
        return Some(parent_right);
    }

    let parent_right_left = match space_manager.view.find(&space_id) {
        Some(view) => view.node(parent_right).left,
        None => return None,
    };
    let Some(parent_right_left) = parent_right_left else {
        return None;
    };

    Some(window_node_find_first_leaf(
        space_id,
        parent_right_left,
        space_manager,
    ))
}

pub(crate) fn window_node_rotate(
    space_id: SpaceId,
    node_id: NodeId,
    degrees: i32,
    space_manager: &mut SpaceManager,
) {
    let (left, right) = {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let node = view.node_mut(node_id);

        if (degrees == 90 && node.split == WindowNodeSplit::Y)
            || (degrees == 270 && node.split == WindowNodeSplit::X)
            || (degrees == 180)
        {
            let temporary = node.left;
            node.left = node.right;
            node.right = temporary;
            node.ratio = 1.0f32 - node.ratio;
        }

        if degrees != 180 {
            if node.split == WindowNodeSplit::X {
                node.split = WindowNodeSplit::Y;
            } else if node.split == WindowNodeSplit::Y {
                node.split = WindowNodeSplit::X;
            }
        }

        (node.left, node.right)
    };

    if !window_node_is_leaf(space_id, node_id, space_manager) {
        if let Some(left) = left {
            window_node_rotate(space_id, left, degrees, space_manager);
        }
        if let Some(right) = right {
            window_node_rotate(space_id, right, degrees, space_manager);
        }
    }
}

pub(crate) fn window_node_mirror(
    space_id: SpaceId,
    node_id: NodeId,
    axis: WindowNodeSplit,
    space_manager: &mut SpaceManager,
) -> NodeId {
    if !window_node_is_leaf(space_id, node_id, space_manager) {
        let (node_left, node_right) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(node_id);
                (node.left, node.right)
            }
            None => return node_id,
        };

        let left = node_left.map(|left| window_node_mirror(space_id, left, axis, space_manager));
        let right =
            node_right.map(|right| window_node_mirror(space_id, right, axis, space_manager));

        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return node_id;
        };
        let node = view.node_mut(node_id);
        if node.split == axis {
            node.left = right;
            node.right = left;
        }
    }

    node_id
}

pub(crate) fn window_node_fence(
    space_id: SpaceId,
    node_id: NodeId,
    direction: i32,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let (node_area, mut parent_id) = match space_manager.view.find(&space_id) {
        Some(view) => {
            let node = view.node(node_id);
            (node.area, node.parent)
        }
        None => return None,
    };

    while let Some(parent) = parent_id {
        let Some(view) = space_manager.view.find(&space_id) else {
            return None;
        };
        let parent_node = view.node(parent);

        if (direction == DIR_NORTH
            && parent_node.split == WindowNodeSplit::X
            && parent_node.area.y < node_area.y)
            || (direction == DIR_WEST
                && parent_node.split == WindowNodeSplit::Y
                && parent_node.area.x < node_area.x)
            || (direction == DIR_SOUTH
                && parent_node.split == WindowNodeSplit::X
                && (parent_node.area.y + parent_node.area.height)
                    > (node_area.y + node_area.height))
            || (direction == DIR_EAST
                && parent_node.split == WindowNodeSplit::Y
                && (parent_node.area.x + parent_node.area.width) > (node_area.x + node_area.width))
        {
            return Some(parent);
        }

        parent_id = parent_node.parent;
    }

    None
}

pub(crate) fn view_find_min_depth_leaf_node(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<NodeId> {
    let mut list: VecDeque<NodeId> = VecDeque::from([node_id]);

    while let Some(current_node_id) = list.pop_front() {
        if window_node_is_leaf(space_id, current_node_id, space_manager) {
            return Some(current_node_id);
        }

        let (left, right) = match space_manager.view.find(&space_id) {
            Some(view) => {
                let node = view.node(current_node_id);
                (node.left, node.right)
            }
            None => return None,
        };

        if let Some(left) = left {
            list.push_back(left);
        }
        if let Some(right) = right {
            list.push_back(right);
        }
    }

    None
}

pub(crate) fn area_is_in_direction(
    first_area: &Area,
    first_area_max_point: CGPoint,
    second_area: &Area,
    second_area_max_point: CGPoint,
    direction: i32,
) -> bool {
    if direction == DIR_NORTH && first_area_max_point.y <= second_area.y as f64 {
        return false;
    }
    if direction == DIR_EAST && second_area_max_point.x <= first_area.x as f64 {
        return false;
    }
    if direction == DIR_SOUTH && second_area_max_point.y <= first_area.y as f64 {
        return false;
    }
    if direction == DIR_WEST && first_area_max_point.x <= second_area.x as f64 {
        return false;
    }

    if direction == DIR_NORTH || direction == DIR_SOUTH {
        return (second_area_max_point.x > first_area.x as f64
            && second_area_max_point.x <= first_area_max_point.x)
            || (second_area.x < first_area.x
                && second_area_max_point.x > first_area_max_point.x)
            || (second_area.x >= first_area.x
                && (second_area.x as f64) < first_area_max_point.x);
    }

    if direction == DIR_EAST || direction == DIR_WEST {
        return (second_area_max_point.y > first_area.y as f64
            && second_area_max_point.y <= first_area_max_point.y)
            || (second_area.y < first_area.y
                && second_area_max_point.y > first_area_max_point.y)
            || (second_area.y >= first_area.y
                && (second_area.y as f64) < first_area_max_point.y);
    }

    false
}

pub(crate) fn area_distance_in_direction(
    first_area: &Area,
    first_area_max_point: CGPoint,
    second_area: &Area,
    second_area_max_point: CGPoint,
    direction: i32,
) -> i32 {
    match direction {
        DIR_NORTH => {
            return (if second_area_max_point.y > first_area.y as f64 {
                second_area_max_point.y - first_area.y as f64
            } else {
                first_area.y as f64 - second_area_max_point.y
            }) as i32;
        }
        DIR_EAST => {
            return (if (second_area.x as f64) < first_area_max_point.x {
                first_area_max_point.x - second_area.x as f64
            } else {
                second_area.x as f64 - first_area_max_point.x
            }) as i32;
        }
        DIR_SOUTH => {
            return (if (second_area.y as f64) < first_area_max_point.y {
                first_area_max_point.y - second_area.y as f64
            } else {
                second_area.y as f64 - first_area_max_point.y
            }) as i32;
        }
        DIR_WEST => {
            return (if second_area_max_point.x > first_area.x as f64 {
                second_area_max_point.x - first_area.x as f64
            } else {
                first_area.x as f64 - second_area_max_point.x
            }) as i32;
        }
        _ => {}
    }

    i32::MAX
}

pub(crate) fn view_find_window_node_in_direction(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    source_node_id: NodeId,
    direction: i32,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    let window_list = space_window_list(space_id, false, window_manager)?;

    let mut best_distance = i32::MAX;
    let mut best_rank = i32::MAX;
    let mut best_node: Option<NodeId> = None;
    let source_area = space_manager.view.find(&space_id)?.node(source_node_id).area;
    let source_area_max = area_max_point(source_area);

    let mut target = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(target_node_id) = target {
        if source_node_id != target_node_id {
            let (target_area, target_first_window_id) = {
                let target_node = space_manager.view.find(&space_id)?.node(target_node_id);
                (target_node.area, target_node.window_order[0])
            };

            let target_area_max = area_max_point(target_area);
            if area_is_in_direction(
                &source_area,
                source_area_max,
                &target_area,
                target_area_max,
                direction,
            ) {
                let distance = area_distance_in_direction(
                    &source_area,
                    source_area_max,
                    &target_area,
                    target_area_max,
                    direction,
                );
                let rank =
                    window_manager_find_rank_of_window_in_list(target_first_window_id, &window_list);
                if (distance < best_distance) || (distance == best_distance && rank < best_rank) {
                    best_node = Some(target_node_id);
                    best_distance = distance;
                    best_rank = rank;
                }
            }
        }

        target = window_node_find_next_leaf(space_id, target_node_id, space_manager);
    }

    best_node
}

pub(crate) fn view_find_window_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
) -> Option<NodeId> {
    let mut node = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(node_id) = node {
        if window_node_contains_window(space_id, node_id, window_id, space_manager) {
            return Some(node_id);
        }

        node = window_node_find_next_leaf(space_id, node_id, space_manager);
    }

    None
}

pub(crate) fn view_remove_window_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) -> Option<NodeId> {
    let node_id = view_find_window_node(space_manager, space_id, window_id)?;

    if space_manager.view.find(&space_id)?.node(node_id).window_count > 1 {
        let view = space_manager.view.find_mut(&space_id)?;
        let node = view.node_mut(node_id);
        let mut removed_entry = false;
        let mut removed_order = false;

        for index in 0..node.window_count {
            let index = index as usize;
            let window_count = node.window_count as usize;

            if !removed_entry && node.window_list[index] == window_id {
                node.window_list.copy_within(index + 1..window_count, index);
                removed_entry = true;
            }

            if !removed_order && node.window_order[index] == window_id {
                node.window_order.copy_within(index + 1..window_count, index);
                removed_order = true;
            }
        }

        debug_assert!(removed_entry);
        debug_assert!(removed_order);
        node.window_count -= 1;
        let node_first_window_id = node.window_order[0];

        if view.insertion_point == window_id {
            view.insertion_point = node_first_window_id;
        }

        return None;
    }

    if node_id == ROOT_NODE_ID {
        space_manager.view.find_mut(&space_id)?.insertion_point = WindowId(0);
        insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
        *space_manager.view.find_mut(&space_id)?.node_mut(node_id) = WindowNode::default();
        view_update(space_manager, space_id, display_manager, window_manager);
        return None;
    }

    let window_zoom_persist = space_manager.window_zoom_persist;

    let (
        parent_id,
        parent_parent,
        child_id,
        child_window_list,
        child_window_order,
        child_window_count,
        child_parent,
        child_zoom,
        child_left,
        child_right,
        child_left_zoom,
        child_right_zoom,
        child_insert_direction,
        child_split,
        child_child,
    ) = {
        let view = space_manager.view.find(&space_id)?;
        let parent_id = view.node(node_id).parent?;
        let parent = view.node(parent_id);
        let child_id = if parent.right == Some(node_id) {
            parent.left?
        } else {
            parent.right?
        };
        let child = view.node(child_id);
        (
            parent_id,
            parent.parent,
            child_id,
            child.window_list,
            child.window_order,
            child.window_count,
            child.parent,
            child.zoom,
            child.left,
            child.right,
            child.left.and_then(|left| view.node(left).zoom),
            child.right.and_then(|right| view.node(right).zoom),
            child.insert_direction,
            child.split,
            child.child,
        )
    };

    let parent_zoom = if !window_zoom_persist {
        None
    } else if child_zoom.is_none() {
        None
    } else if child_zoom == Some(parent_id) {
        parent_parent
    } else {
        Some(ROOT_NODE_ID)
    };

    {
        let view = space_manager.view.find_mut(&space_id)?;
        let parent = view.node_mut(parent_id);
        parent.window_list[..child_window_count as usize]
            .copy_from_slice(&child_window_list[..child_window_count as usize]);
        parent.window_order[..child_window_count as usize]
            .copy_from_slice(&child_window_order[..child_window_count as usize]);
        parent.window_count = child_window_count;

        parent.left = None;
        parent.right = None;
        parent.zoom = parent_zoom;
    }

    if child_insert_direction != 0 {
        let parent_first_window_id = {
            let view = space_manager.view.find_mut(&space_id)?;
            let feedback_window = view.node_mut(child_id).feedback_window.take();
            let parent = view.node_mut(parent_id);
            parent.feedback_window = feedback_window;
            parent.insert_direction = child_insert_direction;
            parent.split = child_split;
            parent.child = child_child;
            parent.window_order[0]
        };
        window_manager.insert_feedback.remove(&parent_first_window_id);
        window_manager
            .insert_feedback
            .add(parent_first_window_id, (space_id, parent_id));
        insert_feedback_show(space_id, parent_id, window_manager, space_manager);
    }

    if child_parent.is_some() && !(child_left.is_none() && child_right.is_none()) {
        {
            let view = space_manager.view.find_mut(&space_id)?;

            view.node_mut(parent_id).left = child_left;
            if let Some(left) = child_left {
                let left_node = view.node_mut(left);
                left_node.parent = Some(parent_id);
                left_node.zoom = if !window_zoom_persist {
                    None
                } else if child_left_zoom.is_none() {
                    None
                } else if child_left_zoom == Some(child_id) {
                    Some(parent_id)
                } else {
                    Some(ROOT_NODE_ID)
                };
            }

            view.node_mut(parent_id).right = child_right;
            if let Some(right) = child_right {
                let right_node = view.node_mut(right);
                right_node.parent = Some(parent_id);
                right_node.zoom = if !window_zoom_persist {
                    None
                } else if child_right_zoom.is_none() {
                    None
                } else if child_right_zoom == Some(child_id) {
                    Some(parent_id)
                } else {
                    Some(ROOT_NODE_ID)
                };
            }
        }

        if !window_zoom_persist {
            window_node_clear_zoom(space_id, parent_id, space_manager);
        }

        window_node_update(space_manager, space_id, parent_id, window_manager);
    }

    insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
    view_free_node(space_id, child_id, window_manager, space_manager, mouse_drag_state);
    view_free_node(space_id, node_id, window_manager, space_manager, mouse_drag_state);

    let auto_balance = space_manager.view.find(&space_id)?.auto_balance;
    if auto_balance != WindowNodeSplit::None as u32 {
        window_node_balance(space_id, ROOT_NODE_ID, auto_balance, space_manager);
        view_update(space_manager, space_id, display_manager, window_manager);
        return Some(ROOT_NODE_ID);
    }

    Some(parent_id)
}

pub(crate) fn view_stack_window_node(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let node = view.node_mut(node_id);

    if node.window_count as usize >= NODE_MAX_WINDOW_COUNT {
        return;
    }

    let mut insert_index = node.window_count;

    for index in 0..node.window_count {
        if node.window_list[index as usize] == node.window_order[0] {
            insert_index = index + 1;
            break;
        }
    }

    if insert_index < node.window_count {
        node.window_list.copy_within(
            insert_index as usize..node.window_count as usize,
            insert_index as usize + 1,
        );
    }

    node.window_list[insert_index as usize] = window_id;
    node.window_order
        .copy_within(0..node.window_count as usize, 1);
    node.window_order[0] = window_id;
    node.window_count += 1;
}

pub(crate) fn view_add_window_node_with_insertion_point(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    insertion_point: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    if !window_node_is_occupied(space_id, ROOT_NODE_ID, space_manager)
        && window_node_is_leaf(space_id, ROOT_NODE_ID, space_manager)
    {
        let root = space_manager.view.find_mut(&space_id)?.node_mut(ROOT_NODE_ID);
        root.window_list[0] = window_id;
        root.window_order[0] = window_id;
        root.window_count = 1;
        return Some(ROOT_NODE_ID);
    }

    let layout = space_manager.view.find(&space_id)?.layout;

    if layout == ViewType::Bsp {
        let mut previous_insertion_point = WindowId(0);
        let mut leaf: Option<NodeId> = None;

        if insertion_point != WindowId(0) {
            let view = space_manager.view.find_mut(&space_id)?;
            previous_insertion_point = view.insertion_point;
            view.insertion_point = insertion_point;
        }

        let view_insertion_point = space_manager.view.find(&space_id)?.insertion_point;
        if view_insertion_point != WindowId(0) {
            leaf = view_find_window_node(space_manager, space_id, view_insertion_point);
            space_manager.view.find_mut(&space_id)?.insertion_point = previous_insertion_point;

            if let Some(leaf) = leaf {
                let do_stack = {
                    let leaf_node = space_manager.view.find_mut(&space_id)?.node_mut(leaf);
                    let do_stack = leaf_node.insert_direction == STACK;

                    leaf_node.insert_direction = 0;
                    do_stack
                };
                insert_feedback_destroy(space_id, leaf, window_manager, space_manager);

                if do_stack {
                    view_stack_window_node(space_id, leaf, window_id, space_manager);
                    return Some(leaf);
                }
            }
        }

        if leaf.is_none() {
            if space_manager.window_insertion_point == WindowInsertionPoint::Focused {
                leaf = view_find_window_node(
                    space_manager,
                    space_id,
                    window_manager.focused_window_id,
                );
            } else if space_manager.window_insertion_point == WindowInsertionPoint::First {
                leaf = Some(window_node_find_first_leaf(
                    space_id,
                    ROOT_NODE_ID,
                    space_manager,
                ));
            } else if space_manager.window_insertion_point == WindowInsertionPoint::Last {
                leaf = Some(window_node_find_last_leaf(
                    space_id,
                    ROOT_NODE_ID,
                    space_manager,
                ));
            }

            if leaf.is_none() {
                leaf = view_find_min_depth_leaf_node(space_id, ROOT_NODE_ID, space_manager);
            }
        }

        let leaf = leaf?;

        window_node_split(space_manager, space_id, leaf, window_id);

        let auto_balance = space_manager.view.find(&space_id)?.auto_balance;
        if auto_balance != WindowNodeSplit::None as u32 {
            window_node_balance(space_id, ROOT_NODE_ID, auto_balance, space_manager);
            view_update(space_manager, space_id, display_manager, window_manager);
            return Some(ROOT_NODE_ID);
        }

        return Some(leaf);
    } else if layout == ViewType::Stack {
        view_stack_window_node(space_id, ROOT_NODE_ID, window_id, space_manager);
        return Some(ROOT_NODE_ID);
    }

    None
}

pub(crate) fn view_add_window_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<NodeId> {
    view_add_window_node_with_insertion_point(
        space_manager,
        space_id,
        window_id,
        WindowId(0),
        display_manager,
        window_manager,
    )
}

pub(crate) fn view_find_window_list(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> Vec<WindowId> {
    let mut window_list: Vec<WindowId> = Vec::new();

    if space_manager.view.find(&space_id).is_none() {
        return window_list;
    }

    let mut node = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(node_id) = node {
        if let Some(view) = space_manager.view.find(&space_id) {
            let leaf = view.node(node_id);
            for index in 0..leaf.window_count {
                window_list.push(leaf.window_list[index as usize]);
            }
        }

        node = window_node_find_next_leaf(space_id, node_id, space_manager);
    }

    window_list
}

pub(crate) fn view_is_invalid(space_manager: &mut SpaceManager, space_id: SpaceId) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    !view.check_flag(ViewFlag::IS_VALID)
}

pub(crate) fn view_is_dirty(space_manager: &mut SpaceManager, space_id: SpaceId) -> bool {
    let Some(view) = space_manager.view.find(&space_id) else {
        return false;
    };

    view.check_flag(ViewFlag::IS_DIRTY)
}

pub(crate) fn view_flush(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_manager: &mut WindowManager,
) {
    if space_manager.view.find(&space_id).is_none() {
        return;
    }

    if space_is_visible(space_id) {
        window_node_flush(space_id, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.clear_flag(ViewFlag::IS_DIRTY);
        }
    } else if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }
}

pub(crate) fn view_serialize(
    response: &mut Response,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    let layout = view.layout;

    let mut flags = flags;
    if flags == 0x0 {
        flags |= !flags;
    }

    let mut did_output = false;
    response.write(format_args!("{{\n"));

    if (flags & SPACE_PROPERTY_ID) != 0 {
        response.write(format_args!("\t\"id\":{}", space_id.0 as i64));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_UUID) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let uuid = space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.uuid.as_ref())
            .and_then(|uuid| ts_cfstring_copy(uuid.as_ref()));
        response.write(format_args!(
            "\t\"uuid\":\"{}\"",
            uuid.as_deref().unwrap_or("<unknown>")
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_INDEX) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"index\":{}",
            space_manager_mission_control_index(space_id)
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_LABEL) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let space_label = space_manager_get_label_for_space(space_manager, space_id);
        response.write(format_args!(
            "\t\"label\":\"{}\"",
            match &space_label {
                Some(space_label) => space_label.label.as_str(),
                None => "",
            }
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_TYPE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"type\":\"{}\"",
            VIEW_TYPE_STR[layout as usize]
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_DISPLAY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"display\":{}",
            display_manager_display_id_arrangement(space_display_id(space_id), display_manager)
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_WINDOWS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let window_list = space_window_list(space_id, true, window_manager).unwrap_or_default();
        let window_count = window_list.len() as i32;

        response.write(format_args!("\t\"windows\":["));
        for index in 0..window_count {
            if index < window_count - 1 {
                response.write(format_args!("{}, ", window_list[index as usize].0 as i32));
            } else {
                response.write(format_args!("{}", window_list[index as usize].0 as i32));
            }
        }
        response.write(format_args!("]"));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_FIRST_WINDOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let first_leaf = window_node_find_first_leaf(space_id, ROOT_NODE_ID, space_manager);
        let first_window_id = space_manager
            .view
            .find(&space_id)
            .map_or(WindowId(0), |view| view.node(first_leaf).window_order[0]);
        response.write(format_args!(
            "\t\"first-window\":{}",
            first_window_id.0 as i32
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_LAST_WINDOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let last_leaf = window_node_find_last_leaf(space_id, ROOT_NODE_ID, space_manager);
        let last_window_id = space_manager
            .view
            .find(&space_id)
            .map_or(WindowId(0), |view| view.node(last_leaf).window_order[0]);
        response.write(format_args!(
            "\t\"last-window\":{}",
            last_window_id.0 as i32
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_HAS_FOCUS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-focus\":{}",
            json_bool(space_id == space_manager.current_space_id)
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_IS_VISIBLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-visible\":{}",
            json_bool(space_is_visible(space_id))
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_IS_FULLSCREEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-native-fullscreen\":{}",
            json_bool(space_is_fullscreen(space_id))
        ));
    }

    response.write(format_args!("\n}}"));
}

pub(crate) fn view_update(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    if space_manager.view.find(&space_id).is_none() {
        return;
    }

    let display_id = space_display_id(space_id);
    let frame = display_bounds_constrained(display_id, false, display_manager);

    {
        let Some(view) = space_manager.view.find_mut(&space_id) else {
            return;
        };
        let enable_padding = view.check_flag(ViewFlag::ENABLE_PADDING);
        let top_padding = view.top_padding;
        let bottom_padding = view.bottom_padding;
        let left_padding = view.left_padding;
        let right_padding = view.right_padding;

        let root = view.node_mut(ROOT_NODE_ID);
        root.area = area_from_cgrect(frame);

        if enable_padding {
            root.area.x += left_padding as f32;
            root.area.width -= (left_padding + right_padding) as f32;
            root.area.y += top_padding as f32;
            root.area.height -= (top_padding + bottom_padding) as f32;
        }
    }

    window_node_update(space_manager, space_id, ROOT_NODE_ID, window_manager);

    if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::IS_VALID);
        view.set_flag(ViewFlag::IS_DIRTY);
    }
}

pub(crate) fn view_create(
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> SpaceId {
    let mut view = View {
        uuid: None,
        space_id: SpaceId(0),
        nodes: vec![Some(WindowNode::default())],
        free_node_ids: Vec::new(),
        insertion_point: WindowId(0),
        layout: ViewType::Default,
        split_type: WindowNodeSplit::None,
        top_padding: 0,
        bottom_padding: 0,
        left_padding: 0,
        right_padding: 0,
        window_gap: 0,
        auto_balance: 0,
        flags: 0,
    };

    view.space_id = space_id;
    view.uuid = unsafe {
        take_create_rule_result(SLSSpaceCopyName(*CONNECTION.get().unwrap(), space_id.0))
    }
    .map(SendCFRetained);

    view.set_flag(ViewFlag::ENABLE_PADDING);
    view.set_flag(ViewFlag::ENABLE_GAP);

    if space_is_user(view.space_id) {
        if !view.check_flag(ViewFlag::LAYOUT) {
            view.layout = space_manager.layout;
        }
        if !view.check_flag(ViewFlag::TOP_PADDING) {
            view.top_padding = space_manager.top_padding;
        }
        if !view.check_flag(ViewFlag::BOTTOM_PADDING) {
            view.bottom_padding = space_manager.bottom_padding;
        }
        if !view.check_flag(ViewFlag::LEFT_PADDING) {
            view.left_padding = space_manager.left_padding;
        }
        if !view.check_flag(ViewFlag::RIGHT_PADDING) {
            view.right_padding = space_manager.right_padding;
        }
        if !view.check_flag(ViewFlag::WINDOW_GAP) {
            view.window_gap = space_manager.window_gap;
        }
        if !view.check_flag(ViewFlag::AUTO_BALANCE) {
            view.auto_balance = space_manager.auto_balance;
        }
        if !view.check_flag(ViewFlag::SPLIT_TYPE) {
            view.split_type = space_manager.split_type;
        }
        space_manager.view.add(space_id, view);
        view_update(space_manager, space_id, display_manager, window_manager);
    } else {
        view.layout = ViewType::Float;
        space_manager.view.add(space_id, view);
    }

    space_id
}

pub(crate) fn view_clear(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let (left, right) = match space_manager.view.find(&space_id) {
        Some(view) => {
            let root = view.node(ROOT_NODE_ID);
            (root.left, root.right)
        }
        None => return,
    };

    if let Some(left) = left {
        window_node_destroy(space_id, left, window_manager, space_manager, mouse_drag_state);
    }
    if let Some(right) = right {
        window_node_destroy(space_id, right, window_manager, space_manager, mouse_drag_state);
    }

    let window_ids = match space_manager.view.find(&space_id) {
        Some(view) => {
            let root = view.node(ROOT_NODE_ID);
            root.window_list[..root.window_count as usize].to_vec()
        }
        None => return,
    };

    for window_id in window_ids {
        window_manager_remove_managed_window(window_manager, window_id);
    }

    insert_feedback_destroy(space_id, ROOT_NODE_ID, window_manager, space_manager);
    if let Some(view) = space_manager.view.find_mut(&space_id) {
        *view.node_mut(ROOT_NODE_ID) = WindowNode::default();
    }
    view_update(space_manager, space_id, display_manager, window_manager);
}

pub(crate) fn view_destroy(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    window_node_destroy(
        space_id,
        ROOT_NODE_ID,
        window_manager,
        space_manager,
        mouse_drag_state,
    );

    if let Some(view) = space_manager.view.find_mut(&space_id) {
        if view.uuid.is_some() {
            drop(view.uuid.take());
        }
    }
}

pub(crate) fn ax_diff(first: f64, second: f64) -> bool {
    let difference = first - second;
    let absolute = if difference < 0.0f64 {
        difference * -1.0f64
    } else {
        difference
    };
    absolute >= 1.5f32 as f64
}

pub(crate) fn view_free_node(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if mouse_drag_state.feedback_node == Some((space_id, node_id)) {
        mouse_drag_state.feedback_node = None;
    }

    let window_ids_whose_insert_feedback_names_the_freed_node: Vec<WindowId> = window_manager
        .insert_feedback
        .iter()
        .filter(|(_, feedback_node)| **feedback_node == (space_id, node_id))
        .map(|(window_id, _)| *window_id)
        .collect();
    for window_id in window_ids_whose_insert_feedback_names_the_freed_node {
        window_manager.insert_feedback.remove(&window_id);
    }

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    view.nodes[node_id.0 as usize] = None;
    view.free_node_ids.push(node_id);
}

pub(crate) fn window_node_collect_subtree_post_order(
    view: &View,
    node_id: NodeId,
    out: &mut Vec<NodeId>,
) {
    let node = view.node(node_id);
    if let Some(left) = node.left {
        window_node_collect_subtree_post_order(view, left, out);
    }
    if let Some(right) = node.right {
        window_node_collect_subtree_post_order(view, right, out);
    }
    out.push(node_id);
}

#[cfg(test)]
mod tests {
    use super::{Area, area_distance_in_direction, area_is_in_direction, area_max_point};
    use crate::ffi::core_foundation::CGPoint;
    use crate::misc::macros::{DIR_EAST, DIR_WEST};

    struct TestArea {
        area: Area,
        area_max: CGPoint,
    }

    fn init_test_display_list() -> [TestArea; 3] {
        let mut display_list: [TestArea; 3] = std::array::from_fn(|_| TestArea {
            area: Area::default(),
            area_max: CGPoint { x: 0.0, y: 0.0 },
        });

        display_list[0].area.x = 0.0;
        display_list[0].area.y = 0.0;
        display_list[0].area.width = 2560.0;
        display_list[0].area.height = 1440.0;
        display_list[0].area_max = area_max_point(display_list[0].area);

        display_list[1].area.x = -1728.0;
        display_list[1].area.y = 0.0;
        display_list[1].area.width = 1728.0;
        display_list[1].area.height = 1117.0;
        display_list[1].area_max = area_max_point(display_list[1].area);

        display_list[2].area.x = 2560.0;
        display_list[2].area.y = 0.0;
        display_list[2].area.width = 1920.0;
        display_list[2].area.height = 1080.0;
        display_list[2].area_max = area_max_point(display_list[2].area);

        display_list
    }

    #[test]
    fn test_display_area_is_in_direction() {
        let display_list = init_test_display_list();

        let t1 = area_is_in_direction(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[1].area,
            display_list[1].area_max,
            DIR_WEST,
        );
        assert_eq!(t1, true);

        let t2 = area_is_in_direction(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[1].area,
            display_list[1].area_max,
            DIR_EAST,
        );
        assert_eq!(t2, false);

        let t3 = area_is_in_direction(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[2].area,
            display_list[2].area_max,
            DIR_WEST,
        );
        assert_eq!(t3, false);

        let t4 = area_is_in_direction(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[2].area,
            display_list[2].area_max,
            DIR_EAST,
        );
        assert_eq!(t4, true);
    }

    fn closest_display_in_direction(
        display_list: &[TestArea],
        display_count: i32,
        source: i32,
        direction: i32,
    ) -> i32 {
        let mut best_index = -1;
        let mut best_distance = i32::MAX;

        for index in 0..display_count {
            if index == source {
                continue;
            }

            let source_display = &display_list[source as usize];
            let candidate_display = &display_list[index as usize];
            if area_is_in_direction(
                &source_display.area,
                source_display.area_max,
                &candidate_display.area,
                candidate_display.area_max,
                direction,
            ) {
                let distance = area_distance_in_direction(
                    &source_display.area,
                    source_display.area_max,
                    &candidate_display.area,
                    candidate_display.area_max,
                    direction,
                );
                if distance < best_distance {
                    best_index = index;
                    best_distance = distance;
                }
            }
        }

        best_index
    }

    #[test]
    fn test_closest_display_in_direction() {
        let display_list = init_test_display_list();
        let display_count = display_list.len() as i32;
        let mut best_index;

        best_index = closest_display_in_direction(&display_list, display_count, 0, DIR_WEST);
        assert_eq!(best_index, 1);

        best_index = closest_display_in_direction(&display_list, display_count, 1, DIR_WEST);
        assert_eq!(best_index, -1);

        best_index = closest_display_in_direction(&display_list, display_count, 2, DIR_WEST);
        assert_eq!(best_index, 0);

        best_index = closest_display_in_direction(&display_list, display_count, 0, DIR_EAST);
        assert_eq!(best_index, 2);

        best_index = closest_display_in_direction(&display_list, display_count, 1, DIR_EAST);
        assert_eq!(best_index, 0);

        best_index = closest_display_in_direction(&display_list, display_count, 2, DIR_EAST);
        assert_eq!(best_index, -1);
    }
}
