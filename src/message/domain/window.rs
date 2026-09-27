use crate::daemon_fail;
use crate::display::identity::display_manager_active_display_id;
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_id;
use crate::layout::tree::NODE_MAX_WINDOW_COUNT;
use crate::message::argument_prefixes::{parse_resize_handle, parse_value_type};
use crate::message::common_failures::{
    daemon_fail_with_unknown_command_for_domain,
    daemon_fail_with_unknown_value_given_to_command_for_domain,
};
use crate::message::labels::{LabelType, parse_label};
use crate::message::selectors::{
    parse_display_selector, parse_insert_selector, parse_space_selector, parse_window_selector,
};
use crate::message::token::{
    MessageCursor, Token, TokenType, c_string_at, token_equals, token_to_value,
};
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::scripting_addition::client::scripting_addition_order_window;
use crate::space::focus::space_manager_active_space;
use crate::space::managed_space::space_is_fullscreen;
use crate::space::manager::SpaceManager;
use crate::space::tiling::space_manager_toggle_window_split;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::arithmetic::in_range_ii;
use crate::support::handles::WindowId;
use crate::support::layer::{LAYER_ABOVE, LAYER_AUTO, LAYER_BELOW, LAYER_NORMAL};
use crate::support::response::Response;
use crate::support::strings::MAXLEN;
use crate::window::floating_and_sticky::{
    window_manager_make_window_floating, window_manager_make_window_sticky,
};
use crate::window::focus::{
    window_manager_focus_window_with_raise, window_manager_focused_window,
    window_manager_toggle_window_expose,
};
use crate::window::frame::{
    window_manager_adjust_window_ratio, window_manager_move_window_relative,
    window_manager_resize_window_relative,
};
use crate::window::fullscreen::{
    window_manager_toggle_window_native_fullscreen, window_manager_toggle_window_pip,
    window_manager_toggle_window_windowed_fullscreen, window_manager_toggle_window_zoom_fullscreen,
    window_manager_toggle_window_zoom_parent,
};
use crate::window::grid::window_manager_apply_grid;
use crate::window::layer::window_manager_set_window_layer;
use crate::window::manager::{WindowManager, WindowOpError};
use crate::window::minimize_and_close::{
    window_manager_close_window, window_manager_deminimize_window, window_manager_minimize_window,
};
use crate::window::model::{WindowFlag, window_check_flag};
use crate::window::opacity::window_manager_set_opacity;
use crate::window::scratchpad::{
    window_manager_remove_scratchpad_for_window, window_manager_scratchpad_recover_windows,
    window_manager_set_scratchpad_for_window, window_manager_toggle_scratchpad_window_by_label,
};
use crate::window::send_to_space::window_manager_send_window_to_space;
use crate::window::shadow::window_manager_toggle_window_shadow;
use crate::window::tree_placement::{
    window_manager_set_window_insertion, window_manager_stack_window, window_manager_swap_window,
    window_manager_warp_window,
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
pub(crate) const ARGUMENT_WINDOW_TOGGLE_FULLSC: &str = "zoom-fullscreen";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_WINDOWED: &str = "windowed-fullscreen";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_NATIVE: &str = "native-fullscreen";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_EXPOSE: &str = "expose";
pub(crate) const ARGUMENT_WINDOW_TOGGLE_PIP: &str = "pip";

pub(crate) const ARGUMENT_WINDOW_SCRATCHPAD_RECOVER: &str = "recover";
/* ----------------------------------------------------------------------------- */

pub(crate) fn handle_domain_window(
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
    let mut acting_window_id = window_manager_focused_window(window_manager);
    let selector = parse_window_selector(
        &mut Response::silent(),
        message_cursor,
        acting_window_id,
        true,
        display_manager,
        window_manager,
        space_manager,
    );

    if selector.did_parse() {
        acting_window_id = selector.resolved();
        command = message_cursor.get_token();
    } else {
        command = selector.token;
    }

    while command.is_valid() {
        if acting_window_id.is_none()
            && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_FOCUS)
            && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_CLOSE)
            && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_MINIMIZE)
            && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_DEMINIMIZE)
            && !token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_TOGGLE)
        {
            daemon_fail!(response, "could not locate the window to act on!\n");
            return;
        }

        if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_FOCUS) {
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_window_id) = selector.resolved() {
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
                    window_manager_focus_window_with_raise(
                        &window_process_serial_number,
                        acting_window,
                        window_element_ref,
                    );
                }
            } else {
                daemon_fail!(response, "could not locate the window to act on!\n");
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_CLOSE) {
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_window_id) = selector.resolved() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                if !window_manager_close_window(acting_window, window_manager) {
                    daemon_fail!(
                        response,
                        "could not close window with id '{}'.\n",
                        acting_window.0 as i32
                    );
                }
            } else {
                daemon_fail!(response, "could not locate the window to act on!\n");
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_MINIMIZE) {
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                true,
                display_manager,
                window_manager,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_window_id) = selector.resolved() {
                    acting_window_id = Some(selector_window_id);
                } else {
                    return;
                }
            }

            if let Some(acting_window) = acting_window_id {
                let result = window_manager_minimize_window(acting_window, window_manager);
                if result == WindowOpError::CantMinimize {
                    daemon_fail!(
                        response,
                        "window with id '{}' does not support the minimize operation.\n",
                        acting_window.0 as i32
                    );
                } else if result == WindowOpError::AlreadyMinimized {
                    daemon_fail!(
                        response,
                        "window with id '{}' is already minimized.\n",
                        acting_window.0 as i32
                    );
                } else if result == WindowOpError::MinimizeFailed {
                    daemon_fail!(
                        response,
                        "could not minimize window with id '{}'.\n",
                        acting_window.0 as i32
                    );
                }
            } else {
                daemon_fail!(response, "could not locate the window to act on!\n");
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_DEMINIMIZE) {
            let selector = parse_window_selector(
                response,
                message_cursor,
                acting_window_id,
                false,
                display_manager,
                window_manager,
                space_manager,
            );
            if let Some(selector_window_id) = selector.resolved() {
                let result = window_manager_deminimize_window(selector_window_id, window_manager);
                if result == WindowOpError::NotMinimized {
                    daemon_fail!(
                        response,
                        "window with id '{}' is not minimized.\n",
                        selector_window_id.0 as i32
                    );
                } else if result == WindowOpError::DeminimizeFailed {
                    daemon_fail!(
                        response,
                        "could not deminimize window with id '{}'.\n",
                        selector_window_id.0 as i32
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_DISPLAY) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_display_selector(
                    response,
                    message_cursor,
                    display_manager_active_display_id(),
                    false,
                    display_manager,
                );
                if let Some(selector_display_id) = selector.resolved() {
                    let space_id = display_space_id(selector_display_id);
                    if space_is_fullscreen(space_id) {
                        daemon_fail!(
                            response,
                            "can not move window to a macOS fullscreen space!\n"
                        );
                    } else {
                        window_manager_send_window_to_space(
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_SPACE) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_space_selector(
                    response,
                    message_cursor,
                    space_manager_active_space(window_manager),
                    false,
                    space_manager,
                );
                if let Some(selector_space_id) = selector.resolved() {
                    if space_is_fullscreen(selector_space_id) {
                        daemon_fail!(
                            response,
                            "can not move window to a macOS fullscreen space!\n"
                        );
                    } else {
                        window_manager_send_window_to_space(
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_SWAP) {
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
                if let Some(selector_window_id) = selector.resolved() {
                    let result = window_manager_swap_window(
                        space_manager,
                        window_manager,
                        acting_window,
                        selector_window_id,
                        display_manager,
                    );
                    if result == WindowOpError::InvalidSrcView {
                        daemon_fail!(response, "the acting window is not within a bsp space.\n");
                    } else if result == WindowOpError::InvalidDstView {
                        daemon_fail!(
                            response,
                            "the selected window is not within a bsp space.\n"
                        );
                    } else if result == WindowOpError::InvalidSrcNode {
                        daemon_fail!(response, "the acting window is not managed.\n");
                    } else if result == WindowOpError::InvalidDstNode {
                        daemon_fail!(response, "the selected window is not managed.\n");
                    } else if result == WindowOpError::SameStack {
                        daemon_fail!(
                            response,
                            "cannot swap a window with a window in the same stack.\n"
                        );
                    } else if result == WindowOpError::SameWindow {
                        daemon_fail!(response, "cannot swap a window with itself.\n");
                    }
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_WARP) {
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
                if let Some(selector_window_id) = selector.resolved() {
                    let result = window_manager_warp_window(
                        space_manager,
                        window_manager,
                        acting_window,
                        selector_window_id,
                        process_manager,
                        display_manager,
                        mouse_drag_state,
                    );
                    if result == WindowOpError::InvalidSrcView {
                        daemon_fail!(response, "the acting window is not within a bsp space.\n");
                    } else if result == WindowOpError::InvalidDstView {
                        daemon_fail!(
                            response,
                            "the selected window is not within a bsp space.\n"
                        );
                    } else if result == WindowOpError::InvalidSrcNode {
                        daemon_fail!(response, "the acting window is not managed.\n");
                    } else if result == WindowOpError::InvalidDstNode {
                        daemon_fail!(response, "the selected window is not managed.\n");
                    } else if result == WindowOpError::SameStack {
                        daemon_fail!(
                            response,
                            "cannot warp a window with a window in the same stack.\n"
                        );
                    } else if result == WindowOpError::SameWindow {
                        daemon_fail!(response, "cannot warp a window onto itself.\n");
                    }
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_STACK) {
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
                if let Some(selector_window_id) = selector.resolved() {
                    let result = window_manager_stack_window(
                        space_manager,
                        window_manager,
                        acting_window,
                        selector_window_id,
                        display_manager,
                        mouse_drag_state,
                    );
                    if result == WindowOpError::InvalidSrcNode {
                        daemon_fail!(response, "the acting window is not managed.\n");
                    } else if result == WindowOpError::MaxStack {
                        daemon_fail!(
                            response,
                            "cannot stack window, max capacity of {} reached.\n",
                            NODE_MAX_WINDOW_COUNT as i32
                        );
                    } else if result == WindowOpError::SameWindow {
                        daemon_fail!(response, "cannot stack a window onto itself.\n");
                    }
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_INSERT) {
            if let Some(acting_window) = acting_window_id {
                let selector = parse_insert_selector(response, message_cursor);
                if let Some(direction) = selector.resolved() {
                    let result = window_manager_set_window_insertion(
                        space_manager,
                        acting_window,
                        direction,
                        display_manager,
                        window_manager,
                    );
                    if result == WindowOpError::InvalidSrcView {
                        daemon_fail!(response, "the acting window is not within a bsp space.\n");
                    } else if result == WindowOpError::InvalidSrcNode {
                        daemon_fail!(response, "the acting window is not managed.\n");
                    }
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_GRID) {
            if let Some(acting_window) = acting_window_id {
                let mut rows: libc::c_int = 0;
                let mut columns: libc::c_int = 0;
                let mut x: libc::c_int = 0;
                let mut y: libc::c_int = 0;
                let mut width: libc::c_int = 0;
                let mut height: libc::c_int = 0;
                let value = message_cursor.get_token();
                let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                    .unwrap_or_default();
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
                    let result = window_manager_apply_grid(
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
                    if result == WindowOpError::InvalidSrcView {
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_MOVE) {
            if let Some(acting_window) = acting_window_id {
                let mut x: libc::c_float = 0.0;
                let mut y: libc::c_float = 0.0;
                let mut type_of_change = [0 as libc::c_char; MAXLEN];
                let value = message_cursor.get_token();
                let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                    .unwrap_or_default();
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
                    let result = window_manager_move_window_relative(
                        window_manager,
                        acting_window,
                        parse_value_type(&type_of_change) as i32,
                        x,
                        y,
                    );
                    if result == WindowOpError::InvalidSrcView {
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_RESIZE) {
            if let Some(acting_window) = acting_window_id {
                let mut width: libc::c_float = 0.0;
                let mut height: libc::c_float = 0.0;
                let mut handle = [0 as libc::c_char; MAXLEN];
                let value = message_cursor.get_token();
                let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                    .unwrap_or_default();
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
                    let result = window_manager_resize_window_relative(
                        window_manager,
                        acting_window,
                        parse_resize_handle(&handle) as i32,
                        width,
                        height,
                        true,
                        display_manager,
                        space_manager,
                    );
                    if result == WindowOpError::InvalidSrcNode {
                        daemon_fail!(response, "cannot locate bsp node for the managed window.\n");
                    } else if result == WindowOpError::InvalidDstNode {
                        daemon_fail!(response, "cannot locate a bsp node fence.\n");
                    } else if result == WindowOpError::InvalidOperation {
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_RATIO) {
            if let Some(acting_window) = acting_window_id {
                let mut ratio: libc::c_float = 0.0;
                let mut type_of_change = [0 as libc::c_char; MAXLEN];
                let value = message_cursor.get_token();
                let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                    .unwrap_or_default();
                let converted = unsafe {
                    libc::sscanf(
                        subject.as_ptr(),
                        ARGUMENT_WINDOW_RATIO.as_ptr(),
                        type_of_change.as_mut_ptr(),
                        &mut ratio as *mut libc::c_float,
                    )
                };
                if converted == 2 {
                    let result = window_manager_adjust_window_ratio(
                        window_manager,
                        acting_window,
                        parse_value_type(&type_of_change) as i32,
                        ratio,
                        space_manager,
                    );
                    if result == WindowOpError::InvalidSrcView {
                        daemon_fail!(response, "cannot adjust ratio of a non-managed window.\n");
                    } else if result == WindowOpError::InvalidSrcNode {
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_TOGGLE) {
            let value = message_cursor.get_token();
            if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_FLOAT) {
                if let Some(acting_window) = acting_window_id {
                    let should_float = window_manager
                        .window
                        .find(&acting_window)
                        .map(|window| !window_check_flag(window, WindowFlag::FLOAT));
                    if let Some(should_float) = should_float {
                        window_manager_make_window_floating(
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
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_STICKY) {
                if let Some(acting_window) = acting_window_id {
                    let should_sticky = window_manager
                        .window
                        .find(&acting_window)
                        .map(|window| !window_check_flag(window, WindowFlag::STICKY));
                    if let Some(should_sticky) = should_sticky {
                        window_manager_make_window_sticky(
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
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_SHADOW) {
                if let Some(acting_window) = acting_window_id {
                    window_manager_toggle_window_shadow(acting_window, window_manager);
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_SPLIT) {
                if let Some(acting_window) = acting_window_id {
                    space_manager_toggle_window_split(
                        space_manager,
                        acting_window,
                        display_manager,
                        window_manager,
                    );
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_PARENT) {
                if let Some(acting_window) = acting_window_id {
                    window_manager_toggle_window_zoom_parent(
                        window_manager,
                        acting_window,
                        space_manager,
                    );
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_FULLSC) {
                if let Some(acting_window) = acting_window_id {
                    window_manager_toggle_window_zoom_fullscreen(
                        window_manager,
                        acting_window,
                        space_manager,
                    );
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_WINDOW_TOGGLE_WINDOWED,
            ) {
                if let Some(acting_window) = acting_window_id {
                    window_manager_toggle_window_windowed_fullscreen(
                        acting_window,
                        display_manager,
                        window_manager,
                    );
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_NATIVE) {
                if let Some(acting_window) = acting_window_id {
                    window_manager_toggle_window_native_fullscreen(acting_window, window_manager);
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_EXPOSE) {
                if let Some(acting_window) = acting_window_id {
                    window_manager_toggle_window_expose(acting_window, window_manager);
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_TOGGLE_PIP) {
                if let Some(acting_window) = acting_window_id {
                    window_manager_toggle_window_pip(
                        space_manager,
                        acting_window,
                        display_manager,
                        window_manager,
                    );
                } else {
                    daemon_fail!(response, "could not locate the window to act on!\n");
                }
            } else if !window_manager_toggle_scratchpad_window_by_label(
                window_manager,
                c_string_at(message_cursor.bytes(), value.start),
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_SUB_LAYER) {
            if let Some(acting_window) = acting_window_id {
                let value = message_cursor.get_token();
                if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_LAYER_BELOW) {
                    if !window_manager_set_window_layer(acting_window, LAYER_BELOW, window_manager)
                    {
                        daemon_fail!(
                            response,
                            "could not change sub-layer of window with id '{}' due to an error with the scripting-addition.\n",
                            acting_window.0 as i32
                        );
                    }
                } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_LAYER_NORMAL)
                {
                    if !window_manager_set_window_layer(acting_window, LAYER_NORMAL, window_manager)
                    {
                        daemon_fail!(
                            response,
                            "could not change sub-layer of window with id '{}' due to an error with the scripting-addition.\n",
                            acting_window.0 as i32
                        );
                    }
                } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_LAYER_ABOVE) {
                    if !window_manager_set_window_layer(acting_window, LAYER_ABOVE, window_manager)
                    {
                        daemon_fail!(
                            response,
                            "could not change sub-layer of window with id '{}' due to an error with the scripting-addition.\n",
                            acting_window.0 as i32
                        );
                    }
                } else if token_equals(value, message_cursor.bytes(), ARGUMENT_WINDOW_LAYER_AUTO) {
                    if !window_manager_set_window_layer(acting_window, LAYER_AUTO, window_manager) {
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_OPACITY) {
            if let Some(acting_window) = acting_window_id {
                let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
                if let TokenType::Float(float_value) = value.type_of_value
                    && in_range_ii(float_value, 0.0f32, 1.0f32)
                {
                    if window_manager_set_opacity(window_manager, acting_window, float_value) {
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_RAISE) {
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

                if selector.token.is_valid() {
                    if let Some(resolved_window_id) = selector.resolved() {
                        selector_window_id = resolved_window_id;
                    } else {
                        return;
                    }
                }

                if !scripting_addition_order_window(acting_window, 1, selector_window_id) {
                    daemon_fail!(
                        response,
                        "could not raise window with id '{}' due to an error with the scripting-addition.\n",
                        acting_window.0 as i32
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_LOWER) {
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

                if selector.token.is_valid() {
                    if let Some(resolved_window_id) = selector.resolved() {
                        selector_window_id = resolved_window_id;
                    } else {
                        return;
                    }
                }

                if !scripting_addition_order_window(acting_window, -1, selector_window_id) {
                    daemon_fail!(
                        response,
                        "could not lower window with id '{}' due to an error with the scripting-addition.\n",
                        acting_window.0 as i32
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_WINDOW_SCRATCHPAD) {
            if let Some(acting_window) = acting_window_id {
                let mut label = None;
                let token = message_cursor.get_token();
                if token.is_valid()
                    && token_equals(
                        token,
                        message_cursor.bytes(),
                        ARGUMENT_WINDOW_SCRATCHPAD_RECOVER,
                    )
                {
                    window_manager_scratchpad_recover_windows(
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    );
                } else if parse_label(
                    response,
                    message_cursor.bytes(),
                    token,
                    LabelType::Window,
                    &mut label,
                ) {
                    if let Some(label) = label {
                        if !window_manager_set_scratchpad_for_window(
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
                    } else if !window_manager_remove_scratchpad_for_window(
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

        command = message_cursor.get_token();
    }
}
