use crate::command::selectors::SpaceSelector;
use crate::space::labels::space_label_with_name;
use crate::space::lookup::{
    query_current_space_of_display_under_the_cursor, query_first_space_in_mission_control_order,
    query_last_space_in_mission_control_order, query_next_space_in_mission_control_order,
    query_previous_space_in_mission_control_order, query_space_at_mission_control_index,
};
use crate::space::manager::SpaceManager;
use crate::support::handles::SpaceId;

pub(crate) fn resolve_space_selector(
    selector: &SpaceSelector,
    relative_to: SpaceId,
    space_manager: &mut SpaceManager,
) -> Result<SpaceId, String> {
    let resolved_space_id = match selector {
        SpaceSelector::Previous => {
            query_previous_space_in_mission_control_order(space_to_move_from(relative_to)?)
        }
        SpaceSelector::Next => {
            query_next_space_in_mission_control_order(space_to_move_from(relative_to)?)
        }
        SpaceSelector::First => query_first_space_in_mission_control_order(),
        SpaceSelector::Last => query_last_space_in_mission_control_order(),
        SpaceSelector::Recent => space_manager.last_space_id,
        SpaceSelector::UnderTheMouse => query_current_space_of_display_under_the_cursor(),
        SpaceSelector::MissionControlIndex(index) => {
            query_space_at_mission_control_index(index.get() as i32)
        }
        SpaceSelector::Label(label) => space_label_with_name(space_manager, label.as_bytes())
            .map_or(SpaceId(0), |space_label| space_label.space_id),
    };

    if resolved_space_id == SpaceId(0) {
        return Err(failure_locating_the_space_of(selector));
    }
    Ok(resolved_space_id)
}

fn space_to_move_from(relative_to: SpaceId) -> Result<SpaceId, String> {
    if relative_to == SpaceId(0) {
        return Err(String::from("could not locate the selected space."));
    }
    Ok(relative_to)
}

fn failure_locating_the_space_of(selector: &SpaceSelector) -> String {
    match selector {
        SpaceSelector::Previous => String::from("could not locate the previous space."),
        SpaceSelector::Next => String::from("could not locate the next space."),
        SpaceSelector::First => String::from("could not locate the first space."),
        SpaceSelector::Last => String::from("could not locate the last space."),
        SpaceSelector::Recent => String::from("could not locate the most recently focused space."),
        SpaceSelector::UnderTheMouse => String::from("could not locate space containing cursor."),
        SpaceSelector::MissionControlIndex(index) => {
            format!("could not locate space with mission-control index '{index}'.")
        }
        SpaceSelector::Label(label) => format!("could not locate space with label '{label}'."),
    }
}
