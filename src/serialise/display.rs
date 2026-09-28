use serde::Serialize;

use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::identity::copy_uuid_of_display;
use crate::display::labels::label_of_display;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_spaces_of_display;
use crate::ffi::core_graphics::CGDisplayBounds;
use crate::serialise::frame::{FrameSnapshot, snapshot_of_frame};
use crate::space::lookup::query_mission_control_index_of_space;
use crate::support::handles::DisplayId;

#[derive(Serialize, Debug, Default)]
pub(crate) struct DisplaySnapshot {
    pub(crate) id: u32,
    pub(crate) uuid: Option<String>,
    pub(crate) index: i32,
    pub(crate) label: Option<String>,
    pub(crate) frame: FrameSnapshot,
    pub(crate) spaces: Vec<i32>,
    pub(crate) has_focus: bool,
}

pub(crate) fn snapshot_of_display(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> DisplaySnapshot {
    DisplaySnapshot {
        id: display_id.0,
        uuid: copy_uuid_of_display(display_id).map(|uuid| uuid.as_ref().to_string()),
        index: query_arrangement_index_of_display(display_id, display_manager),
        label: label_of_display(display_manager, display_id)
            .map(|display_label| display_label.label.clone()),
        frame: snapshot_of_frame(CGDisplayBounds(display_id.0)),
        spaces: mission_control_indexes_of_the_spaces_of_display(display_id),
        has_focus: display_id == display_manager.current_display_id,
    }
}

fn mission_control_indexes_of_the_spaces_of_display(display_id: DisplayId) -> Vec<i32> {
    let space_list = query_spaces_of_display(display_id).unwrap_or_default();
    let Some(first_space_id) = space_list.first() else {
        return Vec::new();
    };
    let first_mission_control_index = query_mission_control_index_of_space(*first_space_id);
    (0..space_list.len() as i32)
        .map(|offset| first_mission_control_index + offset)
        .collect()
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;

    use super::DisplaySnapshot;
    use crate::command::query::DisplayFieldName;

    #[test]
    fn a_display_prints_snake_case_keys_named_like_its_fields() {
        let keys: Vec<String> = serde_json::to_value(DisplaySnapshot::default())
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();

        let names_of_the_fields: Vec<String> = DisplayFieldName::value_variants()
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
    fn a_display_without_a_label_or_uuid_prints_them_as_null() {
        let display = serde_json::to_value(DisplaySnapshot::default()).unwrap();

        assert_eq!(display["label"], serde_json::Value::Null);
        assert_eq!(display["uuid"], serde_json::Value::Null);
        assert_eq!(display["has_focus"], false);
    }
}
