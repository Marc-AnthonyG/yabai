use crate::display::bounds::display_bounds_constrained;
use crate::display::manager::DisplayManager;
use crate::ffi::CFStringOwned;
use crate::ffi::core_foundation::{SendCFRetained, take_create_rule_result};
use crate::ffi::skylight::SLSSpaceCopyName;
use crate::layout::area::area_from_cgrect;
use crate::layout::insertion::insert_feedback_destroy;
use crate::layout::settings::{ViewFlag, ViewType};
use crate::layout::tree::{
    WindowNode, WindowNodeSplit, window_node_destroy, window_node_flush, window_node_update,
};
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::{space_display_id, space_is_user, space_is_visible};
use crate::space::manager::SpaceManager;
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::manager::{WindowManager, window_manager_remove_managed_window};

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
