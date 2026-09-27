use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::space::manager::SpaceManager;
use crate::support::arithmetic::in_range_ii;
use crate::support::handles::{NodeId, SpaceId};

pub(crate) static AUTO_BALANCE_STR: [&str; 4] = ["off", "vertical", "horizontal", "on"];

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
