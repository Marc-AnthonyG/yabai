use crate::daemon_fail;
use crate::display::identity::query_display_showing_the_active_menu_bar;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_current_space_of_display;
use crate::layout::tree::MOST_WINDOWS_A_NODE_CAN_HOLD;
use crate::message::argument_prefixes::{
    parse_absolute_or_relative_change_type, parse_resize_handle,
};
use crate::message::common_failures::{
    daemon_fail_with_unknown_command_for_domain,
    daemon_fail_with_unknown_value_given_to_command_for_domain,
};
use crate::message::labels::{LabelType, parse_label_refusing_numbers_and_reserved_words};
use crate::message::selectors::{
    parse_display_selector, parse_insertion_direction_selector, parse_space_selector,
    parse_window_selector,
};
use crate::message::token::{
    MessageCursor, Token, TokenValueType, is_token_equal_to, null_terminated_bytes_starting_at,
    parse_token_into_typed_value,
};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::scripting_addition::client::order_window_relative_to_other_window_through_scripting_addition;
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::managed_space::is_native_fullscreen_space;
use crate::space::manager::SpaceManager;
use crate::space::tiling::toggle_split_direction_of_the_parent_of_window_leaf;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::arithmetic::is_within_range_including_both_bounds;
use crate::support::handles::WindowId;
use crate::support::layer::{LAYER_ABOVE, LAYER_AUTO, LAYER_BELOW, LAYER_NORMAL};
use crate::support::response::Response;
use crate::support::strings::FIXED_STRING_BUFFER_LENGTH;
use crate::window::floating_and_sticky::{set_whether_window_floats, set_whether_window_is_sticky};
use crate::window::focus::{
    focus_and_raise_window_of_process, query_focused_tracked_window,
    toggle_application_expose_for_window,
};
use crate::window::frame::{
    adjust_split_ratio_of_managed_window_parent_node,
    move_floating_window_by_offset_or_to_position,
    resize_window_by_dragging_edges_or_to_absolute_size,
};
use crate::window::fullscreen::{
    toggle_managed_window_zoom_fullscreen, toggle_managed_window_zoom_parent,
    toggle_window_native_fullscreen, toggle_window_picture_in_picture,
    toggle_window_windowed_fullscreen,
};
use crate::window::grid::place_floating_window_on_display_grid;
use crate::window::group::toggle_group_of_window;
use crate::window::layer::set_window_layer_for_it_and_its_child_windows;
use crate::window::manager::{WindowManager, WindowOperationOutcome};
use crate::window::minimize_and_close::{
    close_window_by_pressing_its_close_button, deminimize_window_through_accessibility,
    minimize_window_through_accessibility,
};
use crate::window::model::{WindowFlag, is_window_flag_set};
use crate::window::opacity::apply_opacity_to_window_through_scripting_addition;
use crate::window::scratchpad::{
    assign_window_to_scratchpad_making_it_float,
    recover_hidden_scratchpad_windows_by_ordering_every_window_in,
    remove_window_from_its_scratchpad, toggle_scratchpad_window_with_label,
};
use crate::window::send_to_space::send_window_to_space;
use crate::window::shadow::toggle_window_shadow;
use crate::window::tree_placement::{
    stack_second_window_onto_the_node_of_first_window, swap_managed_windows,
    toggle_insertion_point_at_window_in_direction,
    warp_first_window_into_the_node_of_second_window,
};

/* --------------------------------DOMAIN WINDOW-------------------------------- */
pub(crate) const COMMAND_WINDOW_FOCUS: &str = "--focus";
pub(crate) const COMMAND_WINDOW_CLOSE: &str = "--close";
pub(crate) const COMMAND_WINDOW_MINIMIZE: &str = "--minimize";
pub(crate) const COMMAND_WINDOW_DEMINIMIZE: &str = "--deminimize";
pub(crate) const COMMAND_WINDOW_DISPLAY: &str = "--display";
pub(crate) const COMMAND_WINDOW_SPACE: &str = "--space";
pub(crate) const COMMAND_WINDOW_SWAP: &str = "--swap";
pub(crate) const COMMAND_WINDOW_WARP: &str = "--warp";
pub(crate) const COMMAND_WINDOW_STACK: &str = "--stack";
pub(crate) const COMMAND_WINDOW_INSERT: &str = "--insert";
pub(crate) const COMMAND_WINDOW_GRID: &str = "--grid";
pub(crate) const COMMAND_WINDOW_MOVE: &str = "--move";
pub(crate) const COMMAND_WINDOW_RESIZE: &str = "--resize";
pub(crate) const COMMAND_WINDOW_RATIO: &str = "--ratio";
pub(crate) const COMMAND_WINDOW_SUB_LAYER: &str = "--sub-layer";
pub(crate) const COMMAND_WINDOW_OPACITY: &str = "--opacity";
pub(crate) const COMMAND_WINDOW_RAISE: &str = "--raise";
pub(crate) const COMMAND_WINDOW_LOWER: &str = "--lower";
pub(crate) const COMMAND_WINDOW_TOGGLE: &str = "--toggle";
pub(crate) const COMMAND_WINDOW_SCRATCHPAD: &str = "--scratchpad";

pub(crate) const ARGUMENT_WINDOW_GRID: &std::ffi::CStr = c"%d:%d:%d:%d:%d:%d";
pub(crate) const ARGUMENT_WINDOW_MOVE: &std::ffi::CStr = c"%255[^:]:%f:%f";
pub(crate) const ARGUMENT_WINDOW_RESIZE: &std::ffi::CStr = c"%255[^:]:%f:%f";
pub(crate) const ARGUMENT_WINDOW_RATIO: &std::ffi::CStr = c"%255[^:]:%f";
pub(crate) const ARGUMENT_WINDOW_LAYER_BELOW: &str = "below";
pub(crate) const ARGUMENT_WINDOW_LAYER_NORMAL: &str = "normal";
pub(crate) const ARGUMENT_WINDOW_LAYER_ABOVE: &str = "above";
pub(crate) const ARGUMENT_WINDOW_LAYER_AUTO: &str = "auto";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_FLOAT: &str = "float";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_STICKY: &str = "sticky";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_SHADOW: &str = "shadow";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_SPLIT: &str = "split";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_PARENT: &str = "zoom-parent";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_ZOOM_FULLSCREEN: &str = "zoom-fullscreen";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_WINDOWED: &str = "windowed-fullscreen";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_NATIVE: &str = "native-fullscreen";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_EXPOSE: &str = "expose";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_PICTURE_IN_PICTURE: &str = "pip";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_GROUP: &str = "group";

pub(crate) const ARGUMENT_WINDOW_SCRATCHPAD_RECOVER: &str = "recover";
/* ----------------------------------------------------------------------------- */

pub(crate) fn run_window_command(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let mut command;
    let mut acting_window_id = query_focused_tracked_window(window_manager);
    let selector = parse_window_selector(
        &mut Response::silent(),
        message_cursor,
        acting_window_id,
        true,
        display_manager,
        window_manager,
        space_manager,
    );

    if selector.is_recognised_selector() {
        acting_window_id = selector.resolved_target();
        command = message_cursor.take_next_token();
    } else {
        command = selector.token;
    }

    while command.is_not_empty() {
        if acting_window_id.is_none()
            && !is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_FOCUS)
            && !is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_CLOSE)
            && !is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_MINIMIZE)
            && !is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_DEMINIMIZE)
            && !is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_TOGGLE)
        {
            daemon_fail!(response, "could not locate the window to act on!\n");
            return;
        }

        if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_FOCUS) {
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_window_id) = selector.resolved_target() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                let window = window_manager.window.find(&acting_window);
                let window_element_ref = window.map(|window| window.element_ref);
                let window_process_serial_number = window
                    .and_then(|window| window.application)
                    .and_then(|process_id| window_manager.application.find(&process_id))
                    .map(|application| application.process_serial_number);
                if let (Some(window_process_serial_number), Some(window_element_ref)) =
                    (window_process_serial_number, window_element_ref)
                {
                    focus_and_raise_window_of_process(
                        &window_process_serial_number,
                        acting_window,
                        window_element_ref,
                    );
                }
            } else {
                daemon_fail!(response, "could not locate the window to act on!\n");
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_CLOSE) {
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_window_id) = selector.resolved_target() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                if !close_window_by_pressing_its_close_button(acting_window, window_manager) {
                    daemon_fail!(
                        response,
                        "could not close window with id '{}'.\n",
                        acting_window.0 as i32
                    );
                }
            } else {
                daemon_fail!(response, "could not locate the window to act on!\n");
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_MINIMIZE) {
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_not_empty() {
                if let Some(selector_window_id) = selector.resolved_target() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                let result = minimize_window_through_accessibility(acting_window, window_manager);
                if result == WindowOperationOutcome::CannotMinimize {
                    daemon_fail!(
                        response,
                        "window with id '{}' does not support the minimize operation.\n",
                        acting_window.0 as i32
                    );
                } else if result == WindowOperationOutcome::AlreadyMinimized {
                    daemon_fail!(
                        response,
                        "window with id '{}' is already minimized.\n",
                        acting_window.0 as i32
                    );
                } else if result == WindowOperationOutcome::MinimizeFailed {
                    daemon_fail!(
                        response,
                        "could not minimize window with id '{}'.\n",
                        acting_window.0 as i32
                    );
                }
            } else {
                daemon_fail!(response, "could not locate the window to act on!\n");
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_DEMINIMIZE) {
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                false,
                display_manager,
                window_manager,
                space_manager,
            );
            if let Some(selector_window_id) = selector.resolved_target() {
                let result =
                    deminimize_window_through_accessibility(selector_window_id, window_manager);
                if result == WindowOperationOutcome::NotMinimized {
                    daemon_fail!(
                        response,
                        "window with id '{}' is not minimized.\n",
                        selector_window_id.0 as i32
                    );
                } else if result == WindowOperationOutcome::DeminimizeFailed {
                    daemon_fail!(
                        response,
                        "could not deminimize window with id '{}'.\n",
                        selector_window_id.0 as i32
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_DISPLAY) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_display_selector(
                    response,
                    message_cursor,
                    query_display_showing_the_active_menu_bar(),
                    false,
                    display_manager,
                );
                if let Some(selector_display_id) = selector.resolved_target() {
                    let space_id = query_current_space_of_display(selector_display_id);
                    if is_native_fullscreen_space(space_id) {
                        daemon_fail!(
                            response,
                            "can not move window to a macOS fullscreen space!\n"
                        );
                    } else {
                        send_window_to_space(
                            space_manager,
                            window_manager,
                            acting_window,
                            space_id,
                            false,
                            process_manager,
                            display_manager,
                            mouse_drag_state,
                        );
                    }
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_SPACE) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_space_selector(
                    response,
                    message_cursor,
                    query_current_space_of_the_focused_display(window_manager),
                    false,
                    space_manager,
                );
                if let Some(selector_space_id) = selector.resolved_target() {
                    if is_native_fullscreen_space(selector_space_id) {
                        daemon_fail!(
                            response,
                            "can not move window to a macOS fullscreen space!\n"
                        );
                    } else {
                        send_window_to_space(
                            space_manager,
                            window_manager,
                            acting_window,
                            selector_space_id,
                            false,
                            process_manager,
                            display_manager,
                            mouse_drag_state,
                        );
                    }
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_SWAP) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_window_selector(
                    response,
                    message_cursor,
                    acting_window_id,
                    false,
                    display_manager,
                    window_manager,
                    space_manager,
                );
                if let Some(selector_window_id) = selector.resolved_target() {
                    let result = swap_managed_windows(
                        space_manager,
                        window_manager,
                        acting_window,
                        selector_window_id,
                        display_manager,
                    );
                    if result == WindowOperationOutcome::InvalidSourceView {
                        daemon_fail!(response, "the acting window is not within a bsp space.\n");
                    } else if result == WindowOperationOutcome::InvalidDestinationView {
                        daemon_fail!(response, "the selected window is not within a bsp space.\n");
                    } else if result == WindowOperationOutcome::InvalidSourceNode {
                        daemon_fail!(response, "the acting window is not managed.\n");
                    } else if result == WindowOperationOutcome::InvalidDestinationNode {
                        daemon_fail!(response, "the selected window is not managed.\n");
                    } else if result == WindowOperationOutcome::SameStack {
                        daemon_fail!(
                            response,
                            "cannot swap a window with a window in the same stack.\n"
                        );
                    } else if result == WindowOperationOutcome::SameWindow {
                        daemon_fail!(response, "cannot swap a window with itself.\n");
                    }
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_WARP) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_window_selector(
                    response,
                    message_cursor,
                    acting_window_id,
                    false,
                    display_manager,
                    window_manager,
                    space_manager,
                );
                if let Some(selector_window_id) = selector.resolved_target() {
                    let result = warp_first_window_into_the_node_of_second_window(
                        space_manager,
                        window_manager,
                        acting_window,
                        selector_window_id,
                        process_manager,
                        display_manager,
                        mouse_drag_state,
                    );
                    if result == WindowOperationOutcome::InvalidSourceView {
                        daemon_fail!(response, "the acting window is not within a bsp space.\n");
                    } else if result == WindowOperationOutcome::InvalidDestinationView {
                        daemon_fail!(response, "the selected window is not within a bsp space.\n");
                    } else if result == WindowOperationOutcome::InvalidSourceNode {
                        daemon_fail!(response, "the acting window is not managed.\n");
                    } else if result == WindowOperationOutcome::InvalidDestinationNode {
                        daemon_fail!(response, "the selected window is not managed.\n");
                    } else if result == WindowOperationOutcome::SameStack {
                        daemon_fail!(
                            response,
                            "cannot warp a window with a window in the same stack.\n"
                        );
                    } else if result == WindowOperationOutcome::SameWindow {
                        daemon_fail!(response, "cannot warp a window onto itself.\n");
                    }
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_STACK) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_window_selector(
                    response,
                    message_cursor,
                    acting_window_id,
                    false,
                    display_manager,
                    window_manager,
                    space_manager,
                );
                if let Some(selector_window_id) = selector.resolved_target() {
                    let result = stack_second_window_onto_the_node_of_first_window(
                        space_manager,
                        window_manager,
                        acting_window,
                        selector_window_id,
                        display_manager,
                        mouse_drag_state,
                    );
                    if result == WindowOperationOutcome::InvalidSourceNode {
                        daemon_fail!(response, "the acting window is not managed.\n");
                    } else if result == WindowOperationOutcome::StackIsFull {
                        daemon_fail!(
                            response,
                            "cannot stack window, max capacity of {} reached.\n",
                            MOST_WINDOWS_A_NODE_CAN_HOLD as i32
                        );
                    } else if result == WindowOperationOutcome::SameWindow {
                        daemon_fail!(response, "cannot stack a window onto itself.\n");
                    }
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_INSERT) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_insertion_direction_selector(response, message_cursor);
                if let Some(direction) = selector.resolved_target() {
                    let result = toggle_insertion_point_at_window_in_direction(
                        space_manager,
                        acting_window,
                        direction,
                        display_manager,
                        window_manager,
                    );
                    if result == WindowOperationOutcome::InvalidSourceView {
                        daemon_fail!(response, "the acting window is not within a bsp space.\n");
                    } else if result == WindowOperationOutcome::InvalidSourceNode {
                        daemon_fail!(response, "the acting window is not managed.\n");
                    }
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_GRID) {
            if let Some(acting_window) = acting_window_id {
                let mut rows: libc::c_int = 0;
                let mut columns: libc::c_int = 0;
                let mut x: libc::c_int = 0;
                let mut y: libc::c_int = 0;
                let mut width: libc::c_int = 0;
                let mut height: libc::c_int = 0;
                let value = message_cursor.take_next_token();
                let subject =
                    std::ffi::CString::new(value.bytes(message_cursor.bytes())).unwrap_or_default();
                let converted = unsafe {
                    libc::sscanf(
                        subject.as_ptr(),
                        ARGUMENT_WINDOW_GRID.as_ptr(),
                        &mut rows as *mut libc::c_int,
                        &mut columns as *mut libc::c_int,
                        &mut x as *mut libc::c_int,
                        &mut y as *mut libc::c_int,
                        &mut width as *mut libc::c_int,
                        &mut height as *mut libc::c_int,
                    )
                };
                if converted == 6 {
                    let result = place_floating_window_on_display_grid(
                        space_manager,
                        window_manager,
                        acting_window,
                        rows as u32,
                        columns as u32,
                        x as u32,
                        y as u32,
                        width as u32,
                        height as u32,
                        display_manager,
                    );
                    if result == WindowOperationOutcome::InvalidSourceView {
                        daemon_fail!(response, "cannot apply grid layout to a managed window.\n");
                    }
                } else {
                    daemon_fail_with_unknown_value_given_to_command_for_domain(
                        response,
                        message_cursor.bytes(),
                        value,
                        command,
                        domain,
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_MOVE) {
            if let Some(acting_window) = acting_window_id {
                let mut x: libc::c_float = 0.0;
                let mut y: libc::c_float = 0.0;
                let mut type_of_change = [0 as libc::c_char; FIXED_STRING_BUFFER_LENGTH];
                let value = message_cursor.take_next_token();
                let subject =
                    std::ffi::CString::new(value.bytes(message_cursor.bytes())).unwrap_or_default();
                let converted = unsafe {
                    libc::sscanf(
                        subject.as_ptr(),
                        ARGUMENT_WINDOW_MOVE.as_ptr(),
                        type_of_change.as_mut_ptr(),
                        &mut x as *mut libc::c_float,
                        &mut y as *mut libc::c_float,
                    )
                };
                if converted == 3 {
                    let result = move_floating_window_by_offset_or_to_position(
                        window_manager,
                        acting_window,
                        parse_absolute_or_relative_change_type(&type_of_change) as i32,
                        x,
                        y,
                    );
                    if result == WindowOperationOutcome::InvalidSourceView {
                        daemon_fail!(response, "cannot move a managed window.\n");
                    }
                } else {
                    daemon_fail_with_unknown_value_given_to_command_for_domain(
                        response,
                        message_cursor.bytes(),
                        value,
                        command,
                        domain,
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_RESIZE) {
            if let Some(acting_window) = acting_window_id {
                let mut width: libc::c_float = 0.0;
                let mut height: libc::c_float = 0.0;
                let mut handle = [0 as libc::c_char; FIXED_STRING_BUFFER_LENGTH];
                let value = message_cursor.take_next_token();
                let subject =
                    std::ffi::CString::new(value.bytes(message_cursor.bytes())).unwrap_or_default();
                let converted = unsafe {
                    libc::sscanf(
                        subject.as_ptr(),
                        ARGUMENT_WINDOW_RESIZE.as_ptr(),
                        handle.as_mut_ptr(),
                        &mut width as *mut libc::c_float,
                        &mut height as *mut libc::c_float,
                    )
                };
                if converted == 3 {
                    let result = resize_window_by_dragging_edges_or_to_absolute_size(
                        window_manager,
                        acting_window,
                        parse_resize_handle(&handle) as i32,
                        width,
                        height,
                        true,
                        display_manager,
                        space_manager,
                    );
                    if result == WindowOperationOutcome::InvalidSourceNode {
                        daemon_fail!(response, "cannot locate bsp node for the managed window.\n");
                    } else if result == WindowOperationOutcome::InvalidDestinationNode {
                        daemon_fail!(response, "cannot locate a bsp node fence.\n");
                    } else if result == WindowOperationOutcome::InvalidOperation {
                        daemon_fail!(
                            response,
                            "cannot use absolute resizing on a managed window.\n"
                        );
                    }
                } else {
                    daemon_fail_with_unknown_value_given_to_command_for_domain(
                        response,
                        message_cursor.bytes(),
                        value,
                        command,
                        domain,
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_RATIO) {
            if let Some(acting_window) = acting_window_id {
                let mut ratio: libc::c_float = 0.0;
                let mut type_of_change = [0 as libc::c_char; FIXED_STRING_BUFFER_LENGTH];
                let value = message_cursor.take_next_token();
                let subject =
                    std::ffi::CString::new(value.bytes(message_cursor.bytes())).unwrap_or_default();
                let converted = unsafe {
                    libc::sscanf(
                        subject.as_ptr(),
                        ARGUMENT_WINDOW_RATIO.as_ptr(),
                        type_of_change.as_mut_ptr(),
                        &mut ratio as *mut libc::c_float,
                    )
                };
                if converted == 2 {
                    let result = adjust_split_ratio_of_managed_window_parent_node(
                        window_manager,
                        acting_window,
                        parse_absolute_or_relative_change_type(&type_of_change) as i32,
                        ratio,
                        space_manager,
                    );
                    if result == WindowOperationOutcome::InvalidSourceView {
                        daemon_fail!(response, "cannot adjust ratio of a non-managed window.\n");
                    } else if result == WindowOperationOutcome::InvalidSourceNode {
                        daemon_fail!(response, "cannot adjust ratio of a root node.\n");
                    }
                } else {
                    daemon_fail_with_unknown_value_given_to_command_for_domain(
                        response,
                        message_cursor.bytes(),
                        value,
                        command,
                        domain,
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_TOGGLE) {
            let value = message_cursor.take_next_token();
            if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_FLOAT) {
                if let Some(acting_window) = acting_window_id {
                    let should_float = window_manager
                        .window
                        .find(&acting_window)
                        .map(|window| !is_window_flag_set(window, WindowFlag::FLOATING));
                    if let Some(should_float) = should_float {
                        set_whether_window_floats(
                            space_manager,
                            window_manager,
                            acting_window,
                            should_float,
                            false,
                            display_manager,
                            mouse_drag_state,
                        );
                    }
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_TOGGLE_STICKY,
            ) {
                if let Some(acting_window) = acting_window_id {
                    let should_sticky = window_manager
                        .window
                        .find(&acting_window)
                        .map(|window| !is_window_flag_set(window, WindowFlag::STICKY));
                    if let Some(should_sticky) = should_sticky {
                        set_whether_window_is_sticky(
                            space_manager,
                            window_manager,
                            acting_window,
                            should_sticky,
                            display_manager,
                            mouse_drag_state,
                        );
                    }
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_TOGGLE_SHADOW,
            ) {
                if let Some(acting_window) = acting_window_id {
                    toggle_window_shadow(acting_window, window_manager);
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_SPLIT)
            {
                if let Some(acting_window) = acting_window_id {
                    toggle_split_direction_of_the_parent_of_window_leaf(
                        space_manager,
                        acting_window,
                        display_manager,
                        window_manager,
                    );
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_TOGGLE_PARENT,
            ) {
                if let Some(acting_window) = acting_window_id {
                    toggle_managed_window_zoom_parent(window_manager, acting_window, space_manager);
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_TOGGLE_ZOOM_FULLSCREEN,
            ) {
                if let Some(acting_window) = acting_window_id {
                    toggle_managed_window_zoom_fullscreen(
                        window_manager,
                        acting_window,
                        space_manager,
                    );
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_TOGGLE_WINDOWED,
            ) {
                if let Some(acting_window) = acting_window_id {
                    toggle_window_windowed_fullscreen(
                        acting_window,
                        display_manager,
                        window_manager,
                    );
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_TOGGLE_NATIVE,
            ) {
                if let Some(acting_window) = acting_window_id {
                    toggle_window_native_fullscreen(acting_window, window_manager);
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_TOGGLE_EXPOSE,
            ) {
                if let Some(acting_window) = acting_window_id {
                    toggle_application_expose_for_window(acting_window, window_manager);
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(
                value,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_TOGGLE_PICTURE_IN_PICTURE,
            ) {
                if let Some(acting_window) = acting_window_id {
                    toggle_window_picture_in_picture(
                        space_manager,
                        acting_window,
                        display_manager,
                        window_manager,
                    );
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_GROUP)
            {
                if let Some(acting_window) = acting_window_id {
                    let result = toggle_group_of_window(
                        space_manager,
                        acting_window,
                        display_manager,
                        window_manager,
                        mouse_drag_state,
                    );
                    if result == WindowOperationOutcome::InvalidSourceView {
                        daemon_fail!(response, "the acting window is not within a bsp space.\n");
                    } else if result == WindowOperationOutcome::InvalidSourceNode {
                        daemon_fail!(response, "the acting window is not managed.\n");
                    }
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if !toggle_scratchpad_window_with_label(
                window_manager,
                null_terminated_bytes_starting_at(message_cursor.bytes(), value.start),
                process_manager,
            ) {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_SUB_LAYER) {
            if let Some(acting_window) = acting_window_id {
                let value = message_cursor.take_next_token();
                if is_token_equal_to(value, message_cursor.bytes(), ARGUMENT_WINDOW_LAYER_BELOW) {
                    if !set_window_layer_for_it_and_its_child_windows(
                        acting_window,
                        LAYER_BELOW,
                        window_manager,
                    ) {
                        daemon_fail!(
                            response,
                            "could not change sub-layer of window with id '{}' due to an error with the scripting-addition.\n",
                            acting_window.0 as i32
                        );
                    }
                } else if is_token_equal_to(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_WINDOW_LAYER_NORMAL,
                ) {
                    if !set_window_layer_for_it_and_its_child_windows(
                        acting_window,
                        LAYER_NORMAL,
                        window_manager,
                    ) {
                        daemon_fail!(
                            response,
                            "could not change sub-layer of window with id '{}' due to an error with the scripting-addition.\n",
                            acting_window.0 as i32
                        );
                    }
                } else if is_token_equal_to(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_WINDOW_LAYER_ABOVE,
                ) {
                    if !set_window_layer_for_it_and_its_child_windows(
                        acting_window,
                        LAYER_ABOVE,
                        window_manager,
                    ) {
                        daemon_fail!(
                            response,
                            "could not change sub-layer of window with id '{}' due to an error with the scripting-addition.\n",
                            acting_window.0 as i32
                        );
                    }
                } else if is_token_equal_to(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_WINDOW_LAYER_AUTO,
                ) {
                    if !set_window_layer_for_it_and_its_child_windows(
                        acting_window,
                        LAYER_AUTO,
                        window_manager,
                    ) {
                        daemon_fail!(
                            response,
                            "could not change sub-layer of window with id '{}' due to an error with the scripting-addition.\n",
                            acting_window.0 as i32
                        );
                    }
                } else {
                    daemon_fail_with_unknown_value_given_to_command_for_domain(
                        response,
                        message_cursor.bytes(),
                        value,
                        command,
                        domain,
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_OPACITY) {
            if let Some(acting_window) = acting_window_id {
                let value = parse_token_into_typed_value(
                    message_cursor.take_next_token(),
                    message_cursor.bytes(),
                );
                if let TokenValueType::Float(float_value) = value.type_of_value
                    && is_within_range_including_both_bounds(float_value, 0.0f32, 1.0f32)
                {
                    if apply_opacity_to_window_through_scripting_addition(
                        window_manager,
                        acting_window,
                        float_value,
                    ) {
                        if let Some(window) = window_manager.window.find_mut(&acting_window) {
                            window.opacity = float_value;
                        }
                    } else {
                        daemon_fail!(
                            response,
                            "could not change opacity of window with id '{}' due to an error with the scripting-addition.\n",
                            acting_window.0 as i32
                        );
                    }
                } else {
                    daemon_fail_with_unknown_value_given_to_command_for_domain(
                        response,
                        message_cursor.bytes(),
                        value.token,
                        command,
                        domain,
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_RAISE) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_window_selector(
                    response,
                    message_cursor,
                    acting_window_id,
                    true,
                    display_manager,
                    window_manager,
                    space_manager,
                );
                let mut selector_window_id = WindowId(0);

                if selector.token.is_not_empty() {
                    if let Some(resolved_window_id) = selector.resolved_target() {
                        selector_window_id = resolved_window_id;
                    } else {
                        return;
                    }
                }

                if !order_window_relative_to_other_window_through_scripting_addition(
                    acting_window,
                    1,
                    selector_window_id,
                ) {
                    daemon_fail!(
                        response,
                        "could not raise window with id '{}' due to an error with the scripting-addition.\n",
                        acting_window.0 as i32
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_LOWER) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_window_selector(
                    response,
                    message_cursor,
                    acting_window_id,
                    true,
                    display_manager,
                    window_manager,
                    space_manager,
                );
                let mut selector_window_id = WindowId(0);

                if selector.token.is_not_empty() {
                    if let Some(resolved_window_id) = selector.resolved_target() {
                        selector_window_id = resolved_window_id;
                    } else {
                        return;
                    }
                }

                if !order_window_relative_to_other_window_through_scripting_addition(
                    acting_window,
                    -1,
                    selector_window_id,
                ) {
                    daemon_fail!(
                        response,
                        "could not lower window with id '{}' due to an error with the scripting-addition.\n",
                        acting_window.0 as i32
                    );
                }
            }
        } else if is_token_equal_to(command, message_cursor.bytes(), COMMAND_WINDOW_SCRATCHPAD) {
            if let Some(acting_window) = acting_window_id {
                let mut label = None;
                let token = message_cursor.take_next_token();
                if token.is_not_empty()
                    && is_token_equal_to(
                        token,
                        message_cursor.bytes(),
                        ARGUMENT_WINDOW_SCRATCHPAD_RECOVER,
                    )
                {
                    recover_hidden_scratchpad_windows_by_ordering_every_window_in(
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    );
                } else if parse_label_refusing_numbers_and_reserved_words(
                    response,
                    message_cursor.bytes(),
                    token,
                    LabelType::Window,
                    &mut label,
                ) {
                    if let Some(label) = label {
                        if !assign_window_to_scratchpad_making_it_float(
                            window_manager,
                            acting_window,
                            label,
                            process_manager,
                            display_manager,
                            space_manager,
                            mouse_drag_state,
                        ) {
                            daemon_fail!(
                                response,
                                "the given scratchpad is already assigned to a different window!\n"
                            );
                        }
                    } else if !remove_window_from_its_scratchpad(
                        window_manager,
                        acting_window,
                        true,
                        process_manager,
                        display_manager,
                        space_manager,
                        mouse_drag_state,
                    ) {
                        daemon_fail!(
                            response,
                            "the selected window was not assigned to a scratchpad!\n"
                        );
                    }
                }
            }
        } else {
            daemon_fail_with_unknown_command_for_domain(
                response,
                message_cursor.bytes(),
                command,
                domain,
            );
        }

        command = message_cursor.take_next_token();
    }
}
