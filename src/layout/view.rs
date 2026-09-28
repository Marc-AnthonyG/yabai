use crate::display::bounds::query_bounds_of_display_left_for_windows;
use crate::display::manager::DisplayManager;
use crate::ffi::CFStringOwned;
use crate::ffi::core_foundation::{CFRetainedAssumedSendAndSync, take_create_rule_result};
use crate::ffi::skylight::SLSSpaceCopyName;
use crate::layout::area::area_from_cgrect;
use crate::layout::group::RememberedGroup;
use crate::layout::insertion::destroy_insert_feedback_of_node;
use crate::layout::settings::{ViewFlag, ViewLayout};
use crate::layout::tree::{
    WindowNode, WindowNodeSplit, free_node_subtree_unmanaging_its_windows,
    move_windows_below_node_into_their_areas, recompute_areas_below_node_redrawing_insert_feedback,
};
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::{
    is_space_visible_on_its_display, is_user_space, query_display_holding_space,
};
use crate::space::manager::SpaceManager;
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::manager::{WindowManager, forget_managed_window};

pub(crate) struct View {
    pub(crate) uuid: Option<CFStringOwned>,
    pub(crate) space_id: SpaceId,
    pub(crate) nodes: Vec<Option<WindowNode>>,
    pub(crate) free_node_ids: Vec<NodeId>,
    pub(crate) insertion_point: WindowId,
    pub(crate) layout: ViewLayout,
    pub(crate) split_type: WindowNodeSplit,
    pub(crate) top_padding: i32,
    pub(crate) bottom_padding: i32,
    pub(crate) left_padding: i32,
    pub(crate) right_padding: i32,
    pub(crate) window_gap: i32,
    pub(crate) auto_balance: u32,
    pub(crate) flags: ViewFlag,
    pub(crate) groups_remembered_outside_bsp: Vec<RememberedGroup>,
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

    pub(crate) fn allocate_empty_node_reusing_a_freed_id(&mut self) -> NodeId {
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
}

pub(crate) fn has_view_out_of_date_areas(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> bool {
    let Some(view) = space_manager.view.get(&space_id) else {
        return false;
    };

    !view.flags.contains(ViewFlag::AREAS_ARE_UP_TO_DATE)
}

pub(crate) fn has_view_windows_awaiting_their_areas(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> bool {
    let Some(view) = space_manager.view.get(&space_id) else {
        return false;
    };

    view.flags.contains(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS)
}

pub(crate) fn move_view_windows_into_their_areas_or_defer_until_space_is_visible(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_manager: &mut WindowManager,
) {
    if !space_manager.view.contains_key(&space_id) {
        return;
    }

    if is_space_visible_on_its_display(space_id) {
        move_windows_below_node_into_their_areas(
            space_id,
            ROOT_NODE_ID,
            window_manager,
            space_manager,
        );
        if let Some(view) = space_manager.view.get_mut(&space_id) {
            view.flags.remove(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
        }
    } else if let Some(view) = space_manager.view.get_mut(&space_id) {
        view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
    }
}

pub(crate) fn recompute_view_areas_from_display_bounds_and_padding(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    if !space_manager.view.contains_key(&space_id) {
        return;
    }

    let display_id = query_display_holding_space(space_id);
    let frame = query_bounds_of_display_left_for_windows(display_id, false, display_manager);

    {
        let Some(view) = space_manager.view.get_mut(&space_id) else {
            return;
        };
        let enable_padding = view.flags.contains(ViewFlag::PADDING_IS_ENABLED);
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

    recompute_areas_below_node_redrawing_insert_feedback(
        space_manager,
        space_id,
        ROOT_NODE_ID,
        window_manager,
    );

    if let Some(view) = space_manager.view.get_mut(&space_id) {
        view.flags.insert(ViewFlag::AREAS_ARE_UP_TO_DATE);
        view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
    }
}

pub(crate) fn create_view_for_space_from_global_settings(
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
        layout: ViewLayout::Default,
        split_type: WindowNodeSplit::None,
        top_padding: 0,
        bottom_padding: 0,
        left_padding: 0,
        right_padding: 0,
        window_gap: 0,
        auto_balance: 0,
        flags: ViewFlag::empty(),
        groups_remembered_outside_bsp: Vec::new(),
    };

    view.space_id = space_id;
    view.uuid = unsafe {
        take_create_rule_result(SLSSpaceCopyName(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            space_id.0,
        ))
    }
    .map(CFRetainedAssumedSendAndSync);

    view.flags.insert(ViewFlag::PADDING_IS_ENABLED);
    view.flags.insert(ViewFlag::WINDOW_GAP_IS_ENABLED);

    if is_user_space(view.space_id) {
        if !view.flags.contains(ViewFlag::OVERRIDES_GLOBAL_LAYOUT) {
            view.layout = space_manager.layout;
        }
        if !view.flags.contains(ViewFlag::OVERRIDES_GLOBAL_TOP_PADDING) {
            view.top_padding = space_manager.top_padding;
        }
        if !view
            .flags
            .contains(ViewFlag::OVERRIDES_GLOBAL_BOTTOM_PADDING)
        {
            view.bottom_padding = space_manager.bottom_padding;
        }
        if !view.flags.contains(ViewFlag::OVERRIDES_GLOBAL_LEFT_PADDING) {
            view.left_padding = space_manager.left_padding;
        }
        if !view
            .flags
            .contains(ViewFlag::OVERRIDES_GLOBAL_RIGHT_PADDING)
        {
            view.right_padding = space_manager.right_padding;
        }
        if !view.flags.contains(ViewFlag::OVERRIDES_GLOBAL_WINDOW_GAP) {
            view.window_gap = space_manager.window_gap;
        }
        if !view.flags.contains(ViewFlag::OVERRIDES_GLOBAL_AUTO_BALANCE) {
            view.auto_balance = space_manager.auto_balance;
        }
        if !view.flags.contains(ViewFlag::OVERRIDES_GLOBAL_SPLIT_TYPE) {
            view.split_type = space_manager.split_type;
        }
        space_manager.view.entry(space_id).or_insert(view);
        recompute_view_areas_from_display_bounds_and_padding(
            space_manager,
            space_id,
            display_manager,
            window_manager,
        );
    } else {
        view.layout = ViewLayout::Float;
        space_manager.view.entry(space_id).or_insert(view);
    }

    space_id
}

pub(crate) fn clear_view_tree_unmanaging_every_window(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let (left, right) = match space_manager.view.get(&space_id) {
        Some(view) => {
            let root = view.node(ROOT_NODE_ID);
            (root.left, root.right)
        }
        None => return,
    };

    if let Some(left) = left {
        free_node_subtree_unmanaging_its_windows(
            space_id,
            left,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    }
    if let Some(right) = right {
        free_node_subtree_unmanaging_its_windows(
            space_id,
            right,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    }

    let window_ids = match space_manager.view.get(&space_id) {
        Some(view) => {
            let root = view.node(ROOT_NODE_ID);
            root.window_list[..root.window_count as usize].to_vec()
        }
        None => return,
    };

    for window_id in window_ids {
        forget_managed_window(window_manager, window_id);
    }

    destroy_insert_feedback_of_node(space_id, ROOT_NODE_ID, window_manager, space_manager);
    if let Some(view) = space_manager.view.get_mut(&space_id) {
        *view.node_mut(ROOT_NODE_ID) = WindowNode::default();
    }
    recompute_view_areas_from_display_bounds_and_padding(
        space_manager,
        space_id,
        display_manager,
        window_manager,
    );
}

pub(crate) fn free_view_tree_and_release_its_uuid(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    free_node_subtree_unmanaging_its_windows(
        space_id,
        ROOT_NODE_ID,
        window_manager,
        space_manager,
        mouse_drag_state,
    );

    if let Some(view) = space_manager.view.get_mut(&space_id) {
        if view.uuid.is_some() {
            drop(view.uuid.take());
        }
    }
}
