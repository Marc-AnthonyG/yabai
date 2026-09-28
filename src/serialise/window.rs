use serde::Serialize;

use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::manager::DisplayManager;
use crate::ffi::skylight::SLSWindowIsOrderedIn;
use crate::layout::group::is_window_in_a_group;
use crate::layout::tree::{
    WindowNodeChild, WindowNodeSplit, is_node_the_left_child_of_its_parent, leaf_holding_window,
    stack_index_of_window_in_node,
};
use crate::mouse::drag::MouseDragState;
use crate::serialise::frame::{FrameSnapshot, snapshot_of_frame};
use crate::space::lookup::query_mission_control_index_of_space;
use crate::space::managed_space::{is_space_visible_on_its_display, query_display_holding_space};
use crate::space::manager::SpaceManager;
use crate::state::process_wide::{
    LAYER_ABOVE_WINDOW_LEVEL, LAYER_BELOW_WINDOW_LEVEL, LAYER_NORMAL_WINDOW_LEVEL,
    SKYLIGHT_CONNECTION_ID,
};
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::manager::{WindowManager, space_managing_window};
use crate::window::model::{
    WindowFlag, is_window_movable, is_window_on_more_than_one_space, is_window_resizable,
    is_window_shadow_shown_according_to_window_server, query_space_holding_window,
    query_window_level_from_window_server, query_window_opacity_from_window_server,
    query_window_sub_level_from_window_server, window_role_as_string, window_subrole_as_string,
    window_title_as_string,
};

#[derive(Serialize, Debug, Default)]
pub(crate) struct WindowSnapshot {
    pub(crate) id: u32,
    pub(crate) pid: i32,
    pub(crate) app: String,
    pub(crate) title: String,
    pub(crate) scratchpad: Option<String>,
    pub(crate) frame: FrameSnapshot,
    pub(crate) role: String,
    pub(crate) subrole: String,
    pub(crate) root_window: bool,
    pub(crate) display: i32,
    pub(crate) space: i32,
    pub(crate) level: i32,
    pub(crate) sub_level: i32,
    pub(crate) layer: &'static str,
    pub(crate) sub_layer: &'static str,
    pub(crate) opacity: f32,
    pub(crate) split_type: WindowNodeSplit,
    pub(crate) split_child: WindowNodeChild,
    pub(crate) stack_index: i32,
    pub(crate) can_move: bool,
    pub(crate) can_resize: bool,
    pub(crate) has_focus: bool,
    pub(crate) has_shadow: bool,
    pub(crate) has_parent_zoom: bool,
    pub(crate) has_fullscreen_zoom: bool,
    pub(crate) has_ax_reference: bool,
    pub(crate) is_native_fullscreen: bool,
    pub(crate) is_visible: bool,
    pub(crate) is_minimized: bool,
    pub(crate) is_hidden: bool,
    pub(crate) is_floating: bool,
    pub(crate) is_sticky: bool,
    pub(crate) is_grabbed: bool,
    pub(crate) is_grouped: bool,
}

pub(crate) fn layer_name_of_window_level(level: i32) -> &'static str {
    if Some(&level) == LAYER_BELOW_WINDOW_LEVEL.get() {
        "below"
    } else if Some(&level) == LAYER_NORMAL_WINDOW_LEVEL.get() {
        "normal"
    } else if Some(&level) == LAYER_ABOVE_WINDOW_LEVEL.get() {
        "above"
    } else {
        "unknown"
    }
}

pub(crate) fn snapshot_of_tracked_window(
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &MouseDragState,
) -> Option<WindowSnapshot> {
    let space_id = query_space_holding_window(window_id);
    let level = query_window_level_from_window_server(window_id);
    let sub_level = query_window_sub_level_from_window_server(window_id);
    let place_in_the_tree = place_of_window_in_the_tree(window_id, window_manager, space_manager);
    let is_grouped = space_managing_window(window_manager, window_id).is_some_and(|space_id| {
        is_window_in_a_group(space_manager, space_id, window_id, window_manager)
    });
    let display =
        query_arrangement_index_of_display(query_display_holding_space(space_id), display_manager);

    let window = window_manager.window.get(&window_id)?;
    let application = window
        .application
        .and_then(|process_id| window_manager.application.get(&process_id));
    let is_hidden = application.is_some_and(|application| application.is_hidden);
    let is_minimized = window.flags.contains(WindowFlag::MINIMIZED);
    let is_sticky =
        window.flags.contains(WindowFlag::STICKY) || is_window_on_more_than_one_space(window_id);

    Some(WindowSnapshot {
        id: window_id.0,
        pid: application.map_or(0, |application| application.process_id.0),
        app: application.map_or_else(String::new, |application| application.name.to_string()),
        title: window_title_as_string(window),
        scratchpad: window.scratchpad.clone(),
        frame: snapshot_of_frame(window.frame),
        role: window_role_as_string(window),
        subrole: window_subrole_as_string(window),
        root_window: window.is_root,
        display,
        space: query_mission_control_index_of_space(space_id),
        level,
        sub_level,
        layer: layer_name_of_window_level(level),
        sub_layer: layer_name_of_window_level(sub_level),
        opacity: query_window_opacity_from_window_server(window_id),
        split_type: place_in_the_tree.split_type,
        split_child: place_in_the_tree.split_child,
        stack_index: place_in_the_tree.stack_index,
        can_move: is_window_movable(window),
        can_resize: is_window_resizable(window),
        has_focus: window_id == window_manager.focused_window_id,
        has_shadow: is_window_shadow_shown_according_to_window_server(window_id),
        has_parent_zoom: place_in_the_tree.has_parent_zoom,
        has_fullscreen_zoom: place_in_the_tree.has_fullscreen_zoom,
        has_ax_reference: true,
        is_native_fullscreen: window.flags.contains(WindowFlag::IN_NATIVE_FULLSCREEN),
        is_visible: is_window_ordered_in(window_id)
            && !is_minimized
            && !is_hidden
            && (is_sticky || is_space_visible_on_its_display(space_id)),
        is_minimized,
        is_hidden,
        is_floating: window.flags.contains(WindowFlag::FLOATING),
        is_sticky,
        is_grabbed: mouse_drag_state.window_id == Some(window_id),
        is_grouped,
    })
}

#[derive(Default)]
struct PlaceOfWindowInTheTree {
    split_type: WindowNodeSplit,
    split_child: WindowNodeChild,
    stack_index: i32,
    has_parent_zoom: bool,
    has_fullscreen_zoom: bool,
}

fn place_of_window_in_the_tree(
    window_id: WindowId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> PlaceOfWindowInTheTree {
    let Some(space_id) = space_managing_window(window_manager, window_id) else {
        return PlaceOfWindowInTheTree::default();
    };
    let Some(node_id) = leaf_holding_window(space_manager, space_id, window_id) else {
        return PlaceOfWindowInTheTree::default();
    };

    PlaceOfWindowInTheTree {
        split_type: split_of_the_parent_of_node(space_id, node_id, space_manager),
        split_child: if is_node_the_left_child_of_its_parent(space_id, node_id, space_manager) {
            WindowNodeChild::First
        } else {
            WindowNodeChild::Second
        },
        stack_index: one_based_stack_index_of_window_when_stacked(
            space_id,
            node_id,
            window_id,
            space_manager,
        ),
        has_parent_zoom: space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .is_some_and(|node| node.zoom.is_some() && node.zoom == node.parent),
        has_fullscreen_zoom: space_manager
            .find_node_in_view_of_space(space_id, node_id)
            .is_some_and(|node| node.zoom == Some(ROOT_NODE_ID)),
    }
}

fn split_of_the_parent_of_node(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &SpaceManager,
) -> WindowNodeSplit {
    space_manager
        .find_node_in_view_of_space(space_id, node_id)
        .and_then(|node| node.parent)
        .and_then(|parent_id| space_manager.find_node_in_view_of_space(space_id, parent_id))
        .map_or(WindowNodeSplit::None, |parent| parent.split)
}

fn one_based_stack_index_of_window_when_stacked(
    space_id: SpaceId,
    node_id: NodeId,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> i32 {
    let window_count = space_manager
        .find_node_in_view_of_space(space_id, node_id)
        .map_or(0, |node| node.window_count);
    if window_count > 1 {
        stack_index_of_window_in_node(space_id, node_id, window_id, space_manager) + 1
    } else {
        0
    }
}

fn is_window_ordered_in(window_id: WindowId) -> bool {
    let mut ordered_in: u8 = 0;
    unsafe {
        SLSWindowIsOrderedIn(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            window_id.0,
            &mut ordered_in,
        )
    };
    ordered_in != 0
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;

    use super::WindowSnapshot;
    use crate::command::query::WindowFieldName;

    #[test]
    fn a_window_prints_snake_case_keys_named_like_its_fields() {
        let keys: Vec<String> = serde_json::to_value(WindowSnapshot::default())
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();

        let names_of_the_fields: Vec<String> = WindowFieldName::value_variants()
            .iter()
            .map(|field| {
                serde_json::to_value(field)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();

        assert_eq!(keys, names_of_the_fields);
        assert!(keys.contains(&String::from("is_minimized")));
        assert!(keys.contains(&String::from("has_focus")));
    }

    #[test]
    fn a_window_prints_its_place_in_the_tree_in_the_spelling_config_set_takes() {
        let snapshot = serde_json::to_value(WindowSnapshot::default()).unwrap();

        assert_eq!(snapshot["split_type"], "none");
        assert_eq!(snapshot["split_child"], "none");
        assert_eq!(snapshot["scratchpad"], serde_json::Value::Null);
    }
}
