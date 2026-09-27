use crate::daemon_fail;
use crate::display::arrangement::{
    display_manager_arrangement_display_id, display_manager_find_closest_display_in_direction,
    display_manager_first_display_id, display_manager_last_display_id,
    display_manager_next_display_id, display_manager_prev_display_id,
};
use crate::display::identity::display_manager_cursor_display_id;
use crate::display::labels::display_manager_get_display_for_label;
use crate::display::manager::DisplayManager;
use crate::message::common_arguments::{
    ARGUMENT_COMMON_SEL_EAST, ARGUMENT_COMMON_SEL_FIRST, ARGUMENT_COMMON_SEL_LAST,
    ARGUMENT_COMMON_SEL_MOUSE, ARGUMENT_COMMON_SEL_NEXT, ARGUMENT_COMMON_SEL_NORTH,
    ARGUMENT_COMMON_SEL_PREV, ARGUMENT_COMMON_SEL_RECENT, ARGUMENT_COMMON_SEL_SOUTH,
    ARGUMENT_COMMON_SEL_STACK, ARGUMENT_COMMON_SEL_STACK_PREFIX, ARGUMENT_COMMON_SEL_WEST,
};
use crate::message::token::{
    MessageCursor, Token, TokenType, c_string_at, token_equals, token_is_positive_integer,
    token_prefix, token_to_value,
};
use crate::space::labels::space_manager_get_space_for_label;
use crate::space::lookup::{
    space_manager_cursor_space, space_manager_first_space, space_manager_last_space,
    space_manager_mission_control_space, space_manager_next_space, space_manager_prev_space,
};
use crate::space::manager::SpaceManager;
use crate::support::direction::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST, STACK};
use crate::support::handles::{DisplayId, SpaceId, WindowId};
use crate::support::response::{FailurePiece, Response};
use crate::window::manager::{WindowManager, window_manager_find_window};
use crate::window::screen_lookup::window_manager_find_window_below_cursor;
use crate::window::stack_lookup::{
    window_manager_find_first_window_in_stack, window_manager_find_last_window_in_stack,
    window_manager_find_next_window_in_stack, window_manager_find_prev_window_in_stack,
    window_manager_find_recent_window_in_stack, window_manager_find_window_in_stack,
};
use crate::window::tree_lookup::{
    window_manager_find_closest_managed_window_in_direction,
    window_manager_find_first_cousin_for_managed_window, window_manager_find_first_managed_window,
    window_manager_find_first_nephew_for_managed_window,
    window_manager_find_largest_managed_window, window_manager_find_last_managed_window,
    window_manager_find_next_managed_window, window_manager_find_prev_managed_window,
    window_manager_find_recent_managed_window,
    window_manager_find_second_cousin_for_managed_window,
    window_manager_find_second_nephew_for_managed_window,
    window_manager_find_sibling_for_managed_window, window_manager_find_smallest_managed_window,
    window_manager_find_uncle_for_managed_window,
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
    pub(crate) fn did_parse(&self) -> bool {
        !matches!(self.outcome, SelectorOutcome::NotASelector)
    }

    pub(crate) fn resolved(&self) -> Option<Target> {
        match self.outcome {
            SelectorOutcome::Resolved(value) => Some(value),
            _ => None,
        }
    }
}

pub(crate) const ARGUMENT_WINDOW_SEL_LARGEST: &str = "largest";
pub(crate) const ARGUMENT_WINDOW_SEL_SMALLEST: &str = "smallest";
pub(crate) const ARGUMENT_WINDOW_SEL_SIBLING: &str = "sibling";
pub(crate) const ARGUMENT_WINDOW_SEL_FNEPHEW: &str = "first_nephew";
pub(crate) const ARGUMENT_WINDOW_SEL_SNEPHEW: &str = "second_nephew";
pub(crate) const ARGUMENT_WINDOW_SEL_UNCLE: &str = "uncle";
pub(crate) const ARGUMENT_WINDOW_SEL_FCOUSIN: &str = "first_cousin";
pub(crate) const ARGUMENT_WINDOW_SEL_SCOUSIN: &str = "second_cousin";

pub(crate) fn parse_display_selector(
    response: &mut Response,
    message_cursor: &mut MessageCursor,
    acting_display_id: DisplayId,
    optional: bool,
    display_manager: &mut DisplayManager,
) -> Selector<DisplayId> {
    let mut result = Selector {
        token: message_cursor.get_token(),
        outcome: SelectorOutcome::ParsedButUnresolved,
    };

    let value = token_to_value(result.token, message_cursor.bytes());
    match value.type_of_value {
        TokenType::Int(int_value) => {
            let display_id = display_manager_arrangement_display_id(int_value, display_manager);
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
        TokenType::String => {
            if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_NORTH,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id = display_manager_find_closest_display_in_direction(
                        acting_display_id,
                        DIR_NORTH,
                    );
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate a northward display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_EAST,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id = display_manager_find_closest_display_in_direction(
                        acting_display_id,
                        DIR_EAST,
                    );
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate a eastward display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_SOUTH,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id = display_manager_find_closest_display_in_direction(
                        acting_display_id,
                        DIR_SOUTH,
                    );
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate a southward display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_WEST,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id = display_manager_find_closest_display_in_direction(
                        acting_display_id,
                        DIR_WEST,
                    );
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate a westward display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_PREV,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id =
                        display_manager_prev_display_id(acting_display_id, display_manager);
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate the previous display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_NEXT,
            ) {
                if acting_display_id != DisplayId(0) {
                    let display_id =
                        display_manager_next_display_id(acting_display_id, display_manager);
                    if display_id != DisplayId(0) {
                        result.outcome = SelectorOutcome::Resolved(display_id);
                    } else {
                        daemon_fail!(response, "could not locate the next display.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected display.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_FIRST,
            ) {
                let display_id = display_manager_first_display_id(display_manager);
                if display_id != DisplayId(0) {
                    result.outcome = SelectorOutcome::Resolved(display_id);
                } else {
                    daemon_fail!(response, "could not locate the first display.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_LAST,
            ) {
                let display_id = display_manager_last_display_id(display_manager);
                if display_id != DisplayId(0) {
                    result.outcome = SelectorOutcome::Resolved(display_id);
                } else {
                    daemon_fail!(response, "could not locate the last display.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_RECENT,
            ) {
                if display_manager.last_display_id != DisplayId(0) {
                    result.outcome = SelectorOutcome::Resolved(display_manager.last_display_id);
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_MOUSE,
            ) {
                let display_id = display_manager_cursor_display_id();
                if display_id != DisplayId(0) {
                    result.outcome = SelectorOutcome::Resolved(display_id);
                } else {
                    daemon_fail!(response, "could not locate display containing cursor.\n");
                }
            } else {
                let display_id = display_manager_get_display_for_label(
                    display_manager,
                    c_string_at(message_cursor.bytes(), value.token.start),
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
                    response.fail_pieces(&[
                        FailurePiece::Text("value '"),
                        FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' is not a valid option for DISPLAY_SEL\n"),
                    ]);
                }
            }
        }
        TokenType::Invalid => {
            result.outcome = SelectorOutcome::NotASelector;
            if !optional {
                response.fail_pieces(&[
                    FailurePiece::Text("value '"),
                    FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                    FailurePiece::Text("' is not a valid option for DISPLAY_SEL\n"),
                ]);
            }
        }
        TokenType::Float(_) | TokenType::U32(_) => {
            result.outcome = SelectorOutcome::NotASelector;
            response.fail_pieces(&[
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
        token: message_cursor.get_token(),
        outcome: SelectorOutcome::ParsedButUnresolved,
    };

    let value = token_to_value(result.token, message_cursor.bytes());
    match value.type_of_value {
        TokenType::Int(int_value) => {
            let space_id = space_manager_mission_control_space(int_value);
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
        TokenType::String => {
            if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_PREV,
            ) {
                if acting_space_id != SpaceId(0) {
                    let space_id = space_manager_prev_space(acting_space_id);
                    if space_id != SpaceId(0) {
                        result.outcome = SelectorOutcome::Resolved(space_id);
                    } else {
                        daemon_fail!(response, "could not locate the previous space.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected space.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_NEXT,
            ) {
                if acting_space_id != SpaceId(0) {
                    let space_id = space_manager_next_space(acting_space_id);
                    if space_id != SpaceId(0) {
                        result.outcome = SelectorOutcome::Resolved(space_id);
                    } else {
                        daemon_fail!(response, "could not locate the next space.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected space.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_FIRST,
            ) {
                let space_id = space_manager_first_space();
                if space_id != SpaceId(0) {
                    result.outcome = SelectorOutcome::Resolved(space_id);
                } else {
                    daemon_fail!(response, "could not locate the first space.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_LAST,
            ) {
                let space_id = space_manager_last_space();
                if space_id != SpaceId(0) {
                    result.outcome = SelectorOutcome::Resolved(space_id);
                } else {
                    daemon_fail!(response, "could not locate the last space.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_RECENT,
            ) {
                if space_manager.last_space_id != SpaceId(0) {
                    result.outcome = SelectorOutcome::Resolved(space_manager.last_space_id);
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_MOUSE,
            ) {
                let space_id = space_manager_cursor_space();
                if space_id != SpaceId(0) {
                    result.outcome = SelectorOutcome::Resolved(space_id);
                } else {
                    daemon_fail!(response, "could not locate space containing cursor.\n");
                }
            } else {
                let space_id = space_manager_get_space_for_label(
                    space_manager,
                    c_string_at(message_cursor.bytes(), value.token.start),
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
                    response.fail_pieces(&[
                        FailurePiece::Text("value '"),
                        FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' is not a valid option for SPACE_SEL\n"),
                    ]);
                }
            }
        }
        TokenType::Invalid => {
            result.outcome = SelectorOutcome::NotASelector;
            if !optional {
                response.fail_pieces(&[
                    FailurePiece::Text("value '"),
                    FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                    FailurePiece::Text("' is not a valid option for SPACE_SEL\n"),
                ]);
            }
        }
        TokenType::Float(_) | TokenType::U32(_) => {
            result.outcome = SelectorOutcome::NotASelector;
            response.fail_pieces(&[
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
        token: message_cursor.get_token(),
        outcome: SelectorOutcome::ParsedButUnresolved,
    };

    let value = token_to_value(result.token, message_cursor.bytes());
    match value.type_of_value {
        TokenType::Int(int_value) => {
            let window = window_manager_find_window(window_manager, WindowId(int_value as u32));
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
        TokenType::String => {
            if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_NORTH,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let closest_window = window_manager_find_closest_managed_window_in_direction(
                        window_manager,
                        acting_window,
                        DIR_NORTH,
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_EAST,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let closest_window = window_manager_find_closest_managed_window_in_direction(
                        window_manager,
                        acting_window,
                        DIR_EAST,
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_SOUTH,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let closest_window = window_manager_find_closest_managed_window_in_direction(
                        window_manager,
                        acting_window,
                        DIR_SOUTH,
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_WEST,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let closest_window = window_manager_find_closest_managed_window_in_direction(
                        window_manager,
                        acting_window,
                        DIR_WEST,
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_MOUSE,
            ) {
                let mouse_window = window_manager_find_window_below_cursor(window_manager);
                if let Some(mouse_window) = mouse_window {
                    result.outcome = SelectorOutcome::Resolved(mouse_window);
                } else {
                    daemon_fail!(response, "could not locate a window below the cursor.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SEL_LARGEST,
            ) {
                let area_window = window_manager_find_largest_managed_window(
                    space_manager,
                    window_manager,
                    display_manager,
                );
                if let Some(area_window) = area_window {
                    result.outcome = SelectorOutcome::Resolved(area_window);
                } else {
                    daemon_fail!(response, "could not locate window with the largest area.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SEL_SMALLEST,
            ) {
                let area_window = window_manager_find_smallest_managed_window(
                    space_manager,
                    window_manager,
                    display_manager,
                );
                if let Some(area_window) = area_window {
                    result.outcome = SelectorOutcome::Resolved(area_window);
                } else {
                    daemon_fail!(response, "could not locate window with the smallest area.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SEL_SIBLING,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let sibling_window = window_manager_find_sibling_for_managed_window(
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SEL_FNEPHEW,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let nephew_window = window_manager_find_first_nephew_for_managed_window(
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SEL_SNEPHEW,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let nephew_window = window_manager_find_second_nephew_for_managed_window(
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SEL_UNCLE,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let uncle_window = window_manager_find_uncle_for_managed_window(
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SEL_FCOUSIN,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let cousin_window = window_manager_find_first_cousin_for_managed_window(
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_SEL_SCOUSIN,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let cousin_window = window_manager_find_second_cousin_for_managed_window(
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_PREV,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let previous_window = window_manager_find_prev_managed_window(
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_NEXT,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let next_window = window_manager_find_next_managed_window(
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
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_FIRST,
            ) {
                let first_window = window_manager_find_first_managed_window(
                    space_manager,
                    window_manager,
                    display_manager,
                );
                if let Some(first_window) = first_window {
                    result.outcome = SelectorOutcome::Resolved(first_window);
                } else {
                    daemon_fail!(response, "could not locate the first managed window.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_LAST,
            ) {
                let last_window = window_manager_find_last_managed_window(
                    space_manager,
                    window_manager,
                    display_manager,
                );
                if let Some(last_window) = last_window {
                    result.outcome = SelectorOutcome::Resolved(last_window);
                } else {
                    daemon_fail!(response, "could not locate the last managed window.\n");
                }
            } else if token_equals(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_RECENT,
            ) {
                let recent_window = window_manager_find_recent_managed_window(window_manager);
                if let Some(recent_window) = recent_window {
                    result.outcome = SelectorOutcome::Resolved(recent_window);
                } else {
                    daemon_fail!(
                        response,
                        "could not locate the most recently focused window.\n"
                    );
                }
            } else if token_prefix(
                result.token,
                message_cursor.bytes(),
                ARGUMENT_COMMON_SEL_STACK_PREFIX,
            ) {
                if let Some(acting_window) = acting_window_id {
                    result.token.start += ARGUMENT_COMMON_SEL_STACK_PREFIX.len();
                    result.token.length -= ARGUMENT_COMMON_SEL_STACK_PREFIX.len();

                    if token_equals(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SEL_PREV,
                    ) {
                        let previous_window = window_manager_find_prev_window_in_stack(
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
                    } else if token_equals(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SEL_NEXT,
                    ) {
                        let next_window = window_manager_find_next_window_in_stack(
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
                    } else if token_equals(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SEL_FIRST,
                    ) {
                        let first_window = window_manager_find_first_window_in_stack(
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
                    } else if token_equals(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SEL_LAST,
                    ) {
                        let last_window = window_manager_find_last_window_in_stack(
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
                    } else if token_equals(
                        result.token,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_SEL_RECENT,
                    ) {
                        let recent_window = window_manager_find_recent_window_in_stack(
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
                    } else if result.token.is_valid()
                        && let Some(index) =
                            token_is_positive_integer(result.token, message_cursor.bytes())
                        && index > 0
                    {
                        let index_window = window_manager_find_window_in_stack(
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
                        response.fail_pieces(&[
                            FailurePiece::Text("value '"),
                            FailurePiece::Text(ARGUMENT_COMMON_SEL_STACK_PREFIX),
                            FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                            FailurePiece::Text("' is not a valid option for WINDOW_SEL\n"),
                        ]);
                    }
                } else {
                    daemon_fail!(response, "could not locate the selected window.\n");
                }
            } else {
                result.outcome = SelectorOutcome::NotASelector;
                response.fail_pieces(&[
                    FailurePiece::Text("value '"),
                    FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                    FailurePiece::Text("' is not a valid option for WINDOW_SEL\n"),
                ]);
            }
        }
        TokenType::Invalid => {
            result.outcome = SelectorOutcome::NotASelector;
            if !optional {
                response.fail_pieces(&[
                    FailurePiece::Text("value '"),
                    FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                    FailurePiece::Text("' is not a valid option for WINDOW_SEL\n"),
                ]);
            }
        }
        TokenType::Float(_) | TokenType::U32(_) => {
            result.outcome = SelectorOutcome::NotASelector;
            response.fail_pieces(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for WINDOW_SEL\n"),
            ]);
        }
    }

    result
}

pub(crate) fn parse_insert_selector(
    response: &mut Response,
    message_cursor: &mut MessageCursor,
) -> Selector<i32> {
    let mut result = Selector {
        token: message_cursor.get_token(),
        outcome: SelectorOutcome::ParsedButUnresolved,
    };

    if token_equals(
        result.token,
        message_cursor.bytes(),
        ARGUMENT_COMMON_SEL_NORTH,
    ) {
        result.outcome = SelectorOutcome::Resolved(DIR_NORTH);
    } else if token_equals(result.token, message_cursor.bytes(), ARGUMENT_COMMON_SEL_EAST) {
        result.outcome = SelectorOutcome::Resolved(DIR_EAST);
    } else if token_equals(
        result.token,
        message_cursor.bytes(),
        ARGUMENT_COMMON_SEL_SOUTH,
    ) {
        result.outcome = SelectorOutcome::Resolved(DIR_SOUTH);
    } else if token_equals(result.token, message_cursor.bytes(), ARGUMENT_COMMON_SEL_WEST) {
        result.outcome = SelectorOutcome::Resolved(DIR_WEST);
    } else if token_equals(
        result.token,
        message_cursor.bytes(),
        ARGUMENT_COMMON_SEL_STACK,
    ) {
        result.outcome = SelectorOutcome::Resolved(STACK);
    } else {
        result.outcome = SelectorOutcome::NotASelector;
        response.fail_pieces(&[
            FailurePiece::Text("value '"),
            FailurePiece::Bytes(result.token.bytes(message_cursor.bytes())),
            FailurePiece::Text("' is not a valid option for DIR_SEL\n"),
        ]);
    }

    result
}
