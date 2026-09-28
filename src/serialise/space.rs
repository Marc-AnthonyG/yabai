use serde::Serialize;

use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::manager::DisplayManager;
use crate::layout::settings::ViewLayout;
use crate::layout::tree::{first_leaf_below_node, last_leaf_below_node};
use crate::space::labels::label_of_space;
use crate::space::lookup::query_mission_control_index_of_space;
use crate::space::managed_space::{
    is_native_fullscreen_space, is_space_visible_on_its_display, query_display_holding_space,
    query_windows_on_space,
};
use crate::space::manager::SpaceManager;
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::window::manager::WindowManager;

#[derive(Serialize, Debug)]
pub(crate) struct SpaceSnapshot {
    pub(crate) id: u64,
    pub(crate) uuid: Option<String>,
    pub(crate) index: i32,
    pub(crate) label: Option<String>,
    #[serde(rename = "type")]
    pub(crate) layout: ViewLayout,
    pub(crate) display: i32,
    pub(crate) windows: Vec<u32>,
    pub(crate) first_window: u32,
    pub(crate) last_window: u32,
    pub(crate) has_focus: bool,
    pub(crate) is_visible: bool,
    pub(crate) is_native_fullscreen: bool,
}

pub(crate) fn snapshot_of_the_space_of_a_view(
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Option<SpaceSnapshot> {
    let view = space_manager.view.get(&space_id)?;
    let layout = view.layout;
    let uuid = view.uuid.as_ref().map(|uuid| uuid.as_ref().to_string());

    let first_leaf = first_leaf_below_node(space_id, ROOT_NODE_ID, space_manager);
    let last_leaf = last_leaf_below_node(space_id, ROOT_NODE_ID, space_manager);

    Some(SpaceSnapshot {
        id: space_id.0,
        uuid,
        index: query_mission_control_index_of_space(space_id),
        label: label_of_space(space_manager, space_id).map(|space_label| space_label.label.clone()),
        layout,
        display: query_arrangement_index_of_display(
            query_display_holding_space(space_id),
            display_manager,
        ),
        windows: query_windows_on_space(space_id, true, window_manager)
            .unwrap_or_default()
            .into_iter()
            .map(|window_id| window_id.0)
            .collect(),
        first_window: front_window_of_leaf(space_id, first_leaf, space_manager).0,
        last_window: front_window_of_leaf(space_id, last_leaf, space_manager).0,
        has_focus: space_id == space_manager.current_space_id,
        is_visible: is_space_visible_on_its_display(space_id),
        is_native_fullscreen: is_native_fullscreen_space(space_id),
    })
}

fn front_window_of_leaf(space_id: SpaceId, leaf: NodeId, space_manager: &SpaceManager) -> WindowId {
    space_manager
        .find_node_in_view_of_space(space_id, leaf)
        .map_or(WindowId(0), |node| node.window_order[0])
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;

    use super::SpaceSnapshot;
    use crate::command::query::SpaceFieldName;
    use crate::layout::settings::ViewLayout;

    fn snapshot_of_a_bsp_space() -> SpaceSnapshot {
        SpaceSnapshot {
            id: 3,
            uuid: None,
            index: 1,
            label: Some(String::from("code")),
            layout: ViewLayout::BinarySpacePartitioning,
            display: 1,
            windows: vec![101, 102],
            first_window: 101,
            last_window: 102,
            has_focus: true,
            is_visible: true,
            is_native_fullscreen: false,
        }
    }

    #[test]
    fn a_space_prints_snake_case_keys_named_like_its_fields() {
        let keys: Vec<String> = serde_json::to_value(snapshot_of_a_bsp_space())
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();

        let names_of_the_fields: Vec<String> = SpaceFieldName::value_variants()
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
    }

    #[test]
    fn a_space_prints_its_layout_as_type_in_the_spelling_config_set_takes() {
        let space = serde_json::to_value(snapshot_of_a_bsp_space()).unwrap();

        assert_eq!(space["type"], "bsp");
        assert_eq!(space["label"], "code");
        assert_eq!(space["windows"], serde_json::json!([101, 102]));
    }
}
