use crate::command::selectors::{
    CardinalDirection, DisplaySelector, SpaceSelector, StackPositionSelector, WindowSelector,
};
use crate::display::arrangement::{
    closest_display_in_direction_of_display, query_display_at_arrangement_index,
    query_first_display_in_arrangement, query_last_display_in_arrangement,
    query_next_display_in_arrangement, query_previous_display_in_arrangement,
};
use crate::display::identity::{
    query_display_showing_the_active_menu_bar, query_display_under_the_cursor,
};
use crate::display::labels::display_label_with_name;
use crate::display::manager::DisplayManager;
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::labels::space_label_with_name;
use crate::space::lookup::{
    query_current_space_of_display_under_the_cursor, query_first_space_in_mission_control_order,
    query_last_space_in_mission_control_order, query_next_space_in_mission_control_order,
    query_previous_space_in_mission_control_order, query_space_at_mission_control_index,
};
use crate::space::manager::SpaceManager;
use crate::support::direction::{DIRECTION_EAST, DIRECTION_NORTH, DIRECTION_SOUTH, DIRECTION_WEST};
use crate::support::handles::{DisplayId, SpaceId, WindowId};
use crate::window::focus::query_focused_tracked_window;
use crate::window::manager::{WindowManager, tracked_window_with_id};
use crate::window::screen_lookup::query_tracked_window_under_cursor;
use crate::window::stack_lookup::{
    first_window_in_stack_holding_window, last_window_in_stack_holding_window,
    next_window_in_stack_holding_window, previous_window_in_stack_holding_window,
    previously_focused_window_in_stack_holding_window,
    window_at_one_based_position_in_stack_holding_window,
};
use crate::window::tree_lookup::{
    closest_managed_window_in_direction, first_cousin_window_of_managed_window,
    first_managed_window_in_active_space, first_nephew_window_of_managed_window,
    largest_managed_window_in_active_space, last_managed_window_in_active_space,
    managed_window_after_window_in_active_space, managed_window_before_window_in_active_space,
    previously_focused_window_if_managed, second_cousin_window_of_managed_window,
    second_nephew_window_of_managed_window, sibling_window_of_managed_window,
    smallest_managed_window_in_active_space, uncle_window_of_managed_window,
};

pub(crate) fn resolve_display_selector_or_the_display_showing_the_active_menu_bar(
    selector: Option<&DisplaySelector>,
    display_manager: &mut DisplayManager,
) -> Result<DisplayId, String> {
    let display_showing_the_active_menu_bar = query_display_showing_the_active_menu_bar();
    match selector {
        Some(selector) => resolve_display_selector(
            selector,
            display_showing_the_active_menu_bar,
            display_manager,
        ),
        None if display_showing_the_active_menu_bar == DisplayId(0) => Err(String::from(
            "could not locate the display showing the active menu bar.",
        )),
        None => Ok(display_showing_the_active_menu_bar),
    }
}

pub(crate) fn resolve_space_selector_or_the_focused_space(
    selector: Option<&SpaceSelector>,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<SpaceId, String> {
    let focused_space_id = query_current_space_of_the_focused_display(window_manager);
    match selector {
        Some(selector) => resolve_space_selector(selector, focused_space_id, space_manager),
        None if focused_space_id == SpaceId(0) => {
            Err(String::from("could not locate the focused space."))
        }
        None => Ok(focused_space_id),
    }
}

pub(crate) fn resolve_window_selector_or_the_focused_window(
    selector: Option<&WindowSelector>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<WindowId, String> {
    resolve_window_selector_or_the_focused_window_if_there_is_one(
        selector,
        display_manager,
        window_manager,
        space_manager,
    )?
    .ok_or_else(|| String::from("could not locate the focused window."))
}

pub(crate) fn resolve_window_selector_or_the_focused_window_if_there_is_one(
    selector: Option<&WindowSelector>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<Option<WindowId>, String> {
    let focused_window_id = query_focused_tracked_window(window_manager);
    match selector {
        Some(selector) => resolve_window_selector(
            selector,
            focused_window_id,
            display_manager,
            window_manager,
            space_manager,
        )
        .map(Some),
        None => Ok(focused_window_id),
    }
}

pub(crate) fn resolve_display_selector(
    selector: &DisplaySelector,
    relative_to: DisplayId,
    display_manager: &mut DisplayManager,
) -> Result<DisplayId, String> {
    let resolved_display_id = match selector {
        DisplaySelector::InDirection(direction) => closest_display_in_direction_of_display(
            display_to_move_from(relative_to)?,
            degrees_of_direction(*direction),
        ),
        DisplaySelector::Previous => query_previous_display_in_arrangement(
            display_to_move_from(relative_to)?,
            display_manager,
        ),
        DisplaySelector::Next => {
            query_next_display_in_arrangement(display_to_move_from(relative_to)?, display_manager)
        }
        DisplaySelector::First => query_first_display_in_arrangement(display_manager),
        DisplaySelector::Last => query_last_display_in_arrangement(display_manager),
        DisplaySelector::Recent => display_manager.last_display_id,
        DisplaySelector::UnderTheMouse => query_display_under_the_cursor(),
        DisplaySelector::ArrangementIndex(index) => {
            query_display_at_arrangement_index(index.get() as i32, display_manager)
        }
        DisplaySelector::Label(label) => display_label_with_name(display_manager, label.as_bytes())
            .map_or(DisplayId(0), |display_label| display_label.display_id),
    };

    if resolved_display_id == DisplayId(0) {
        return Err(failure_locating_the_display_of(selector));
    }
    Ok(resolved_display_id)
}

fn display_to_move_from(relative_to: DisplayId) -> Result<DisplayId, String> {
    if relative_to == DisplayId(0) {
        return Err(String::from("could not locate the selected display."));
    }
    Ok(relative_to)
}

fn failure_locating_the_display_of(selector: &DisplaySelector) -> String {
    match selector {
        DisplaySelector::InDirection(direction) => format!(
            "could not locate a display to the {}.",
            name_of_direction(*direction)
        ),
        DisplaySelector::Previous => String::from("could not locate the previous display."),
        DisplaySelector::Next => String::from("could not locate the next display."),
        DisplaySelector::First => String::from("could not locate the first display."),
        DisplaySelector::Last => String::from("could not locate the last display."),
        DisplaySelector::Recent => {
            String::from("could not locate the most recently focused display.")
        }
        DisplaySelector::UnderTheMouse => {
            String::from("could not locate display containing cursor.")
        }
        DisplaySelector::ArrangementIndex(index) => {
            format!("could not locate display with arrangement index '{index}'.")
        }
        DisplaySelector::Label(label) => format!("could not locate display with label '{label}'."),
    }
}

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

pub(crate) fn resolve_window_selector(
    selector: &WindowSelector,
    relative_to: Option<WindowId>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<WindowId, String> {
    let resolved_window_id = match selector {
        WindowSelector::InDirection(direction) => closest_managed_window_in_direction(
            window_manager,
            window_to_move_from(relative_to)?,
            degrees_of_direction(*direction),
            space_manager,
        ),
        WindowSelector::Previous => managed_window_before_window_in_active_space(
            space_manager,
            window_manager,
            window_to_move_from(relative_to)?,
            display_manager,
        ),
        WindowSelector::Next => managed_window_after_window_in_active_space(
            space_manager,
            window_manager,
            window_to_move_from(relative_to)?,
            display_manager,
        ),
        WindowSelector::First => {
            first_managed_window_in_active_space(space_manager, window_manager, display_manager)
        }
        WindowSelector::Last => {
            last_managed_window_in_active_space(space_manager, window_manager, display_manager)
        }
        WindowSelector::Recent => previously_focused_window_if_managed(window_manager),
        WindowSelector::UnderTheMouse => query_tracked_window_under_cursor(window_manager),
        WindowSelector::Largest => {
            largest_managed_window_in_active_space(space_manager, window_manager, display_manager)
        }
        WindowSelector::Smallest => {
            smallest_managed_window_in_active_space(space_manager, window_manager, display_manager)
        }
        WindowSelector::Sibling => sibling_window_of_managed_window(
            window_manager,
            window_to_move_from(relative_to)?,
            space_manager,
        ),
        WindowSelector::FirstNephew => first_nephew_window_of_managed_window(
            window_manager,
            window_to_move_from(relative_to)?,
            space_manager,
        ),
        WindowSelector::SecondNephew => second_nephew_window_of_managed_window(
            window_manager,
            window_to_move_from(relative_to)?,
            space_manager,
        ),
        WindowSelector::Uncle => uncle_window_of_managed_window(
            window_manager,
            window_to_move_from(relative_to)?,
            space_manager,
        ),
        WindowSelector::FirstCousin => first_cousin_window_of_managed_window(
            window_manager,
            window_to_move_from(relative_to)?,
            space_manager,
        ),
        WindowSelector::SecondCousin => second_cousin_window_of_managed_window(
            window_manager,
            window_to_move_from(relative_to)?,
            space_manager,
        ),
        WindowSelector::InTheStack(stack_position) => {
            window_at_position_in_the_stack_holding_window(
                *stack_position,
                window_to_move_from(relative_to)?,
                display_manager,
                window_manager,
                space_manager,
            )
        }
        WindowSelector::Id(window_id) => {
            tracked_window_with_id(window_manager, WindowId(*window_id))
        }
    };

    resolved_window_id.ok_or_else(|| failure_locating_the_window_of(selector))
}

fn window_at_position_in_the_stack_holding_window(
    stack_position: StackPositionSelector,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    match stack_position {
        StackPositionSelector::Previous => previous_window_in_stack_holding_window(
            space_manager,
            window_manager,
            window_id,
            display_manager,
        ),
        StackPositionSelector::Next => next_window_in_stack_holding_window(
            space_manager,
            window_manager,
            window_id,
            display_manager,
        ),
        StackPositionSelector::First => first_window_in_stack_holding_window(
            space_manager,
            window_manager,
            window_id,
            display_manager,
        ),
        StackPositionSelector::Last => last_window_in_stack_holding_window(
            space_manager,
            window_manager,
            window_id,
            display_manager,
        ),
        StackPositionSelector::Recent => previously_focused_window_in_stack_holding_window(
            space_manager,
            window_manager,
            window_id,
            display_manager,
        ),
        StackPositionSelector::OneBasedPosition(position) => {
            window_at_one_based_position_in_stack_holding_window(
                space_manager,
                window_manager,
                window_id,
                i32::try_from(position.get()).unwrap_or(i32::MAX),
                display_manager,
            )
        }
    }
}

fn window_to_move_from(relative_to: Option<WindowId>) -> Result<WindowId, String> {
    relative_to.ok_or_else(|| String::from("could not locate the selected window."))
}

fn failure_locating_the_window_of(selector: &WindowSelector) -> String {
    match selector {
        WindowSelector::InDirection(direction) => format!(
            "could not locate a managed window to the {}.",
            name_of_direction(*direction)
        ),
        WindowSelector::Previous => String::from("could not locate the prev managed window."),
        WindowSelector::Next => String::from("could not locate the next managed window."),
        WindowSelector::First => String::from("could not locate the first managed window."),
        WindowSelector::Last => String::from("could not locate the last managed window."),
        WindowSelector::Recent => {
            String::from("could not locate the most recently focused window.")
        }
        WindowSelector::UnderTheMouse => {
            String::from("could not locate a window below the cursor.")
        }
        WindowSelector::Largest => String::from("could not locate window with the largest area."),
        WindowSelector::Smallest => String::from("could not locate window with the smallest area."),
        WindowSelector::Sibling => String::from("could not locate sibling of window."),
        WindowSelector::FirstNephew => String::from("could not locate first nephew of window."),
        WindowSelector::SecondNephew => String::from("could not locate second nephew of window."),
        WindowSelector::Uncle => String::from("could not locate uncle of window."),
        WindowSelector::FirstCousin => String::from("could not locate first cousin of window."),
        WindowSelector::SecondCousin => String::from("could not locate second cousin of window."),
        WindowSelector::InTheStack(stack_position) => {
            failure_locating_the_stacked_window_at(*stack_position)
        }
        WindowSelector::Id(window_id) => {
            format!("could not locate window with the specified id '{window_id}'.")
        }
    }
}

fn failure_locating_the_stacked_window_at(stack_position: StackPositionSelector) -> String {
    match stack_position {
        StackPositionSelector::Previous => {
            String::from("could not locate the prev stacked window.")
        }
        StackPositionSelector::Next => String::from("could not locate the next stacked window."),
        StackPositionSelector::First => String::from("could not locate the first stacked window."),
        StackPositionSelector::Last => String::from("could not locate the last stacked window."),
        StackPositionSelector::Recent => {
            String::from("could not locate the recent stacked window.")
        }
        StackPositionSelector::OneBasedPosition(position) => {
            format!("could not locate the stacked window in position {position}.")
        }
    }
}

fn degrees_of_direction(direction: CardinalDirection) -> i32 {
    match direction {
        CardinalDirection::North => DIRECTION_NORTH,
        CardinalDirection::East => DIRECTION_EAST,
        CardinalDirection::South => DIRECTION_SOUTH,
        CardinalDirection::West => DIRECTION_WEST,
    }
}

fn name_of_direction(direction: CardinalDirection) -> &'static str {
    match direction {
        CardinalDirection::North => "north",
        CardinalDirection::East => "east",
        CardinalDirection::South => "south",
        CardinalDirection::West => "west",
    }
}
