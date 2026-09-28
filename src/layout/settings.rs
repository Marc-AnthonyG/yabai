use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::space::manager::SpaceManager;
use crate::support::handles::{NodeId, SpaceId};

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(i32)]
pub(crate) enum ViewLayout {
    #[value(skip)]
    Default = 0,
    #[value(name = "bsp")]
    #[serde(rename = "bsp")]
    BinarySpacePartitioning = 1,
    Stack = 2,
    Float = 3,
}

pub(crate) static VIEW_LAYOUT_NAMES: [&str; 4] = ["default", "bsp", "stack", "float"];

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Default)]
    pub(crate) struct ViewFlag: u64 {
        const OVERRIDES_GLOBAL_LAYOUT = 0x001;
        const OVERRIDES_GLOBAL_TOP_PADDING = 0x002;
        const OVERRIDES_GLOBAL_BOTTOM_PADDING = 0x004;
        const OVERRIDES_GLOBAL_LEFT_PADDING = 0x008;
        const OVERRIDES_GLOBAL_RIGHT_PADDING = 0x010;
        const OVERRIDES_GLOBAL_WINDOW_GAP = 0x020;
        const OVERRIDES_GLOBAL_AUTO_BALANCE = 0x040;
        const PADDING_IS_ENABLED = 0x080;
        const WINDOW_GAP_IS_ENABLED = 0x100;
        const AREAS_ARE_UP_TO_DATE = 0x200;
        const WINDOWS_AWAIT_THEIR_AREAS = 0x400;
        const OVERRIDES_GLOBAL_SPLIT_TYPE = 0x800;
    }
}

pub(crate) fn effective_child_for_new_window_in_node(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> WindowNodeChild {
    let child = match space_manager.view.get(&space_id) {
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
    let Some(view) = space_manager.view.get(&space_id) else {
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
    let ratio = match space_manager.view.get(&space_id) {
        Some(view) => view.node(node_id).ratio,
        None => return space_manager.split_ratio,
    };

    if (0.1..=0.9).contains(&ratio) {
        ratio
    } else {
        space_manager.split_ratio
    }
}

pub(crate) fn effective_window_gap_of_view(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> i32 {
    let Some(view) = space_manager.view.get(&space_id) else {
        return 0;
    };

    if view.flags.contains(ViewFlag::WINDOW_GAP_IS_ENABLED) {
        view.window_gap
    } else {
        0
    }
}
