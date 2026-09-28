use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::space::manager::SpaceManager;
use crate::support::arithmetic::is_within_range_including_both_bounds;
use crate::support::handles::{NodeId, SpaceId};

pub(crate) static AUTO_BALANCE_NAMES: [&str; 4] = ["off", "vertical", "horizontal", "on"];

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum ViewLayout {
    Default = 0,
    BinarySpacePartitioning = 1,
    Stack = 2,
    Float = 3,
}

pub(crate) static VIEW_LAYOUT_NAMES: [&str; 4] = ["default", "bsp", "stack", "float"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViewFlag(pub u64);

impl ViewFlag {
    pub(crate) const OVERRIDES_GLOBAL_LAYOUT: ViewFlag = ViewFlag(0x001);
    pub(crate) const OVERRIDES_GLOBAL_TOP_PADDING: ViewFlag = ViewFlag(0x002);
    pub(crate) const OVERRIDES_GLOBAL_BOTTOM_PADDING: ViewFlag = ViewFlag(0x004);
    pub(crate) const OVERRIDES_GLOBAL_LEFT_PADDING: ViewFlag = ViewFlag(0x008);
    pub(crate) const OVERRIDES_GLOBAL_RIGHT_PADDING: ViewFlag = ViewFlag(0x010);
    pub(crate) const OVERRIDES_GLOBAL_WINDOW_GAP: ViewFlag = ViewFlag(0x020);
    pub(crate) const OVERRIDES_GLOBAL_AUTO_BALANCE: ViewFlag = ViewFlag(0x040);
    pub(crate) const PADDING_IS_ENABLED: ViewFlag = ViewFlag(0x080);
    pub(crate) const WINDOW_GAP_IS_ENABLED: ViewFlag = ViewFlag(0x100);
    pub(crate) const AREAS_ARE_UP_TO_DATE: ViewFlag = ViewFlag(0x200);
    pub(crate) const WINDOWS_AWAIT_THEIR_AREAS: ViewFlag = ViewFlag(0x400);
    pub(crate) const OVERRIDES_GLOBAL_SPLIT_TYPE: ViewFlag = ViewFlag(0x800);
}

pub(crate) fn effective_child_for_new_window_in_node(
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

pub(crate) fn effective_split_of_node(
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
        WindowNodeSplit::Vertical
    } else {
        WindowNodeSplit::Horizontal
    }
}

pub(crate) fn effective_ratio_of_node(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> f32 {
    let ratio = match space_manager.view.find(&space_id) {
        Some(view) => view.node(node_id).ratio,
        None => return space_manager.split_ratio,
    };

    if is_within_range_including_both_bounds(ratio, 0.1f32, 0.9f32) {
        ratio
    } else {
        space_manager.split_ratio
    }
}

pub(crate) fn effective_window_gap_of_view(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> i32 {
    let Some(view) = space_manager.view.find(&space_id) else {
        return 0;
    };

    if view.has_flag(ViewFlag::WINDOW_GAP_IS_ENABLED) {
        view.window_gap
    } else {
        0
    }
}
