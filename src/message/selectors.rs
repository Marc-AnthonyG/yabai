use crate::daemon_fail;
use crate::display::arrangement::{
    closest_display_in_direction_of_display, query_display_at_arrangement_index,
    query_first_display_in_arrangement, query_last_display_in_arrangement,
    query_next_display_in_arrangement, query_previous_display_in_arrangement,
};
use crate::display::identity::query_display_under_the_cursor;
use crate::display::labels::display_label_with_name;
use crate::display::manager::DisplayManager;
use crate::message::common_arguments::{
    ARGUMENT_COMMON_SELECTOR_EAST, ARGUMENT_COMMON_SELECTOR_FIRST, ARGUMENT_COMMON_SELECTOR_LAST,
    ARGUMENT_COMMON_SELECTOR_MOUSE, ARGUMENT_COMMON_SELECTOR_NEXT, ARGUMENT_COMMON_SELECTOR_NORTH,
    ARGUMENT_COMMON_SELECTOR_PREVIOUS, ARGUMENT_COMMON_SELECTOR_RECENT,
    ARGUMENT_COMMON_SELECTOR_SOUTH, ARGUMENT_COMMON_SELECTOR_STACK,
    ARGUMENT_COMMON_SELECTOR_STACK_PREFIX, ARGUMENT_COMMON_SELECTOR_WEST,
};
use crate::message::token::{
    MessageCursor, Token, TokenValueType, is_token_equal_to, is_token_prefixed_by,
    null_terminated_bytes_starting_at, parse_token_as_non_negative_decimal_integer,
    parse_token_into_typed_value,
};
use crate::space::labels::space_label_with_name;
use crate::space::lookup::{
    query_current_space_of_display_under_the_cursor, query_first_space_in_mission_control_order,
    query_last_space_in_mission_control_order, query_next_space_in_mission_control_order,
    query_previous_space_in_mission_control_order, query_space_at_mission_control_index,
};
use crate::space::manager::SpaceManager;
use crate::support::direction::{
    DIRECTION_EAST, DIRECTION_NORTH, DIRECTION_SOUTH, DIRECTION_STACK_INSTEAD_OF_SPLIT,
    DIRECTION_WEST,
};
use crate::support::handles::{DisplayId, SpaceId, WindowId};
use crate::support::response::{FailurePiece, Response};
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

pub(crate) enum SelectorOutcome<Target> {
    NotASelector,
    ParsedButUnresolved,
    Resolved(Target),
}

pub(crate) struct Selector<Target> {
    pub(crate) token: Token,
    pub(crate) outcome: SelectorOutcome<Target>,
}

impl<Target: Copy> Selector<Target> {
    pub(crate) fn is_recognised_selector(&self) -> bool {
        !matches!(self.outcome, SelectorOutcome::NotASelector)
    }

    pub(crate) fn resolved_target(&self) -> Option<Target> {
        match self.outcome {
            SelectorOutcome::Resolved(value) => Some(value),
            _ => None,
        }
    }
}

pub(crate) const ARGUMENT_WINDOW_SELECTOR_LARGEST: &str = "largest";
pub(crate) const ARGUMENT_WINDOW_SELECTOR_SMALLEST: &str = "smallest";
pub(crate) const ARGUMENT_WINDOW_SELECTOR_SIBLING: &str = "sibling";
pub(crate) const ARGUMENT_WINDOW_SELECTOR_FIRST_NEPHEW: &str = "first_nephew";
pub(crate) const ARGUMENT_WINDOW_SELECTOR_SECOND_NEPHEW: &str = "second_nephew";
pub(crate) const ARGUMENT_WINDOW_SELECTOR_UNCLE: &str = "uncle";
pub(crate) const ARGUMENT_WINDOW_SELECTOR_FIRST_COUSIN: &str = "first_cousin";
pub(crate) const ARGUMENT_WINDOW_SELECTOR_SECOND_COUSIN: &str = "second_cousin";

pub(crate) fn parse_display_selector(
    response: &mut Response,
    message_cursor: &mut MessageCursor,
    acting_display_id: DisplayId,
    optional: bool,
    display_manager: &mut DisplayManager,
) -> Selector<DisplayId> {
    let mut result = Selector {
        token: message_cursor.take_next_token(),
        outcome: SelectorOutcome::ParsedButUnresolved,
    };

    let value = parse_token_into_typed_value(result.token, message_cursor.bytes());
    match value.type_of_value {
        TokenValueType::Integer(int_value) => {
            let display_id = query_display_at_arrangement_index(int_value, display_manager);
            if display_id != DisplayId(0) {
                result.outcome = SelectorOutcome::Resolved(display_id);
            } else {
                daemon_fail!(
                    response,
                    "could not locate display with arrangement index '{}'.\n",
                    int_value
                );
            }
        }
        TokenValueType::String => {
            if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_NORTH,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id =
                        closest_display_in_direction_of_display(acting_display_id, DIRECTION_NORTH);
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate a northward display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_EAST,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id =
                        closest_display_in_direction_of_display(acting_display_id, DIRECTION_EAST);
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate a eastward display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_SOUTH,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id =
                        closest_display_in_direction_of_display(acting_display_id, DIRECTION_SOUTH);
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate a southward display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_WEST,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id =
                        closest_display_in_direction_of_display(acting_display_id, DIRECTION_WEST);
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate a westward display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_PREVIOUS,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id =
                        query_previous_display_in_arrangement(acting_display_id, display_manager);
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate the previous display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_NEXT,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id =
                        query_next_display_in_arrangement(acting_display_id, display_manager);
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate the next display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_FIRST,
            ) {
                let display_id = query_first_display_in_arrangement(display_manager);
                if display_id != DisplayId(0) {
                    result.outcome = SelectorOutcome::Resolved(display_id);
                } else {
                    daemon_fail!(response, "could not locate the first display.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_LAST,
            ) {
                let display_id = query_last_display_in_arrangement(display_manager);
                if display_id != DisplayId(0) {
                    result.outcome = SelectorOutcome::Resolved(display_id);
                } else {
                    daemon_fail!(response, "could not locate the last display.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_RECENT,
            ) {
                if display_manager.last_display_id != DisplayId(0) {
                    result.outcome = SelectorOutcome::Resolved(display_manager.last_display_id);
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_MOUSE,
            ) {
                let display_id = query_display_under_the_cursor();
                if display_id != DisplayId(0) {
                    result.outcome = SelectorOutcome::Resolved(display_id);
                } else {
                    daemon_fail!(response, "could not locate display containing cursor.\n");
                }
            } else {
                let display_id = display_label_with_name(
                    display_manager,
                    null_terminated_bytes_starting_at(message_cursor.bytes(), value.token.start),
                )
                .map(|display_label| display_label.display_id);
                if let Some(display_id) = display_id {
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        result.outcome = SelectorOutcome::ParsedButUnresolved;
                    }
                } else {
                    result.outcome = SelectorOutcome::NotASelector;
                    response.write_failure_pieces_unless_silent(&[
                        FailurePiece::Text("value '"),
                        FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' is not a valid option for DISPLAY_SEL\n"),
                    ]);
                }
            }
        }
        TokenValueType::Invalid => {
            result.outcome = SelectorOutcome::NotASelector;
            if !optional {
                response.write_failure_pieces_unless_silent(&[
                    FailurePiece::Text("value '"),
                    FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                    FailurePiece::Text("' is not a valid option for DISPLAY_SEL\n"),
                ]);
            }
        }
        TokenValueType::Float(_) | TokenValueType::Hexadecimal(_) => {
            result.outcome = SelectorOutcome::NotASelector;
            response.write_failure_pieces_unless_silent(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for DISPLAY_SEL\n"),
            ]);
        }
    }

    result
}

pub(crate) fn parse_space_selector(
    response: &mut Response,
    message_cursor: &mut MessageCursor,
    acting_space_id: SpaceId,
    optional: bool,
    space_manager: &mut SpaceManager,
) -> Selector<SpaceId> {
    let mut result = Selector {
        token: message_cursor.take_next_token(),
        outcome: SelectorOutcome::ParsedButUnresolved,
    };

    let value = parse_token_into_typed_value(result.token, message_cursor.bytes());
    match value.type_of_value {
        TokenValueType::Integer(int_value) => {
            let space_id = query_space_at_mission_control_index(int_value);
            if space_id != SpaceId(0) {
                result.outcome = SelectorOutcome::Resolved(space_id);
            } else {
                daemon_fail!(
                    response,
                    "could not locate space with mission-control index '{}'.\n",
                    int_value
                );
            }
        }
        TokenValueType::String => {
            if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_PREVIOUS,
            ) {
                if acting_space_id != SpaceId(0) {
                    let space_id = query_previous_space_in_mission_control_order(acting_space_id);
                    if space_id != SpaceId(0) {
                        result.outcome = SelectorOutcome::Resolved(space_id);
                    } else {
                        daemon_fail!(response, "could not locate the previous space.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected space.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_NEXT,
            ) {
                if acting_space_id != SpaceId(0) {
                    let space_id = query_next_space_in_mission_control_order(acting_space_id);
                    if space_id != SpaceId(0) {
                        result.outcome = SelectorOutcome::Resolved(space_id);
                    } else {
                        daemon_fail!(response, "could not locate the next space.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected space.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_FIRST,
            ) {
                let space_id = query_first_space_in_mission_control_order();
                if space_id != SpaceId(0) {
                    result.outcome = SelectorOutcome::Resolved(space_id);
                } else {
                    daemon_fail!(response, "could not locate the first space.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_LAST,
            ) {
                let space_id = query_last_space_in_mission_control_order();
                if space_id != SpaceId(0) {
                    result.outcome = SelectorOutcome::Resolved(space_id);
                } else {
                    daemon_fail!(response, "could not locate the last space.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_RECENT,
            ) {
                if space_manager.last_space_id != SpaceId(0) {
                    result.outcome = SelectorOutcome::Resolved(space_manager.last_space_id);
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_MOUSE,
            ) {
                let space_id = query_current_space_of_display_under_the_cursor();
                if space_id != SpaceId(0) {
                    result.outcome = SelectorOutcome::Resolved(space_id);
                } else {
                    daemon_fail!(response, "could not locate space containing cursor.\n");
                }
            } else {
                let space_id = space_label_with_name(
                    space_manager,
                    null_terminated_bytes_starting_at(message_cursor.bytes(), value.token.start),
                )
                .map(|space_label| space_label.space_id);
                if let Some(space_id) = space_id {
                    if space_id != SpaceId(0) {
                        result.outcome = SelectorOutcome::Resolved(space_id);
                    } else {
                        result.outcome = SelectorOutcome::ParsedButUnresolved;
                    }
                } else {
                    result.outcome = SelectorOutcome::NotASelector;
                    response.write_failure_pieces_unless_silent(&[
                        FailurePiece::Text("value '"),
                        FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' is not a valid option for SPACE_SEL\n"),
                    ]);
                }
            }
        }
        TokenValueType::Invalid => {
            result.outcome = SelectorOutcome::NotASelector;
            if !optional {
                response.write_failure_pieces_unless_silent(&[
                    FailurePiece::Text("value '"),
                    FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                    FailurePiece::Text("' is not a valid option for SPACE_SEL\n"),
                ]);
            }
        }
        TokenValueType::Float(_) | TokenValueType::Hexadecimal(_) => {
            result.outcome = SelectorOutcome::NotASelector;
            response.write_failure_pieces_unless_silent(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for SPACE_SEL\n"),
            ]);
        }
    }

    result
}

pub(crate) fn parse_window_selector(
    response: &mut Response,
    message_cursor: &mut MessageCursor,
    acting_window_id: Option<WindowId>,
    optional: bool,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Selector<WindowId> {
    let mut result = Selector {
        token: message_cursor.take_next_token(),
        outcome: SelectorOutcome::ParsedButUnresolved,
    };

    let value = parse_token_into_typed_value(result.token, message_cursor.bytes());
    match value.type_of_value {
        TokenValueType::Integer(int_value) => {
            let window = tracked_window_with_id(window_manager, WindowId(int_value as u32));
            if let Some(window) = window {
                result.outcome = SelectorOutcome::Resolved(window);
            } else {
                daemon_fail!(
                    response,
                    "could not locate window with the specified id '{}'.\n",
                    int_value
                );
            }
        }
        TokenValueType::String => {
            if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_NORTH,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let closest_window = closest_managed_window_in_direction(
                        window_manager,
                        acting_window,
                        DIRECTION_NORTH,
                        space_manager,
                    );
                    if let Some(closest_window) = closest_window {
                        result.outcome = SelectorOutcome::Resolved(closest_window);
                    } else {
                        daemon_fail!(response, "could not locate a northward managed window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_EAST,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let closest_window = closest_managed_window_in_direction(
                        window_manager,
                        acting_window,
                        DIRECTION_EAST,
                        space_manager,
                    );
                    if let Some(closest_window) = closest_window {
                        result.outcome = SelectorOutcome::Resolved(closest_window);
                    } else {
                        daemon_fail!(response, "could not locate a eastward managed window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_SOUTH,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let closest_window = closest_managed_window_in_direction(
                        window_manager,
                        acting_window,
                        DIRECTION_SOUTH,
                        space_manager,
                    );
                    if let Some(closest_window) = closest_window {
                        result.outcome = SelectorOutcome::Resolved(closest_window);
                    } else {
                        daemon_fail!(response, "could not locate a southward managed window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_WEST,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let closest_window = closest_managed_window_in_direction(
                        window_manager,
                        acting_window,
                        DIRECTION_WEST,
                        space_manager,
                    );
                    if let Some(closest_window) = closest_window {
                        result.outcome = SelectorOutcome::Resolved(closest_window);
                    } else {
                        daemon_fail!(response, "could not locate a westward managed window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_MOUSE,
            ) {
                let mouse_window = query_tracked_window_under_cursor(window_manager);
                if let Some(mouse_window) = mouse_window {
                    result.outcome = SelectorOutcome::Resolved(mouse_window);
                } else {
                    daemon_fail!(response, "could not locate a window below the cursor.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SELECTOR_LARGEST,
            ) {
                let area_window = largest_managed_window_in_active_space(
                    space_manager,
                    window_manager,
                    display_manager,
                );
                if let Some(area_window) = area_window {
                    result.outcome = SelectorOutcome::Resolved(area_window);
                } else {
                    daemon_fail!(response, "could not locate window with the largest area.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SELECTOR_SMALLEST,
            ) {
                let area_window = smallest_managed_window_in_active_space(
                    space_manager,
                    window_manager,
                    display_manager,
                );
                if let Some(area_window) = area_window {
                    result.outcome = SelectorOutcome::Resolved(area_window);
                } else {
                    daemon_fail!(
                        response,
                        "could not locate window with the smallest area.\n"
                    );
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SELECTOR_SIBLING,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let sibling_window = sibling_window_of_managed_window(
                        window_manager,
                        acting_window,
                        space_manager,
                    );
                    if let Some(sibling_window) = sibling_window {
                        result.outcome = SelectorOutcome::Resolved(sibling_window);
                    } else {
                        daemon_fail!(response, "could not locate sibling of window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SELECTOR_FIRST_NEPHEW,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let nephew_window = first_nephew_window_of_managed_window(
                        window_manager,
                        acting_window,
                        space_manager,
                    );
                    if let Some(nephew_window) = nephew_window {
                        result.outcome = SelectorOutcome::Resolved(nephew_window);
                    } else {
                        daemon_fail!(response, "could not locate first nephew of window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SELECTOR_SECOND_NEPHEW,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let nephew_window = second_nephew_window_of_managed_window(
                        window_manager,
                        acting_window,
                        space_manager,
                    );
                    if let Some(nephew_window) = nephew_window {
                        result.outcome = SelectorOutcome::Resolved(nephew_window);
                    } else {
                        daemon_fail!(response, "could not locate second nephew of window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SELECTOR_UNCLE,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let uncle_window = uncle_window_of_managed_window(
                        window_manager,
                        acting_window,
                        space_manager,
                    );
                    if let Some(uncle_window) = uncle_window {
                        result.outcome = SelectorOutcome::Resolved(uncle_window);
                    } else {
                        daemon_fail!(response, "could not locate uncle of window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SELECTOR_FIRST_COUSIN,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let cousin_window = first_cousin_window_of_managed_window(
                        window_manager,
                        acting_window,
                        space_manager,
                    );
                    if let Some(cousin_window) = cousin_window {
                        result.outcome = SelectorOutcome::Resolved(cousin_window);
                    } else {
                        daemon_fail!(response, "could not locate first cousin of window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SELECTOR_SECOND_COUSIN,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let cousin_window = second_cousin_window_of_managed_window(
                        window_manager,
                        acting_window,
                        space_manager,
                    );
                    if let Some(cousin_window) = cousin_window {
                        result.outcome = SelectorOutcome::Resolved(cousin_window);
                    } else {
                        daemon_fail!(response, "could not locate second cousin of window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_PREVIOUS,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let previous_window = managed_window_before_window_in_active_space(
                        space_manager,
                        window_manager,
                        acting_window,
                        display_manager,
                    );
                    if let Some(previous_window) = previous_window {
                        result.outcome = SelectorOutcome::Resolved(previous_window);
                    } else {
                        daemon_fail!(response, "could not locate the prev managed window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_NEXT,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let next_window = managed_window_after_window_in_active_space(
                        space_manager,
                        window_manager,
                        acting_window,
                        display_manager,
                    );
                    if let Some(next_window) = next_window {
                        result.outcome = SelectorOutcome::Resolved(next_window);
                    } else {
                        daemon_fail!(response, "could not locate the next managed window.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_FIRST,
            ) {
                let first_window = first_managed_window_in_active_space(
                    space_manager,
                    window_manager,
                    display_manager,
                );
                if let Some(first_window) = first_window {
                    result.outcome = SelectorOutcome::Resolved(first_window);
                } else {
                    daemon_fail!(response, "could not locate the first managed window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_LAST,
            ) {
                let last_window = last_managed_window_in_active_space(
                    space_manager,
                    window_manager,
                    display_manager,
                );
                if let Some(last_window) = last_window {
                    result.outcome = SelectorOutcome::Resolved(last_window);
                } else {
                    daemon_fail!(response, "could not locate the last managed window.\n");
                }
            } else if is_token_equal_to(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_RECENT,
            ) {
                let recent_window = previously_focused_window_if_managed(window_manager);
                if let Some(recent_window) = recent_window {
                    result.outcome = SelectorOutcome::Resolved(recent_window);
                } else {
                    daemon_fail!(
                        response,
                        "could not locate the most recently focused window.\n"
                    );
                }
            } else if is_token_prefixed_by(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SELECTOR_STACK_PREFIX,
            ) {
                if let Some(acting_window) = acting_window_id {
                    result.token.start += ARGUMENT_COMMON_SELECTOR_STACK_PREFIX.len();
                    result.token.length -= ARGUMENT_COMMON_SELECTOR_STACK_PREFIX.len();

                    if is_token_equal_to(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SELECTOR_PREVIOUS,
                    ) {
                        let previous_window = previous_window_in_stack_holding_window(
                            space_manager,
                            window_manager,
                            acting_window,
                            display_manager,
                        );
                        if let Some(previous_window) = previous_window {
                            result.outcome = SelectorOutcome::Resolved(previous_window);
                        } else {
                            daemon_fail!(response, "could not locate the prev stacked window.\n");
                        }
                    } else if is_token_equal_to(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SELECTOR_NEXT,
                    ) {
                        let next_window = next_window_in_stack_holding_window(
                            space_manager,
                            window_manager,
                            acting_window,
                            display_manager,
                        );
                        if let Some(next_window) = next_window {
                            result.outcome = SelectorOutcome::Resolved(next_window);
                        } else {
                            daemon_fail!(response, "could not locate the next stacked window.\n");
                        }
                    } else if is_token_equal_to(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SELECTOR_FIRST,
                    ) {
                        let first_window = first_window_in_stack_holding_window(
                            space_manager,
                            window_manager,
                            acting_window,
                            display_manager,
                        );
                        if let Some(first_window) = first_window {
                            result.outcome = SelectorOutcome::Resolved(first_window);
                        } else {
                            daemon_fail!(response, "could not locate the first stacked window.\n");
                        }
                    } else if is_token_equal_to(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SELECTOR_LAST,
                    ) {
                        let last_window = last_window_in_stack_holding_window(
                            space_manager,
                            window_manager,
                            acting_window,
                            display_manager,
                        );
                        if let Some(last_window) = last_window {
                            result.outcome = SelectorOutcome::Resolved(last_window);
                        } else {
                            daemon_fail!(response, "could not locate the last stacked window.\n");
                        }
                    } else if is_token_equal_to(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SELECTOR_RECENT,
                    ) {
                        let recent_window = previously_focused_window_in_stack_holding_window(
                            space_manager,
                            window_manager,
                            acting_window,
                            display_manager,
                        );
                        if let Some(recent_window) = recent_window {
                            result.outcome = SelectorOutcome::Resolved(recent_window);
                        } else {
                            daemon_fail!(response, "could not locate the recent stacked window.\n");
                        }
                    } else if result.token.is_not_empty()
                        && let Some(index) = parse_token_as_non_negative_decimal_integer(
                            result.token,
                            message_cursor.bytes(),
                        )
                        && index > 0
                    {
                        let index_window = window_at_one_based_position_in_stack_holding_window(
                            space_manager,
                            window_manager,
                            acting_window,
                            index,
                            display_manager,
                        );
                        if let Some(index_window) = index_window {
                            result.outcome = SelectorOutcome::Resolved(index_window);
                        } else {
                            daemon_fail!(
                                response,
                                "could not locate the stacked window in position {}.\n",
                                index
                            );
                        }
                    } else {
                        result.outcome = SelectorOutcome::NotASelector;
                        response.write_failure_pieces_unless_silent(&[
                            FailurePiece::Text("value '"),
                            FailurePiece::Text(ARGUMENT_COMMON_SELECTOR_STACK_PREFIX),
                            FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                            FailurePiece::Text("' is not a valid option for WINDOW_SEL\n"),
                        ]);
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else {
                result.outcome = SelectorOutcome::NotASelector;
                response.write_failure_pieces_unless_silent(&[
                    FailurePiece::Text("value '"),
                    FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                    FailurePiece::Text("' is not a valid option for WINDOW_SEL\n"),
                ]);
            }
        }
        TokenValueType::Invalid => {
            result.outcome = SelectorOutcome::NotASelector;
            if !optional {
                response.write_failure_pieces_unless_silent(&[
                    FailurePiece::Text("value '"),
                    FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                    FailurePiece::Text("' is not a valid option for WINDOW_SEL\n"),
                ]);
            }
        }
        TokenValueType::Float(_) | TokenValueType::Hexadecimal(_) => {
            result.outcome = SelectorOutcome::NotASelector;
            response.write_failure_pieces_unless_silent(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for WINDOW_SEL\n"),
            ]);
        }
    }

    result
}

pub(crate) fn parse_insertion_direction_selector(
    response: &mut Response,
    message_cursor: &mut MessageCursor,
) -> Selector<i32> {
    let mut result = Selector {
        token: message_cursor.take_next_token(),
        outcome: SelectorOutcome::ParsedButUnresolved,
    };

    if is_token_equal_to(
        result.token,
        message_cursor.bytes(),
        ARGUMENT_COMMON_SELECTOR_NORTH,
    ) {
        result.outcome = SelectorOutcome::Resolved(DIRECTION_NORTH);
    } else if is_token_equal_to(
        result.token,
        message_cursor.bytes(),
        ARGUMENT_COMMON_SELECTOR_EAST,
    ) {
        result.outcome = SelectorOutcome::Resolved(DIRECTION_EAST);
    } else if is_token_equal_to(
        result.token,
        message_cursor.bytes(),
        ARGUMENT_COMMON_SELECTOR_SOUTH,
    ) {
        result.outcome = SelectorOutcome::Resolved(DIRECTION_SOUTH);
    } else if is_token_equal_to(
        result.token,
        message_cursor.bytes(),
        ARGUMENT_COMMON_SELECTOR_WEST,
    ) {
        result.outcome = SelectorOutcome::Resolved(DIRECTION_WEST);
    } else if is_token_equal_to(
        result.token,
        message_cursor.bytes(),
        ARGUMENT_COMMON_SELECTOR_STACK,
    ) {
        result.outcome = SelectorOutcome::Resolved(DIRECTION_STACK_INSTEAD_OF_SPLIT);
    } else {
        result.outcome = SelectorOutcome::NotASelector;
        response.write_failure_pieces_unless_silent(&[
            FailurePiece::Text("value '"),
            FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
            FailurePiece::Text("' is not a valid option for DIR_SEL\n"),
        ]);
    }

    result
}
