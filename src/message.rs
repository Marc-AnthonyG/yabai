use std::ffi::CString;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;

use crate::daemon_fail;
use crate::display_manager::{
    DISPLAY_ARRANGEMENT_ORDER_STR, DisplayArrangementOrder, DisplayManager, EXTERNAL_BAR_MODE_STR,
    ExternalBarMode, display_manager_active_display_id, display_manager_query_displays, display_manager_arrangement_display_id,
    display_manager_cursor_display_id, display_manager_focus_display, display_manager_focus_space,
    display_manager_remove_label_for_display, display_manager_set_label_for_display,
    display_manager_find_closest_display_in_direction, display_manager_first_display_id,
    display_manager_get_display_for_label, display_manager_last_display_id,
    display_manager_next_display_id, display_manager_prev_display_id,
};
use crate::display::{
    DISPLAY_PROPERTY_STR, DISPLAY_PROPERTY_VAL, display_serialize, display_space_id,
};
use crate::event_loop::{Event, event_loop_post};
use crate::event_signal::{
    SIGNAL_TYPE_COUNT, Signal, SignalProp, SignalType, event_signal_add, event_signal_list,
    event_signal_remove, event_signal_remove_by_index, signal_type_from_string,
};
use crate::ffi::core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use crate::globals::{MOUSE_TAP_STATE, VERBOSE};
use crate::handles::{DisplayId, SpaceId, WindowId};
use crate::misc::helpers::{
    ANIMATION_EASING_TYPE_STR, AnimationEasingType, BOOL_STR, EASING_TYPE_COUNT,
    rgba_color_from_hex,
};
use crate::misc::macros::{
    DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST, LAYER_ABOVE, LAYER_AUTO, LAYER_BELOW, LAYER_NORMAL,
    MAXLEN, ResizeHandle, STACK, TYPE_ABS, TYPE_REL, in_range_ei, in_range_ii,
};
use crate::misc::regex::PosixRegex;
use crate::misc::response::{FailurePiece, Response};
use crate::mission_control::MissionControlMode;
use crate::mouse_handler::{MOUSE_MOD_STR, MOUSE_MODE_STR, MouseMod, MouseMode};
use crate::process_manager::ProcessManager;
use crate::rule::{
    RULE_PROP_OFF, RULE_PROP_ON, Rule, RuleEffectsFlag, RuleFlag, rule_add, rule_apply,
    rule_reapply_all, rule_reapply_by_index, rule_reapply_by_label, rule_remove_by_index,
    rule_remove_by_label,
};
use crate::sa::{scripting_addition_is_sip_friendly, scripting_addition_order_window};
use crate::space::{space_display_id, space_is_fullscreen, space_is_user};
use crate::space_manager::{
    SpaceManager, SpaceOpError, space_manager_active_space, space_manager_add_space,
    space_manager_balance_space, space_manager_destroy_space, space_manager_equalize_space,
    space_manager_focus_space, space_manager_mirror_space, space_manager_move_space_to_display,
    space_manager_move_space_to_space, space_manager_remove_label_for_space,
    space_manager_rotate_space, space_manager_set_gap_for_space, space_manager_set_label_for_space,
    space_manager_set_layout_for_space, space_manager_set_padding_for_space,
    space_manager_swap_space_with_space, space_manager_switch_space,
    space_manager_toggle_gap_for_space, space_manager_toggle_mission_control,
    space_manager_toggle_padding_for_space, space_manager_toggle_show_desktop,
    space_manager_toggle_window_split,
    space_manager_cursor_space, space_manager_find_view, space_manager_first_space,
    space_manager_get_space_for_label, space_manager_last_space, space_manager_mark_spaces_invalid,
    space_manager_mission_control_space, space_manager_next_space, space_manager_prev_space,
    space_manager_query_space, space_manager_query_spaces_for_display,
    space_manager_query_spaces_for_displays, space_manager_query_spaces_for_window,
    space_manager_set_auto_balance_for_all_spaces, space_manager_set_bottom_padding_for_all_spaces,
    space_manager_set_layout_for_all_spaces, space_manager_set_left_padding_for_all_spaces,
    space_manager_set_right_padding_for_all_spaces, space_manager_set_split_type_for_all_spaces,
    space_manager_set_top_padding_for_all_spaces, space_manager_set_window_gap_for_all_spaces,
};
use crate::state::MouseDragState;
use crate::view::{
    AUTO_BALANCE_STR, NODE_MAX_WINDOW_COUNT, SPACE_PROPERTY_STR, SPACE_PROPERTY_VAL, VIEW_TYPE_STR, ViewFlag, ViewType, WINDOW_INSERTION_POINT_STR,
    WINDOW_NODE_CHILD_STR, WINDOW_NODE_SPLIT_STR, WindowInsertionPoint, WindowNodeChild,
    WindowNodeSplit, view_clear, view_flush, view_update,
};
use crate::window::{
    WINDOW_PROPERTY_STR, WINDOW_PROPERTY_VAL, WindowFlag, window_check_flag, window_display_id,
    window_serialize,
};
use crate::window_manager::{
    FFM_MODE_STR, FfmMode, PURIFY_MODE_STR, PurifyMode, WINDOW_ORIGIN_MODE_STR, WindowManager,
    WindowOpError, WindowOriginMode, window_manager_adjust_window_ratio, window_manager_apply_grid,
    window_manager_close_window, window_manager_deminimize_window,
    window_manager_focus_window_with_raise, window_manager_focused_window,
    window_manager_make_window_floating, window_manager_make_window_sticky,
    window_manager_minimize_window, window_manager_move_window_relative,
    window_manager_remove_scratchpad_for_window, window_manager_resize_window_relative,
    window_manager_scratchpad_recover_windows, window_manager_send_window_to_space,
    window_manager_set_opacity, window_manager_set_scratchpad_for_window,
    window_manager_set_window_insertion, window_manager_set_window_layer,
    window_manager_stack_window, window_manager_swap_window,
    window_manager_toggle_scratchpad_window_by_label, window_manager_toggle_window_expose,
    window_manager_toggle_window_native_fullscreen, window_manager_toggle_window_pip,
    window_manager_toggle_window_shadow, window_manager_toggle_window_windowed_fullscreen,
    window_manager_toggle_window_zoom_fullscreen, window_manager_toggle_window_zoom_parent,
    window_manager_query_window_rules, window_manager_query_windows_for_display,
    window_manager_query_windows_for_displays, window_manager_query_windows_for_spaces,
    window_manager_warp_window, window_manager_set_focus_follows_mouse, window_manager_set_purify_mode,
    window_manager_set_active_window_opacity, window_manager_set_menubar_opacity,
    window_manager_set_normal_window_opacity, window_manager_set_window_opacity_enabled,
    window_manager_validate_and_check_for_windows_on_space,
    window_manager_find_closest_managed_window_in_direction,
    window_manager_find_first_cousin_for_managed_window, window_manager_find_first_managed_window,
    window_manager_find_first_nephew_for_managed_window, window_manager_find_first_window_in_stack,
    window_manager_find_largest_managed_window, window_manager_find_last_managed_window,
    window_manager_find_last_window_in_stack, window_manager_find_next_managed_window,
    window_manager_find_next_window_in_stack, window_manager_find_prev_managed_window,
    window_manager_find_prev_window_in_stack, window_manager_find_recent_managed_window,
    window_manager_find_recent_window_in_stack, window_manager_find_second_cousin_for_managed_window,
    window_manager_find_second_nephew_for_managed_window,
    window_manager_find_sibling_for_managed_window, window_manager_find_smallest_managed_window,
    window_manager_find_uncle_for_managed_window, window_manager_find_window,
    window_manager_find_window_below_cursor, window_manager_find_window_in_stack,
};

pub(crate) struct MessageLoop {
    pub(crate) listener: UnixListener,
    pub(crate) thread: JoinHandle<()>,
}

#[derive(Clone, Copy)]
pub(crate) struct Token {
    pub(crate) start: usize,
    pub(crate) length: usize,
}

impl Token {
    pub(crate) fn bytes(self, message_bytes: &[u8]) -> &[u8] {
        &message_bytes[self.start..self.start + self.length]
    }

    pub(crate) fn as_c_string_pointer(self, message_bytes: &[u8]) -> *const libc::c_char {
        message_bytes[self.start..].as_ptr() as *const libc::c_char
    }
}

pub(crate) struct MessageCursor<'message> {
    pub(crate) bytes: &'message mut [u8],
    pub(crate) at: usize,
}

impl<'message> MessageCursor<'message> {
    pub(crate) fn new(bytes: &'message mut [u8]) -> MessageCursor<'message> {
        MessageCursor { bytes, at: 0 }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &*self.bytes
    }

    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        &mut *self.bytes
    }

    pub(crate) fn cursor_at(&mut self, at: usize) -> MessageCursor<'_> {
        MessageCursor {
            bytes: &mut *self.bytes,
            at,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TokenType {
    Invalid,
    Unknown,
    Int(i32),
    Float(f32),
    U32(u32),
    String,
}

#[derive(Clone, Copy)]
pub(crate) struct TokenValue {
    pub(crate) token: Token,
    pub(crate) type_of_value: TokenType,
}

pub(crate) struct KeyValuePair {
    pub(crate) key: usize,
    pub(crate) value: usize,
    pub(crate) exclusion: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum LabelType {
    Display,
    Space,
    Window,
}

pub(crate) struct Properties {
    pub(crate) token: Token,
    pub(crate) did_parse: bool,
    pub(crate) did_error: bool,
    pub(crate) flags: u64,
}

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

pub(crate) const DOMAIN_CONFIG: &str = "config";
pub(crate) const DOMAIN_DISPLAY: &str = "display";
pub(crate) const DOMAIN_SPACE: &str = "space";
pub(crate) const DOMAIN_WINDOW: &str = "window";
pub(crate) const DOMAIN_QUERY: &str = "query";
pub(crate) const DOMAIN_RULE: &str = "rule";
pub(crate) const DOMAIN_SIGNAL: &str = "signal";

/* --------------------------------DOMAIN CONFIG-------------------------------- */
pub(crate) const COMMAND_CONFIG_DEBUG_OUTPUT: &str = "debug_output";
pub(crate) const COMMAND_CONFIG_MFF: &str = "mouse_follows_focus";
pub(crate) const COMMAND_CONFIG_FFM: &str = "focus_follows_mouse";
pub(crate) const COMMAND_CONFIG_DISPLAY_ORDER: &str = "display_arrangement_order";
pub(crate) const COMMAND_CONFIG_WINDOW_ORIGIN: &str = "window_origin_display";
pub(crate) const COMMAND_CONFIG_WINDOW_PLACEMENT: &str = "window_placement";
pub(crate) const COMMAND_CONFIG_WINDOW_INSERT_POINT: &str = "window_insertion_point";
pub(crate) const COMMAND_CONFIG_WINDOW_ZOOM_PERSIST: &str = "window_zoom_persist";
pub(crate) const COMMAND_CONFIG_OPACITY: &str = "window_opacity";
pub(crate) const COMMAND_CONFIG_OPACITY_DURATION: &str = "window_opacity_duration";
pub(crate) const COMMAND_CONFIG_ANIMATION_DURATION: &str = "window_animation_duration";
pub(crate) const COMMAND_CONFIG_ANIMATION_EASING: &str = "window_animation_easing";
pub(crate) const COMMAND_CONFIG_SHADOW: &str = "window_shadow";
pub(crate) const COMMAND_CONFIG_MENUBAR_OPACITY: &str = "menubar_opacity";
pub(crate) const COMMAND_CONFIG_ACTIVE_WINDOW_OPACITY: &str = "active_window_opacity";
pub(crate) const COMMAND_CONFIG_NORMAL_WINDOW_OPACITY: &str = "normal_window_opacity";
pub(crate) const COMMAND_CONFIG_INSERT_FEEDBACK_COLOR: &str = "insert_feedback_color";
pub(crate) const COMMAND_CONFIG_TOP_PADDING: &str = "top_padding";
pub(crate) const COMMAND_CONFIG_BOTTOM_PADDING: &str = "bottom_padding";
pub(crate) const COMMAND_CONFIG_LEFT_PADDING: &str = "left_padding";
pub(crate) const COMMAND_CONFIG_RIGHT_PADDING: &str = "right_padding";
pub(crate) const COMMAND_CONFIG_LAYOUT: &str = "layout";
pub(crate) const COMMAND_CONFIG_WINDOW_GAP: &str = "window_gap";
pub(crate) const COMMAND_CONFIG_SPLIT_RATIO: &str = "split_ratio";
pub(crate) const COMMAND_CONFIG_SPLIT_TYPE: &str = "split_type";
pub(crate) const COMMAND_CONFIG_AUTO_BALANCE: &str = "auto_balance";
pub(crate) const COMMAND_CONFIG_MOUSE_MOD: &str = "mouse_modifier";
pub(crate) const COMMAND_CONFIG_MOUSE_ACTION1: &str = "mouse_action1";
pub(crate) const COMMAND_CONFIG_MOUSE_ACTION2: &str = "mouse_action2";
pub(crate) const COMMAND_CONFIG_MOUSE_DROP_ACTION: &str = "mouse_drop_action";
pub(crate) const COMMAND_CONFIG_EXTERNAL_BAR: &str = "external_bar";
pub(crate) const COMMAND_CONFIG_SKIP_SPACE_ANIMATION: &str = "skip_window_focus_animation";

pub(crate) const SELECTOR_CONFIG_SPACE: &str = "--space";

pub(crate) const ARGUMENT_CONFIG_FFM_AUTOFOCUS: &str = "autofocus";
pub(crate) const ARGUMENT_CONFIG_FFM_AUTORAISE: &str = "autoraise";
pub(crate) const ARGUMENT_CONFIG_DISPLAY_ORDER_DEFAULT: &str = "default";
pub(crate) const ARGUMENT_CONFIG_DISPLAY_ORDER_X: &str = "horizontal";
pub(crate) const ARGUMENT_CONFIG_DISPLAY_ORDER_Y: &str = "vertical";
pub(crate) const ARGUMENT_CONFIG_WINDOW_ORIGIN_DEFAULT: &str = "default";
pub(crate) const ARGUMENT_CONFIG_WINDOW_ORIGIN_FOCUSED: &str = "focused";
pub(crate) const ARGUMENT_CONFIG_WINDOW_ORIGIN_CURSOR: &str = "cursor";
pub(crate) const ARGUMENT_CONFIG_WINDOW_PLACEMENT_FST: &str = "first_child";
pub(crate) const ARGUMENT_CONFIG_WINDOW_PLACEMENT_SND: &str = "second_child";
pub(crate) const ARGUMENT_CONFIG_WINDOW_INSERT_FOCUSED: &str = "focused";
pub(crate) const ARGUMENT_CONFIG_WINDOW_INSERT_FIRST: &str = "first";
pub(crate) const ARGUMENT_CONFIG_WINDOW_INSERT_LAST: &str = "last";
pub(crate) const ARGUMENT_CONFIG_SHADOW_FLT: &str = "float";
pub(crate) const ARGUMENT_CONFIG_LAYOUT_BSP: &str = "bsp";
pub(crate) const ARGUMENT_CONFIG_LAYOUT_STACK: &str = "stack";
pub(crate) const ARGUMENT_CONFIG_LAYOUT_FLOAT: &str = "float";
pub(crate) const ARGUMENT_CONFIG_SPLIT_TYPE_Y: &str = "vertical";
pub(crate) const ARGUMENT_CONFIG_SPLIT_TYPE_X: &str = "horizontal";
pub(crate) const ARGUMENT_CONFIG_SPLIT_TYPE_AUTO: &str = "auto";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_ALT: &str = "alt";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_SHIFT: &str = "shift";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_CMD: &str = "cmd";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_CTRL: &str = "ctrl";
pub(crate) const ARGUMENT_CONFIG_MOUSE_MOD_FN: &str = "fn";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_MOVE: &str = "move";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_RESIZE: &str = "resize";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_SWAP: &str = "swap";
pub(crate) const ARGUMENT_CONFIG_MOUSE_ACTION_STACK: &str = "stack";
pub(crate) const ARGUMENT_CONFIG_EXTERNAL_BAR_MAIN: &str = "main";
pub(crate) const ARGUMENT_CONFIG_EXTERNAL_BAR_ALL: &str = "all";
pub(crate) const ARGUMENT_CONFIG_EXTERNAL_BAR: &std::ffi::CStr = c"%5[^:]:%d:%d";
/* ----------------------------------------------------------------------------- */

/* --------------------------------DOMAIN DISPLAY------------------------------- */
pub(crate) const COMMAND_DISPLAY_FOCUS: &str = "--focus";
pub(crate) const COMMAND_DISPLAY_SPACE: &str = "--space";
pub(crate) const COMMAND_DISPLAY_LABEL: &str = "--label";
/* ----------------------------------------------------------------------------- */

/* --------------------------------DOMAIN SPACE--------------------------------- */
pub(crate) const COMMAND_SPACE_FOCUS: &str = "--focus";
pub(crate) const COMMAND_SPACE_SWITCH: &str = "--switch";
pub(crate) const COMMAND_SPACE_CREATE: &str = "--create";
pub(crate) const COMMAND_SPACE_DESTROY: &str = "--destroy";
pub(crate) const COMMAND_SPACE_MOVE: &str = "--move";
pub(crate) const COMMAND_SPACE_SWAP: &str = "--swap";
pub(crate) const COMMAND_SPACE_DISPLAY: &str = "--display";
pub(crate) const COMMAND_SPACE_EQUALIZE: &str = "--equalize";
pub(crate) const COMMAND_SPACE_BALANCE: &str = "--balance";
pub(crate) const COMMAND_SPACE_MIRROR: &str = "--mirror";
pub(crate) const COMMAND_SPACE_ROTATE: &str = "--rotate";
pub(crate) const COMMAND_SPACE_PADDING: &str = "--padding";
pub(crate) const COMMAND_SPACE_GAP: &str = "--gap";
pub(crate) const COMMAND_SPACE_TOGGLE: &str = "--toggle";
pub(crate) const COMMAND_SPACE_LAYOUT: &str = "--layout";
pub(crate) const COMMAND_SPACE_LABEL: &str = "--label";

pub(crate) const ARGUMENT_SPACE_ROTATE_90: &str = "90";
pub(crate) const ARGUMENT_SPACE_ROTATE_180: &str = "180";
pub(crate) const ARGUMENT_SPACE_ROTATE_270: &str = "270";
pub(crate) const ARGUMENT_SPACE_PADDING: &std::ffi::CStr = c"%255[^:]:%d:%d:%d:%d";
pub(crate) const ARGUMENT_SPACE_GAP: &std::ffi::CStr = c"%255[^:]:%d";
pub(crate) const ARGUMENT_SPACE_TGL_PADDING: &str = "padding";
pub(crate) const ARGUMENT_SPACE_TGL_GAP: &str = "gap";
pub(crate) const ARGUMENT_SPACE_TGL_MC: &str = "mission-control";
pub(crate) const ARGUMENT_SPACE_TGL_SD: &str = "show-desktop";
pub(crate) const ARGUMENT_SPACE_LAYOUT_BSP: &str = "bsp";
pub(crate) const ARGUMENT_SPACE_LAYOUT_STACK: &str = "stack";
pub(crate) const ARGUMENT_SPACE_LAYOUT_FLT: &str = "float";
/* ----------------------------------------------------------------------------- */

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

pub(crate) const ARGUMENT_WINDOW_SEL_LARGEST: &str = "largest";
pub(crate) const ARGUMENT_WINDOW_SEL_SMALLEST: &str = "smallest";
pub(crate) const ARGUMENT_WINDOW_SEL_SIBLING: &str = "sibling";
pub(crate) const ARGUMENT_WINDOW_SEL_FNEPHEW: &str = "first_nephew";
pub(crate) const ARGUMENT_WINDOW_SEL_SNEPHEW: &str = "second_nephew";
pub(crate) const ARGUMENT_WINDOW_SEL_UNCLE: &str = "uncle";
pub(crate) const ARGUMENT_WINDOW_SEL_FCOUSIN: &str = "first_cousin";
pub(crate) const ARGUMENT_WINDOW_SEL_SCOUSIN: &str = "second_cousin";
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

/* --------------------------------DOMAIN QUERY--------------------------------- */
pub(crate) const COMMAND_QUERY_DISPLAYS: &str = "--displays";
pub(crate) const COMMAND_QUERY_SPACES: &str = "--spaces";
pub(crate) const COMMAND_QUERY_WINDOWS: &str = "--windows";

pub(crate) const ARGUMENT_QUERY_DISPLAY: &str = "--display";
pub(crate) const ARGUMENT_QUERY_SPACE: &str = "--space";
pub(crate) const ARGUMENT_QUERY_WINDOW: &str = "--window";
/* ----------------------------------------------------------------------------- */

/* --------------------------------DOMAIN RULE---------------------------------- */
pub(crate) const COMMAND_RULE_ADD: &str = "--add";
pub(crate) const COMMAND_RULE_REM: &str = "--remove";
pub(crate) const COMMAND_RULE_APPLY: &str = "--apply";
pub(crate) const COMMAND_RULE_LS: &str = "--list";

pub(crate) const ARGUMENT_RULE_ONE_SHOT: &str = "--one-shot";
pub(crate) const ARGUMENT_RULE_KEY_APP: &str = "app";
pub(crate) const ARGUMENT_RULE_KEY_TITLE: &str = "title";
pub(crate) const ARGUMENT_RULE_KEY_ROLE: &str = "role";
pub(crate) const ARGUMENT_RULE_KEY_SUBROLE: &str = "subrole";
pub(crate) const ARGUMENT_RULE_KEY_DISPLAY: &str = "display";
pub(crate) const ARGUMENT_RULE_KEY_SPACE: &str = "space";
pub(crate) const ARGUMENT_RULE_KEY_OPACITY: &str = "opacity";
pub(crate) const ARGUMENT_RULE_KEY_MANAGE: &str = "manage";
pub(crate) const ARGUMENT_RULE_KEY_STICKY: &str = "sticky";
pub(crate) const ARGUMENT_RULE_KEY_MFF: &str = "mouse_follows_focus";
pub(crate) const ARGUMENT_RULE_KEY_SUB_LAYER: &str = "sub-layer";
pub(crate) const ARGUMENT_RULE_KEY_FULLSCR: &str = "native-fullscreen";
pub(crate) const ARGUMENT_RULE_KEY_GRID: &str = "grid";
pub(crate) const ARGUMENT_RULE_KEY_LABEL: &str = "label";
pub(crate) const ARGUMENT_RULE_KEY_SCRATCHPAD: &str = "scratchpad";

pub(crate) const ARGUMENT_RULE_VALUE_SPACE: u8 = b'^';
pub(crate) const ARGUMENT_RULE_VALUE_GRID: &std::ffi::CStr = c"%d:%d:%d:%d:%d:%d";
/* ----------------------------------------------------------------------------- */

/* --------------------------------DOMAIN SIGNAL-------------------------------- */
pub(crate) const COMMAND_SIGNAL_ADD: &str = "--add";
pub(crate) const COMMAND_SIGNAL_REM: &str = "--remove";
pub(crate) const COMMAND_SIGNAL_LS: &str = "--list";

pub(crate) const ARGUMENT_SIGNAL_KEY_APP: &str = "app";
pub(crate) const ARGUMENT_SIGNAL_KEY_TITLE: &str = "title";
pub(crate) const ARGUMENT_SIGNAL_KEY_ACTIVE: &str = "active";
pub(crate) const ARGUMENT_SIGNAL_KEY_EVENT: &str = "event";
pub(crate) const ARGUMENT_SIGNAL_KEY_ACTION: &str = "action";
pub(crate) const ARGUMENT_SIGNAL_KEY_LABEL: &str = "label";

pub(crate) const ARGUMENT_SIGNAL_VALUE_YES: &str = "yes";
pub(crate) const ARGUMENT_SIGNAL_VALUE_NO: &str = "no";
/* ----------------------------------------------------------------------------- */

/* --------------------------------COMMON ARGUMENTS----------------------------- */
pub(crate) const ARGUMENT_COMMON_VAL_ON: &str = "on";
pub(crate) const ARGUMENT_COMMON_VAL_OFF: &str = "off";
pub(crate) const ARGUMENT_COMMON_SEL_PREV: &str = "prev";
pub(crate) const ARGUMENT_COMMON_SEL_NEXT: &str = "next";
pub(crate) const ARGUMENT_COMMON_SEL_FIRST: &str = "first";
pub(crate) const ARGUMENT_COMMON_SEL_LAST: &str = "last";
pub(crate) const ARGUMENT_COMMON_SEL_RECENT: &str = "recent";
pub(crate) const ARGUMENT_COMMON_SEL_NORTH: &str = "north";
pub(crate) const ARGUMENT_COMMON_SEL_EAST: &str = "east";
pub(crate) const ARGUMENT_COMMON_SEL_SOUTH: &str = "south";
pub(crate) const ARGUMENT_COMMON_SEL_WEST: &str = "west";
pub(crate) const ARGUMENT_COMMON_SEL_MOUSE: &str = "mouse";
pub(crate) const ARGUMENT_COMMON_SEL_STACK: &str = "stack";
pub(crate) const ARGUMENT_COMMON_SEL_STACK_PREFIX: &str = "stack.";
pub(crate) const ARGUMENT_COMMON_VAL_AXIS_X: &str = "x-axis";
pub(crate) const ARGUMENT_COMMON_VAL_AXIS_Y: &str = "y-axis";
/* ----------------------------------------------------------------------------- */

pub(crate) const SCAN_ONE_FLOAT: &std::ffi::CStr = c"%f";

pub(crate) static MESSAGE_LOOP: OnceLock<MessageLoop> = OnceLock::new();

pub(crate) const RESERVED_DISPLAY_IDENTIFIERS: [&str; 10] = [
    ARGUMENT_COMMON_SEL_NORTH,
    ARGUMENT_COMMON_SEL_EAST,
    ARGUMENT_COMMON_SEL_SOUTH,
    ARGUMENT_COMMON_SEL_WEST,
    ARGUMENT_COMMON_SEL_PREV,
    ARGUMENT_COMMON_SEL_NEXT,
    ARGUMENT_COMMON_SEL_FIRST,
    ARGUMENT_COMMON_SEL_LAST,
    ARGUMENT_COMMON_SEL_RECENT,
    ARGUMENT_COMMON_SEL_MOUSE,
];

pub(crate) const RESERVED_SPACE_IDENTIFIERS: [&str; 6] = [
    ARGUMENT_COMMON_SEL_PREV,
    ARGUMENT_COMMON_SEL_NEXT,
    ARGUMENT_COMMON_SEL_FIRST,
    ARGUMENT_COMMON_SEL_LAST,
    ARGUMENT_COMMON_SEL_RECENT,
    ARGUMENT_COMMON_SEL_MOUSE,
];

pub(crate) const RESERVED_WINDOW_IDENTIFIERS: [&str; 11] = [
    ARGUMENT_WINDOW_TOGGLE_FLOAT,
    ARGUMENT_WINDOW_TOGGLE_STICKY,
    ARGUMENT_WINDOW_TOGGLE_SHADOW,
    ARGUMENT_WINDOW_TOGGLE_SPLIT,
    ARGUMENT_WINDOW_TOGGLE_PARENT,
    ARGUMENT_WINDOW_TOGGLE_FULLSC,
    ARGUMENT_WINDOW_TOGGLE_WINDOWED,
    ARGUMENT_WINDOW_TOGGLE_NATIVE,
    ARGUMENT_WINDOW_TOGGLE_EXPOSE,
    ARGUMENT_WINDOW_TOGGLE_PIP,
    ARGUMENT_WINDOW_SCRATCHPAD_RECOVER,
];

impl MessageCursor<'_> {
    pub(crate) fn get_token(&mut self) -> Token {
        let start = self.at;
        while self.at < self.bytes.len() && self.bytes[self.at] != 0 {
            self.at += 1;
        }
        let length = self.at - start;

        let stopped_on_a_null = self.at < self.bytes.len() && self.bytes[self.at] == 0;
        let byte_after_the_null_is_not_null =
            self.at + 1 < self.bytes.len() && self.bytes[self.at + 1] != 0;

        if stopped_on_a_null && byte_after_the_null_is_not_null {
            self.at += 1;
        } else {
            // NOTE(asmvik): don't go past the null-terminator
        }

        Token { start, length }
    }
}

pub(crate) fn token_prefix(token: Token, message_bytes: &[u8], candidate: &str) -> bool {
    let token_bytes = token.bytes(message_bytes);
    let candidate_bytes = candidate.as_bytes();

    for index in 0..token_bytes.len() {
        if index == candidate_bytes.len() {
            return true;
        }
        if token_bytes[index] != candidate_bytes[index] {
            return false;
        }
    }

    token_bytes.len() == candidate_bytes.len()
}

pub(crate) fn token_equals(token: Token, message_bytes: &[u8], candidate: &str) -> bool {
    token.bytes(message_bytes) == candidate.as_bytes()
}

impl Token {
    pub(crate) fn is_valid(self) -> bool {
        self.length > 0
    }
}

pub(crate) fn token_is_positive_integer(token: Token, message_bytes: &[u8]) -> Option<i32> {
    let mut value: i32 = 0;

    for character in token.bytes(message_bytes) {
        if !(*character >= b'0' && *character <= b'9') {
            return None;
        }
        value = value
            .wrapping_mul(10)
            .wrapping_add((*character - b'0') as i32);
    }

    Some(value)
}

pub(crate) fn token_is_hexadecimal(token: Token, message_bytes: &[u8]) -> Option<u32> {
    if token.length <= 2 {
        return None;
    }

    let token_bytes = token.bytes(message_bytes);
    if !(token_bytes[0] == b'0' && (token_bytes[1] == b'x' || token_bytes[1] == b'X')) {
        return None;
    }

    let mut value: u32 = 0;
    for character in &token_bytes[2..] {
        let digit = match *character {
            b'0'..=b'9' => *character - b'0',
            b'a'..=b'f' => *character - b'a' + 0xA,
            b'A'..=b'F' => *character - b'A' + 0xA,
            _ => return None,
        };
        value = value.wrapping_mul(16).wrapping_add(digit as u32);
    }

    Some(value)
}

pub(crate) fn token_is_float(token: Token, message_bytes: &[u8]) -> Option<f32> {
    let mut end: *mut libc::c_char = std::ptr::null_mut();
    let value = unsafe { libc::strtof(token.as_c_string_pointer(message_bytes), &mut end) };

    if end.is_null() || unsafe { *end } != 0 {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn token_to_value(token: Token, message_bytes: &[u8]) -> TokenValue {
    let type_of_value = if !token.is_valid() {
        TokenType::Invalid
    } else if let Some(value) = token_is_positive_integer(token, message_bytes) {
        TokenType::Int(value)
    } else if let Some(value) = token_is_hexadecimal(token, message_bytes) {
        TokenType::U32(value)
    } else if let Some(value) = token_is_float(token, message_bytes) {
        TokenType::Float(value)
    } else {
        TokenType::String
    };

    TokenValue {
        token,
        type_of_value,
    }
}

pub(crate) fn c_string_at(message_bytes: &[u8], start: usize) -> &[u8] {
    let end = message_bytes[start..]
        .iter()
        .position(|byte| *byte == 0)
        .map_or(message_bytes.len(), |offset| start + offset);
    &message_bytes[start..end]
}

pub(crate) fn parse_key_value_pair(
    message_bytes: &mut [u8],
    token_start: usize,
) -> Option<KeyValuePair> {
    let mut at = token_start;

    while at < message_bytes.len() && message_bytes[at] != 0 {
        let first_character = message_bytes[at];
        let second_character = if at + 1 < message_bytes.len() {
            message_bytes[at + 1]
        } else {
            0
        };

        if first_character == b'!' && second_character == b'=' {
            break;
        } else if first_character == b'=' {
            break;
        }

        at += 1;
    }

    let first_character = if at < message_bytes.len() {
        message_bytes[at]
    } else {
        0
    };
    let second_character = if at + 1 < message_bytes.len() {
        message_bytes[at + 1]
    } else {
        0
    };

    let index = if first_character == b'!' && second_character == b'=' {
        2
    } else {
        1
    };
    let check = if index == 2 { b'!' } else { b'=' };

    if first_character != check {
        return None;
    }

    let value = at + index;
    if value < message_bytes.len() && message_bytes[value] != 0 {
        message_bytes[at] = 0;
        Some(KeyValuePair {
            key: token_start,
            value,
            exclusion: index == 2,
        })
    } else {
        None
    }
}

pub(crate) fn c_string_in_buffer_equals(buffer: &[libc::c_char; MAXLEN], candidate: &str) -> bool {
    let end = buffer
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(buffer.len());
    buffer[..end]
        .iter()
        .map(|character| *character as u8)
        .eq(candidate.bytes())
}

pub(crate) fn parse_value_type(type_of_change: &[libc::c_char; MAXLEN]) -> u8 {
    if c_string_in_buffer_equals(type_of_change, "abs") {
        TYPE_ABS as u8
    } else if c_string_in_buffer_equals(type_of_change, "rel") {
        TYPE_REL as u8
    } else {
        0
    }
}

pub(crate) fn parse_resize_handle(handle: &[libc::c_char; MAXLEN]) -> u8 {
    if c_string_in_buffer_equals(handle, "top") {
        ResizeHandle::TOP.0
    } else if c_string_in_buffer_equals(handle, "bottom") {
        ResizeHandle::BOTTOM.0
    } else if c_string_in_buffer_equals(handle, "left") {
        ResizeHandle::LEFT.0
    } else if c_string_in_buffer_equals(handle, "right") {
        ResizeHandle::RIGHT.0
    } else if c_string_in_buffer_equals(handle, "top_left") {
        ResizeHandle::TOP.0 | ResizeHandle::LEFT.0
    } else if c_string_in_buffer_equals(handle, "top_right") {
        ResizeHandle::TOP.0 | ResizeHandle::RIGHT.0
    } else if c_string_in_buffer_equals(handle, "bottom_left") {
        ResizeHandle::BOTTOM.0 | ResizeHandle::LEFT.0
    } else if c_string_in_buffer_equals(handle, "bottom_right") {
        ResizeHandle::BOTTOM.0 | ResizeHandle::RIGHT.0
    } else if c_string_in_buffer_equals(handle, "abs") {
        ResizeHandle::ABS.0
    } else {
        0
    }
}

pub(crate) fn parse_label(
    response: &mut Response,
    message_bytes: &[u8],
    token: Token,
    label_type: LabelType,
    label: &mut Option<String>,
) -> bool {
    let value = token_to_value(token, message_bytes);

    if matches!(value.type_of_value, TokenType::Invalid) {
        *label = None;
        return true;
    }

    if !matches!(value.type_of_value, TokenType::String) {
        response.fail_pieces(&[
            FailurePiece::Text("'"),
            FailurePiece::Bytes(token.bytes(message_bytes)),
            FailurePiece::Text("' cannot be used as a label.\n"),
        ]);
        return false;
    }

    match label_type {
        LabelType::Display => {
            for index in 0..RESERVED_DISPLAY_IDENTIFIERS.len() {
                if token_equals(token, message_bytes, RESERVED_DISPLAY_IDENTIFIERS[index]) {
                    response.fail_pieces(&[
                        FailurePiece::Text("'"),
                        FailurePiece::Bytes(token.bytes(message_bytes)),
                        FailurePiece::Text(
                            "' is a reserved keyword and cannot be used as a label.\n",
                        ),
                    ]);
                    return false;
                }
            }
        }
        LabelType::Space => {
            for index in 0..RESERVED_SPACE_IDENTIFIERS.len() {
                if token_equals(token, message_bytes, RESERVED_SPACE_IDENTIFIERS[index]) {
                    response.fail_pieces(&[
                        FailurePiece::Text("'"),
                        FailurePiece::Bytes(token.bytes(message_bytes)),
                        FailurePiece::Text(
                            "' is a reserved keyword and cannot be used as a label.\n",
                        ),
                    ]);
                    return false;
                }
            }
        }
        LabelType::Window => {
            for index in 0..RESERVED_WINDOW_IDENTIFIERS.len() {
                if token_equals(token, message_bytes, RESERVED_WINDOW_IDENTIFIERS[index]) {
                    response.fail_pieces(&[
                        FailurePiece::Text("'"),
                        FailurePiece::Bytes(token.bytes(message_bytes)),
                        FailurePiece::Text(
                            "' is a reserved keyword and cannot be used as a scratchpad.\n",
                        ),
                    ]);
                    return false;
                }
            }
        }
    }

    *label = Some(String::from_utf8_lossy(token.bytes(message_bytes)).into_owned());

    true
}

pub(crate) fn parse_property(
    properties: &mut Properties,
    property: &[u8],
    property_values: &[u64],
    property_strings: &[&str],
) -> bool {
    for index in 0..property_strings.len() {
        if property == property_strings[index].as_bytes() {
            properties.flags |= property_values[index];
            return true;
        }
    }

    false
}

pub(crate) fn parse_properties(
    response: &mut Response,
    message_bytes: &mut [u8],
    token: Token,
    property_values: &[u64],
    property_strings: &[&str],
) -> Properties {
    let mut result = Properties {
        token,
        did_parse: false,
        did_error: false,
        flags: 0,
    };

    result.did_parse = token.is_valid() && !token_prefix(token, message_bytes, "--");
    if !result.did_parse {
        return result;
    }

    let mut cursor = 0;
    for index in 0..token.length {
        if index + 1 == token.length {
            let property = c_string_at(message_bytes, token.start + cursor);
            if !parse_property(&mut result, property, property_values, property_strings) {
                let reported = &message_bytes[token.start + cursor..token.start + index + 1];
                response.fail_pieces(&[
                    FailurePiece::Text("'"),
                    FailurePiece::BytesStoppingAtFirstNull(reported),
                    FailurePiece::Text("' is not a valid property.\n"),
                ]);
                result.did_error = true;
            }
        } else if message_bytes[token.start + index] == b',' {
            message_bytes[token.start + index] = 0;

            let property = c_string_at(message_bytes, token.start + cursor);
            if !parse_property(&mut result, property, property_values, property_strings) {
                let reported = &message_bytes[token.start + cursor..token.start + index + 1];
                response.fail_pieces(&[
                    FailurePiece::Text("'"),
                    FailurePiece::BytesStoppingAtFirstNull(reported),
                    FailurePiece::Text("' is not a valid property.\n"),
                ]);
                result.did_error = true;
            }

            cursor = index + 1;
        }
    }

    result
}

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
        TokenType::Unknown | TokenType::Float(_) | TokenType::U32(_) => {
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
        TokenType::Unknown | TokenType::Float(_) | TokenType::U32(_) => {
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
        TokenType::Unknown | TokenType::Float(_) | TokenType::U32(_) => {
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

pub(crate) fn daemon_fail_with_unknown_value_given_to_command_for_domain(
    response: &mut Response,
    message_bytes: &[u8],
    value: Token,
    command: Token,
    domain: Token,
) {
    response.fail_pieces(&[
        FailurePiece::Text("unknown value '"),
        FailurePiece::Bytes(value.bytes(message_bytes)),
        FailurePiece::Text("' given to command '"),
        FailurePiece::Bytes(command.bytes(message_bytes)),
        FailurePiece::Text("' for domain '"),
        FailurePiece::Bytes(domain.bytes(message_bytes)),
        FailurePiece::Text("'\n"),
    ]);
}

pub(crate) fn daemon_fail_with_unknown_option_given_to_command_for_domain(
    response: &mut Response,
    message_bytes: &[u8],
    option: Token,
    command: Token,
    domain: Token,
) {
    response.fail_pieces(&[
        FailurePiece::Text("unknown option '"),
        FailurePiece::Bytes(option.bytes(message_bytes)),
        FailurePiece::Text("' given to command '"),
        FailurePiece::Bytes(command.bytes(message_bytes)),
        FailurePiece::Text("' for domain '"),
        FailurePiece::Bytes(domain.bytes(message_bytes)),
        FailurePiece::Text("'\n"),
    ]);
}

pub(crate) fn daemon_fail_with_invalid_value_for_key(
    response: &mut Response,
    value: &[u8],
    key: &[u8],
) {
    response.fail_pieces(&[
        FailurePiece::Text("invalid value '"),
        FailurePiece::Bytes(value),
        FailurePiece::Text("' for key '"),
        FailurePiece::Bytes(key),
        FailurePiece::Text("'\n"),
    ]);
}

pub(crate) fn daemon_fail_with_invalid_regex_pattern_for_key(
    response: &mut Response,
    value: &[u8],
    key: &[u8],
) {
    response.fail_pieces(&[
        FailurePiece::Text("invalid regex pattern '"),
        FailurePiece::Bytes(value),
        FailurePiece::Text("' for key '"),
        FailurePiece::Bytes(key),
        FailurePiece::Text("'\n"),
    ]);
}

pub(crate) fn daemon_fail_with_unknown_command_for_domain(
    response: &mut Response,
    message_bytes: &[u8],
    command: Token,
    domain: Token,
) {
    response.fail_pieces(&[
        FailurePiece::Text("unknown command '"),
        FailurePiece::Bytes(command.bytes(message_bytes)),
        FailurePiece::Text("' for domain '"),
        FailurePiece::Bytes(domain.bytes(message_bytes)),
        FailurePiece::Text("'\n"),
    ]);
}

pub(crate) fn handle_domain_config(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut selector_space_id = SpaceId(0);
    let selector = message_cursor.get_token();
    let mut command = selector;

    let found_selector = token_equals(selector, message_cursor.bytes(), SELECTOR_CONFIG_SPACE);
    if found_selector {
        let space_selector =
            parse_space_selector(response, message_cursor, SpaceId(0), false, space_manager);
        let Some(space_id) = space_selector.resolved() else {
            return;
        };

        selector_space_id = space_id;
        command = message_cursor.get_token();
    }

    while command.is_valid() {
        if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_DEBUG_OUTPUT) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[VERBOSE.load(Ordering::Relaxed) as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                VERBOSE.store(false, Ordering::Relaxed);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                VERBOSE.store(true, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_MFF) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[window_manager.enable_mff as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                window_manager.enable_mff = false;
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                window_manager.enable_mff = true;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_FFM) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    FFM_MODE_STR[window_manager.ffm_mode as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                window_manager_set_focus_follows_mouse(window_manager, FfmMode::Disabled);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_FFM_AUTOFOCUS,
            ) {
                window_manager_set_focus_follows_mouse(window_manager, FfmMode::Autofocus);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_FFM_AUTORAISE,
            ) {
                window_manager_set_focus_follows_mouse(window_manager, FfmMode::Autoraise);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_DISPLAY_ORDER) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    DISPLAY_ARRANGEMENT_ORDER_STR[display_manager.order as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_DISPLAY_ORDER_DEFAULT,
            ) {
                display_manager.order = DisplayArrangementOrder::Default;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_DISPLAY_ORDER_X,
            ) {
                display_manager.order = DisplayArrangementOrder::X;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_DISPLAY_ORDER_Y,
            ) {
                display_manager.order = DisplayArrangementOrder::Y;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_WINDOW_ORIGIN) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_ORIGIN_MODE_STR[window_manager.window_origin_mode as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_ORIGIN_DEFAULT,
            ) {
                window_manager.window_origin_mode = WindowOriginMode::Default;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_ORIGIN_FOCUSED,
            ) {
                window_manager.window_origin_mode = WindowOriginMode::Focused;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_ORIGIN_CURSOR,
            ) {
                window_manager.window_origin_mode = WindowOriginMode::Cursor;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_PLACEMENT,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_NODE_CHILD_STR[space_manager.window_placement as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_PLACEMENT_FST,
            ) {
                space_manager.window_placement = WindowNodeChild::First;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_PLACEMENT_SND,
            ) {
                space_manager.window_placement = WindowNodeChild::Second;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_INSERT_POINT,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_INSERTION_POINT_STR[space_manager.window_insertion_point as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_INSERT_FOCUSED,
            ) {
                space_manager.window_insertion_point = WindowInsertionPoint::Focused;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_INSERT_FIRST,
            ) {
                space_manager.window_insertion_point = WindowInsertionPoint::First;
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_WINDOW_INSERT_LAST,
            ) {
                space_manager.window_insertion_point = WindowInsertionPoint::Last;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_WINDOW_ZOOM_PERSIST,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[space_manager.window_zoom_persist as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                space_manager.window_zoom_persist = false;
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                space_manager.window_zoom_persist = true;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_SKIP_SPACE_ANIMATION,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[space_manager.skip_window_focus_animation as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                space_manager.skip_window_focus_animation = false;
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                space_manager.skip_window_focus_animation = true;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_OPACITY) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    BOOL_STR[window_manager.enable_window_opacity as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                window_manager_set_window_opacity_enabled(window_manager, false);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                window_manager_set_window_opacity_enabled(window_manager, true);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_OPACITY_DURATION,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_six_decimals_as_printf_does(window_manager.window_opacity_duration as f64)
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value {
                window_manager.window_opacity_duration = float_value;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_ANIMATION_DURATION,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{}\n",
                    format_float_with_six_decimals_as_printf_does(window_manager.window_animation_duration as f64)
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value {
                if float_value == 0.0f32 {
                    window_manager.window_animation_duration = float_value;
                } else if !scripting_addition_is_sip_friendly() {
                    response.fail_pieces(&[
                        FailurePiece::Text("command '"),
                        FailurePiece::Bytes(command.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' for domain '"),
                        FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
                        FailurePiece::Text(
                            "' requires System Integrity Protection to be partially disabled! ignoring request..\n",
                        ),
                    ]);
                } else if CGPreflightScreenCaptureAccess() {
                    window_manager.window_animation_duration = float_value;
                } else {
                    response.fail_pieces(&[
                        FailurePiece::Text("command '"),
                        FailurePiece::Bytes(command.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' for domain '"),
                        FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
                        FailurePiece::Text(
                            "' requires Screen Recording permissions! ignoring request..\n",
                        ),
                    ]);
                    CGRequestScreenCaptureAccess();
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
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_ANIMATION_EASING,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    ANIMATION_EASING_TYPE_STR[window_manager.window_animation_easing as usize]
                ));
            } else {
                let mut found_match = false;
                for index in 0..EASING_TYPE_COUNT {
                    if token_equals(value, message_cursor.bytes(), ANIMATION_EASING_TYPE_STR[index])
                    {
                        if let Some(easing) = AnimationEasingType::from_index(index) {
                            window_manager.window_animation_easing = easing;
                        }
                        found_match = true;
                        break;
                    }
                }
                if !found_match {
                    daemon_fail_with_unknown_value_given_to_command_for_domain(
                        response,
                        message_cursor.bytes(),
                        value,
                        command,
                        domain,
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_SHADOW) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    PURIFY_MODE_STR[window_manager.purify_mode as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                window_manager_set_purify_mode(window_manager, PurifyMode::Always);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_SHADOW_FLT) {
                window_manager_set_purify_mode(window_manager, PurifyMode::Managed);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                window_manager_set_purify_mode(window_manager, PurifyMode::Disabled);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MENUBAR_OPACITY,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{:.4}\n",
                    window_manager.menubar_opacity as f64
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value
                && in_range_ii(float_value, 0.0f32, 1.0f32)
            {
                window_manager_set_menubar_opacity(window_manager, float_value);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_ACTIVE_WINDOW_OPACITY,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{:.4}\n",
                    window_manager.active_window_opacity as f64
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value
                && in_range_ei(float_value, 0.0f32, 1.0f32)
            {
                window_manager_set_active_window_opacity(window_manager, float_value);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_NORMAL_WINDOW_OPACITY,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "{:.4}\n",
                    window_manager.normal_window_opacity as f64
                ));
            } else if let TokenType::Float(float_value) = value.type_of_value
                && in_range_ei(float_value, 0.0f32, 1.0f32)
            {
                window_manager_set_normal_window_opacity(window_manager, float_value);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_INSERT_FEEDBACK_COLOR,
        ) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!(
                    "0x{:x}\n",
                    window_manager.insert_feedback_color.packed
                ));
            } else if let TokenType::U32(u32_value) = value.type_of_value
                && u32_value != 0
            {
                window_manager.insert_feedback_color = rgba_color_from_hex(u32_value);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_TOP_PADDING) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.top_padding));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::TOP_PADDING);
                        view.top_padding = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
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
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.top_padding));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_top_padding_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_BOTTOM_PADDING) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.bottom_padding));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::BOTTOM_PADDING);
                        view.bottom_padding = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
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
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.bottom_padding));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_bottom_padding_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_LEFT_PADDING) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.left_padding));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::LEFT_PADDING);
                        view.left_padding = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
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
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.left_padding));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_left_padding_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_RIGHT_PADDING) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.right_padding));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::RIGHT_PADDING);
                        view.right_padding = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
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
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.right_padding));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_right_padding_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_WINDOW_GAP) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if let TokenType::Invalid = value.type_of_value {
                        response.write(format_args!("{}\n", view.window_gap));
                    } else if let TokenType::Int(int_value) = value.type_of_value {
                        view.set_flag(ViewFlag::WINDOW_GAP);
                        view.window_gap = int_value;
                        view_update(space_manager, view_space_id, display_manager, window_manager);
                        view_flush(space_manager, view_space_id, window_manager);
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
            } else if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{}\n", space_manager.window_gap));
            } else if let TokenType::Int(int_value) = value.type_of_value {
                space_manager_set_window_gap_for_all_spaces(
                    space_manager,
                    int_value,
                    display_manager,
                    window_manager,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_LAYOUT) {
            let value = message_cursor.get_token();
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if !value.is_valid() {
                    if let Some(view) = space_manager.view.find(&view_space_id) {
                        response.write(format_args!("{}\n", VIEW_TYPE_STR[view.layout as usize]));
                    }
                } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_BSP) {
                    if space_is_user(selector_space_id) {
                        if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                            view.set_flag(ViewFlag::LAYOUT);
                            view.layout = ViewType::Bsp;
                        }
                        view_clear(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                            mouse_drag_state,
                        );
                        window_manager_validate_and_check_for_windows_on_space(
                            space_manager,
                            window_manager,
                            selector_space_id,
                            display_manager,
                            mouse_drag_state,
                        );
                    } else {
                        daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
                    }
                } else if token_equals(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_CONFIG_LAYOUT_STACK,
                ) {
                    if space_is_user(selector_space_id) {
                        if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                            view.set_flag(ViewFlag::LAYOUT);
                            view.layout = ViewType::Stack;
                        }
                        view_clear(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                            mouse_drag_state,
                        );
                        window_manager_validate_and_check_for_windows_on_space(
                            space_manager,
                            window_manager,
                            selector_space_id,
                            display_manager,
                            mouse_drag_state,
                        );
                    } else {
                        daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
                    }
                } else if token_equals(
                    value,
                    message_cursor.bytes(),
                    ARGUMENT_CONFIG_LAYOUT_FLOAT,
                ) {
                    if space_is_user(selector_space_id) {
                        if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                            view.set_flag(ViewFlag::LAYOUT);
                            view.layout = ViewType::Float;
                        }
                        view_clear(
                            space_manager,
                            view_space_id,
                            display_manager,
                            window_manager,
                            mouse_drag_state,
                        );
                    } else {
                        daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
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
            } else if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    VIEW_TYPE_STR[space_manager.layout as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_BSP) {
                space_manager_set_layout_for_all_spaces(
                    space_manager,
                    ViewType::Bsp,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_STACK) {
                space_manager_set_layout_for_all_spaces(
                    space_manager,
                    ViewType::Stack,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_LAYOUT_FLOAT) {
                space_manager_set_layout_for_all_spaces(
                    space_manager,
                    ViewType::Float,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_SPLIT_RATIO) {
            let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
            if let TokenType::Invalid = value.type_of_value {
                response.write(format_args!("{:.4}\n", space_manager.split_ratio as f64));
            } else if let TokenType::Float(float_value) = value.type_of_value
                && in_range_ii(float_value, 0.1f32, 0.9f32)
            {
                space_manager.split_ratio = float_value;
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value.token,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_SPLIT_TYPE) {
            let value = message_cursor.get_token();
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if !value.is_valid() {
                        response.write(format_args!(
                            "{}\n",
                            WINDOW_NODE_SPLIT_STR[view.split_type as usize]
                        ));
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_CONFIG_SPLIT_TYPE_Y,
                    ) {
                        view.set_flag(ViewFlag::SPLIT_TYPE);
                        view.split_type = WindowNodeSplit::Y;
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_CONFIG_SPLIT_TYPE_X,
                    ) {
                        view.set_flag(ViewFlag::SPLIT_TYPE);
                        view.split_type = WindowNodeSplit::X;
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_CONFIG_SPLIT_TYPE_AUTO,
                    ) {
                        view.set_flag(ViewFlag::SPLIT_TYPE);
                        view.split_type = WindowNodeSplit::Auto;
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
            } else if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    WINDOW_NODE_SPLIT_STR[space_manager.split_type as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_SPLIT_TYPE_Y) {
                space_manager_set_split_type_for_all_spaces(space_manager, WindowNodeSplit::Y);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_SPLIT_TYPE_X) {
                space_manager_set_split_type_for_all_spaces(space_manager, WindowNodeSplit::X);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_SPLIT_TYPE_AUTO,
            ) {
                space_manager_set_split_type_for_all_spaces(space_manager, WindowNodeSplit::Auto);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_AUTO_BALANCE) {
            let value = message_cursor.get_token();
            if selector_space_id != SpaceId(0) {
                let view_space_id = space_manager_find_view(
                    space_manager,
                    selector_space_id,
                    display_manager,
                    window_manager,
                );
                if let Some(view) = space_manager.view.find_mut(&view_space_id) {
                    if !value.is_valid() {
                        response.write(format_args!(
                            "{}\n",
                            AUTO_BALANCE_STR[view.auto_balance as usize]
                        ));
                    } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                        view.set_flag(ViewFlag::AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::None as u32;
                    } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                        view.set_flag(ViewFlag::AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::X as u32 | WindowNodeSplit::Y as u32;
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_VAL_AXIS_X,
                    ) {
                        view.set_flag(ViewFlag::AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::X as u32;
                    } else if token_equals(
                        value,
                        message_cursor.bytes(),
                        ARGUMENT_COMMON_VAL_AXIS_Y,
                    ) {
                        view.set_flag(ViewFlag::AUTO_BALANCE);
                        view.auto_balance = WindowNodeSplit::Y as u32;
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
            } else if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    AUTO_BALANCE_STR[space_manager.auto_balance as usize]
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_OFF) {
                space_manager_set_auto_balance_for_all_spaces(
                    space_manager,
                    WindowNodeSplit::None as u32,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_ON) {
                space_manager_set_auto_balance_for_all_spaces(
                    space_manager,
                    WindowNodeSplit::X as u32 | WindowNodeSplit::Y as u32,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_X) {
                space_manager_set_auto_balance_for_all_spaces(
                    space_manager,
                    WindowNodeSplit::X as u32,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_Y) {
                space_manager_set_auto_balance_for_all_spaces(
                    space_manager,
                    WindowNodeSplit::Y as u32,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_MOUSE_MOD) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MOD_STR[MOUSE_TAP_STATE.modifier.load(Ordering::Relaxed) as usize]
                        .unwrap_or("(null)")
                ));
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_MOUSE_MOD_ALT) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::ALT.0, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_MOD_SHIFT,
            ) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::SHIFT.0, Ordering::Relaxed);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_MOUSE_MOD_CMD) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::CMD.0, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_MOD_CTRL,
            ) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::CTRL.0, Ordering::Relaxed);
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_CONFIG_MOUSE_MOD_FN) {
                MOUSE_TAP_STATE
                    .modifier
                    .store(MouseMod::FN.0, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_MOUSE_ACTION1) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODE_STR[MOUSE_TAP_STATE.action1.load(Ordering::Relaxed) as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_MOVE,
            ) {
                MOUSE_TAP_STATE
                    .action1
                    .store(MouseMode::Move as u8, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_RESIZE,
            ) {
                MOUSE_TAP_STATE
                    .action1
                    .store(MouseMode::Resize as u8, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_MOUSE_ACTION2) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODE_STR[MOUSE_TAP_STATE.action2.load(Ordering::Relaxed) as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_MOVE,
            ) {
                MOUSE_TAP_STATE
                    .action2
                    .store(MouseMode::Move as u8, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_RESIZE,
            ) {
                MOUSE_TAP_STATE
                    .action2
                    .store(MouseMode::Resize as u8, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(
            command,
            message_cursor.bytes(),
            COMMAND_CONFIG_MOUSE_DROP_ACTION,
        ) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                response.write(format_args!(
                    "{}\n",
                    MOUSE_MODE_STR[MOUSE_TAP_STATE.drop_action.load(Ordering::Relaxed) as usize]
                ));
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_SWAP,
            ) {
                MOUSE_TAP_STATE
                    .drop_action
                    .store(MouseMode::Swap as u8, Ordering::Relaxed);
            } else if token_equals(
                value,
                message_cursor.bytes(),
                ARGUMENT_CONFIG_MOUSE_ACTION_STACK,
            ) {
                MOUSE_TAP_STATE
                    .drop_action
                    .store(MouseMode::Stack as u8, Ordering::Relaxed);
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_CONFIG_EXTERNAL_BAR) {
            let mut top: libc::c_int = 0;
            let mut bottom: libc::c_int = 0;
            let mut mode = [0 as libc::c_char; 6];
            let value = message_cursor.get_token();
            let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                .unwrap_or_default();
            let converted = unsafe {
                libc::sscanf(
                    subject.as_ptr(),
                    ARGUMENT_CONFIG_EXTERNAL_BAR.as_ptr(),
                    mode.as_mut_ptr(),
                    &mut top as *mut libc::c_int,
                    &mut bottom as *mut libc::c_int,
                )
            };
            if converted == 3 {
                let mode = mode.map(|character| character as u8);
                let mode = c_string_at(&mode, 0);
                if mode == ARGUMENT_CONFIG_EXTERNAL_BAR_MAIN.as_bytes() {
                    display_manager.mode = ExternalBarMode::Main;
                    display_manager.top_padding = top;
                    display_manager.bottom_padding = bottom;
                    space_manager_mark_spaces_invalid(space_manager, display_manager, window_manager);
                } else if mode == ARGUMENT_CONFIG_EXTERNAL_BAR_ALL.as_bytes() {
                    display_manager.mode = ExternalBarMode::All;
                    display_manager.top_padding = top;
                    display_manager.bottom_padding = bottom;
                    space_manager_mark_spaces_invalid(space_manager, display_manager, window_manager);
                } else if mode == ARGUMENT_COMMON_VAL_OFF.as_bytes() {
                    display_manager.mode = ExternalBarMode::Off;
                    display_manager.top_padding = top;
                    display_manager.bottom_padding = bottom;
                    space_manager_mark_spaces_invalid(space_manager, display_manager, window_manager);
                } else {
                    response.fail_pieces(&[
                        FailurePiece::Text("unknown mode '"),
                        FailurePiece::Bytes(mode),
                        FailurePiece::Text("' specified in value '"),
                        FailurePiece::Bytes(value.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' given to command '"),
                        FailurePiece::Bytes(command.bytes(message_cursor.bytes())),
                        FailurePiece::Text("' for domain '"),
                        FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
                        FailurePiece::Text("'\n"),
                    ]);
                }
            } else {
                response.write(format_args!(
                    "{}:{}:{}\n",
                    EXTERNAL_BAR_MODE_STR[display_manager.mode as usize],
                    display_manager.top_padding,
                    display_manager.bottom_padding
                ));
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

pub(crate) fn handle_domain_display(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mission_control_mode: &mut MissionControlMode,
) {
    let command;
    let mut acting_display_id = display_manager_active_display_id();
    let selector = parse_display_selector(
        &mut Response::silent(),
        message_cursor,
        acting_display_id,
        true,
        display_manager,
    );

    if selector.did_parse() {
        acting_display_id = selector.resolved().unwrap_or(DisplayId(0));
        command = message_cursor.get_token();
    } else {
        command = selector.token;
    }

    if acting_display_id == DisplayId(0) {
        daemon_fail!(response, "could not locate the display to act on!\n");
        return;
    }

    if token_equals(command, message_cursor.bytes(), COMMAND_DISPLAY_FOCUS) {
        let selector = parse_display_selector(
            response,
            message_cursor,
            acting_display_id,
            false,
            display_manager,
        );
        if let Some(selector_display_id) = selector.resolved() {
            if acting_display_id != selector_display_id {
                display_manager_focus_display(
                    selector_display_id,
                    display_space_id(selector_display_id),
                    window_manager,
                );
            } else {
                daemon_fail!(response, "cannot focus an already focused display.\n");
            }
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_DISPLAY_SPACE) {
        let selector = parse_space_selector(
            response,
            message_cursor,
            display_space_id(acting_display_id),
            false,
            space_manager,
        );
        if let Some(selector_space_id) = selector.resolved() {
            let result =
                display_manager_focus_space(acting_display_id, selector_space_id, mission_control_mode);
            if result == SpaceOpError::SameDisplay {
                daemon_fail!(response, "acting display does not contain the given space.\n");
            } else if result == SpaceOpError::DisplayIsAnimating {
                daemon_fail!(
                    response,
                    "cannot focus space because the display is in the middle of an animation.\n"
                );
            } else if result == SpaceOpError::InMissionControl {
                daemon_fail!(response, "cannot focus space because mission-control is active.\n");
            } else if result == SpaceOpError::ScriptingAddition {
                daemon_fail!(
                    response,
                    "cannot focus space due to an error with the scripting-addition.\n"
                );
            }
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_DISPLAY_LABEL) {
        let mut label = None;
        let token = message_cursor.get_token();
        if parse_label(
            response,
            message_cursor.bytes(),
            token,
            LabelType::Display,
            &mut label,
        ) {
            if let Some(label) = label {
                display_manager_set_label_for_display(display_manager, acting_display_id, label);
            } else if !display_manager_remove_label_for_display(display_manager, acting_display_id)
            {
                daemon_fail!(
                    response,
                    "the selected display was not associated with a label!\n"
                );
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
}

pub(crate) fn handle_domain_space(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let mut command;
    let mut acting_space_id = space_manager_active_space(window_manager);
    let selector = parse_space_selector(
        &mut Response::silent(),
        message_cursor,
        acting_space_id,
        true,
        space_manager,
    );

    if selector.did_parse() {
        acting_space_id = selector.resolved().unwrap_or(SpaceId(0));
        command = message_cursor.get_token();
    } else {
        command = selector.token;
    }

    if acting_space_id == SpaceId(0) {
        daemon_fail!(response, "could not locate the space to act on!\n");
        return;
    }

    while command.is_valid() {
        if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_FOCUS) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved() {
                let result =
                    space_manager_focus_space(selector_space_id, window_manager, mission_control_mode);
                if result == SpaceOpError::SameSpace {
                    daemon_fail!(response, "cannot focus an already focused space.\n");
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot focus space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot focus space because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot focus space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_SWITCH) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved() {
                let result = space_manager_switch_space(
                    selector_space_id,
                    display_manager,
                    window_manager,
                    space_manager,
                    mission_control_mode,
                    mouse_drag_state,
                );
                if result == SpaceOpError::SameSpace {
                    daemon_fail!(response, "cannot focus an already focused space.\n");
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot focus space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot focus space because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot focus space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_MOVE) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved() {
                let result = space_manager_move_space_to_space(
                    acting_space_id,
                    selector_space_id,
                    window_manager,
                    mission_control_mode,
                );
                if result == SpaceOpError::SameSpace {
                    daemon_fail!(response, "cannot move space to itself.\n");
                } else if result == SpaceOpError::SameDisplay {
                    daemon_fail!(
                        response,
                        "cannot move space across display boundaries. use --display instead.\n"
                    );
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot move space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot move space because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot move space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_SWAP) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                false,
                space_manager,
            );
            if let Some(selector_space_id) = selector.resolved() {
                let result = space_manager_swap_space_with_space(
                    acting_space_id,
                    selector_space_id,
                    display_manager,
                    window_manager,
                    space_manager,
                    mission_control_mode,
                    mouse_drag_state,
                );
                if result == SpaceOpError::SameSpace {
                    daemon_fail!(response, "cannot swap space with itself.\n");
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot swap space because the display is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot swap space because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot swap space due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_DISPLAY) {
            let selector = parse_display_selector(
                response,
                message_cursor,
                display_manager_active_display_id(),
                false,
                display_manager,
            );
            if let Some(selector_display_id) = selector.resolved() {
                let result = space_manager_move_space_to_display(
                    space_manager,
                    acting_space_id,
                    selector_display_id,
                    display_manager,
                    window_manager,
                    mission_control_mode,
                );
                if result == SpaceOpError::MissingSrc {
                    daemon_fail!(response, "could not locate the space to act on.\n");
                } else if result == SpaceOpError::MissingDst {
                    daemon_fail!(
                        response,
                        "could not locate the active space of the given display.\n"
                    );
                } else if result == SpaceOpError::InvalidSrc {
                    daemon_fail!(
                        response,
                        "acting space is the last user-space on the source display and cannot be moved.\n"
                    );
                } else if result == SpaceOpError::InvalidDst {
                    daemon_fail!(
                        response,
                        "acting space is already located on the given display.\n"
                    );
                } else if result == SpaceOpError::DisplayIsAnimating {
                    daemon_fail!(
                        response,
                        "cannot send space to display because it is in the middle of an animation.\n"
                    );
                } else if result == SpaceOpError::InMissionControl {
                    daemon_fail!(
                        response,
                        "cannot send space to display because mission-control is active.\n"
                    );
                } else if result == SpaceOpError::ScriptingAddition {
                    daemon_fail!(
                        response,
                        "cannot send space to display due to an error with the scripting-addition.\n"
                    );
                }
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_CREATE) {
            let selector = parse_display_selector(
                response,
                message_cursor,
                display_manager_active_display_id(),
                true,
                display_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_display_id) = selector.resolved() {
                    acting_space_id = display_space_id(selector_display_id);
                } else {
                    return;
                }
            }

            let result = space_manager_add_space(acting_space_id, mission_control_mode);
            if result == SpaceOpError::MissingSrc {
                daemon_fail!(response, "could not locate the space to act on.\n");
            } else if result == SpaceOpError::DisplayIsAnimating {
                daemon_fail!(
                    response,
                    "cannot create space because the display is in the middle of an animation.\n"
                );
            } else if result == SpaceOpError::InMissionControl {
                daemon_fail!(
                    response,
                    "cannot create space because mission-control is active.\n"
                );
            } else if result == SpaceOpError::ScriptingAddition {
                daemon_fail!(
                    response,
                    "cannot create space due to an error with the scripting-addition.\n"
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_DESTROY) {
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                true,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_space_id) = selector.resolved() {
                    acting_space_id = selector_space_id;
                } else {
                    return;
                }
            }

            let result = space_manager_destroy_space(
                acting_space_id,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            );
            if result == SpaceOpError::MissingSrc {
                daemon_fail!(response, "could not locate the space to act on.\n");
            } else if result == SpaceOpError::InvalidSrc {
                daemon_fail!(
                    response,
                    "acting space is the last user-space on the source display and cannot be destroyed.\n"
                );
            } else if result == SpaceOpError::InvalidType {
                daemon_fail!(response, "cannot destroy a macOS fullscreen space.\n");
            } else if result == SpaceOpError::DisplayIsAnimating {
                daemon_fail!(
                    response,
                    "cannot destroy space because the display is in the middle of an animation.\n"
                );
            } else if result == SpaceOpError::InMissionControl {
                daemon_fail!(
                    response,
                    "cannot destroy space because mission-control is active.\n"
                );
            } else if result == SpaceOpError::ScriptingAddition {
                daemon_fail!(
                    response,
                    "cannot destroy space due to an error with the scripting-addition.\n"
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_EQUALIZE) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                if !space_manager_equalize_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X as u32 | WindowNodeSplit::Y as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot equalize a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_X) {
                if !space_manager_equalize_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot equalize a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_Y) {
                if !space_manager_equalize_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Y as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot equalize a non-managed space.\n");
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_BALANCE) {
            let value = message_cursor.get_token();
            if !value.is_valid() {
                if !space_manager_balance_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X as u32 | WindowNodeSplit::Y as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot balance a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_X) {
                if !space_manager_balance_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot balance a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_Y) {
                if !space_manager_balance_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Y as u32,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot balance a non-managed space.\n");
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_MIRROR) {
            let value = message_cursor.get_token();
            if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_X) {
                if !space_manager_mirror_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::X,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot mirror a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_COMMON_VAL_AXIS_Y) {
                if !space_manager_mirror_space(
                    space_manager,
                    acting_space_id,
                    WindowNodeSplit::Y,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot mirror a non-managed space.\n");
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_ROTATE) {
            let value = message_cursor.get_token();
            if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_ROTATE_90) {
                if !space_manager_rotate_space(
                    space_manager,
                    acting_space_id,
                    90,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot rotate a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_ROTATE_180) {
                if !space_manager_rotate_space(
                    space_manager,
                    acting_space_id,
                    180,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot rotate a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_ROTATE_270) {
                if !space_manager_rotate_space(
                    space_manager,
                    acting_space_id,
                    270,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot rotate a non-managed space.\n");
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_PADDING) {
            let mut top: libc::c_int = 0;
            let mut bottom: libc::c_int = 0;
            let mut left: libc::c_int = 0;
            let mut right: libc::c_int = 0;
            let mut type_of_change = [0 as libc::c_char; MAXLEN];
            let value = message_cursor.get_token();
            let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                .unwrap_or_default();
            let converted = unsafe {
                libc::sscanf(
                    subject.as_ptr(),
                    ARGUMENT_SPACE_PADDING.as_ptr(),
                    type_of_change.as_mut_ptr(),
                    &mut top as *mut libc::c_int,
                    &mut bottom as *mut libc::c_int,
                    &mut left as *mut libc::c_int,
                    &mut right as *mut libc::c_int,
                )
            };
            if converted == 5 {
                if !space_manager_set_padding_for_space(
                    space_manager,
                    acting_space_id,
                    parse_value_type(&type_of_change) as i32,
                    top,
                    bottom,
                    left,
                    right,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot set padding for a non-managed space.\n");
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_GAP) {
            let mut gap: libc::c_int = 0;
            let mut type_of_change = [0 as libc::c_char; MAXLEN];
            let value = message_cursor.get_token();
            let subject = std::ffi::CString::new(value.bytes(message_cursor.bytes()))
                .unwrap_or_default();
            let converted = unsafe {
                libc::sscanf(
                    subject.as_ptr(),
                    ARGUMENT_SPACE_GAP.as_ptr(),
                    type_of_change.as_mut_ptr(),
                    &mut gap as *mut libc::c_int,
                )
            };
            if converted == 2 {
                if !space_manager_set_gap_for_space(
                    space_manager,
                    acting_space_id,
                    parse_value_type(&type_of_change) as i32,
                    gap,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot set gap for a non-managed space.\n");
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_TOGGLE) {
            let value = message_cursor.get_token();
            if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_TGL_PADDING) {
                if !space_manager_toggle_padding_for_space(
                    space_manager,
                    acting_space_id,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot toggle padding for a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_TGL_GAP) {
                if !space_manager_toggle_gap_for_space(
                    space_manager,
                    acting_space_id,
                    display_manager,
                    window_manager,
                ) {
                    daemon_fail!(response, "cannot toggle gap for a non-managed space.\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_TGL_MC) {
                space_manager_toggle_mission_control(
                    acting_space_id,
                    window_manager,
                    mission_control_mode,
                );
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_TGL_SD) {
                space_manager_toggle_show_desktop(
                    acting_space_id,
                    window_manager,
                    mission_control_mode,
                );
            } else {
                daemon_fail_with_unknown_value_given_to_command_for_domain(
                    response,
                    message_cursor.bytes(),
                    value,
                    command,
                    domain,
                );
            }
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_LAYOUT) {
            let value = message_cursor.get_token();
            if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_LAYOUT_BSP) {
                if space_is_user(acting_space_id) {
                    space_manager_set_layout_for_space(
                        space_manager,
                        acting_space_id,
                        ViewType::Bsp,
                        display_manager,
                        window_manager,
                        mouse_drag_state,
                    );
                } else {
                    daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_LAYOUT_STACK) {
                if space_is_user(acting_space_id) {
                    space_manager_set_layout_for_space(
                        space_manager,
                        acting_space_id,
                        ViewType::Stack,
                        display_manager,
                        window_manager,
                        mouse_drag_state,
                    );
                } else {
                    daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
                }
            } else if token_equals(value, message_cursor.bytes(), ARGUMENT_SPACE_LAYOUT_FLT) {
                if space_is_user(acting_space_id) {
                    space_manager_set_layout_for_space(
                        space_manager,
                        acting_space_id,
                        ViewType::Float,
                        display_manager,
                        window_manager,
                        mouse_drag_state,
                    );
                } else {
                    daemon_fail!(response, "cannot set layout for a macOS fullscreen space!\n");
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
        } else if token_equals(command, message_cursor.bytes(), COMMAND_SPACE_LABEL) {
            let mut label = None;
            let token = message_cursor.get_token();
            if parse_label(
                response,
                message_cursor.bytes(),
                token,
                LabelType::Space,
                &mut label,
            ) {
                if let Some(label) = label {
                    space_manager_set_label_for_space(space_manager, acting_space_id, label);
                } else if !space_manager_remove_label_for_space(space_manager, acting_space_id) {
                    daemon_fail!(response, "the selected space was not associated with a label!\n");
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

pub(crate) fn handle_domain_query(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let command = message_cursor.get_token();
    if token_equals(command, message_cursor.bytes(), COMMAND_QUERY_DISPLAYS) {
        let token = message_cursor.get_token();
        let properties = parse_properties(
            response,
            message_cursor.bytes_mut(),
            token,
            &DISPLAY_PROPERTY_VAL,
            &DISPLAY_PROPERTY_STR,
        );
        if properties.did_error {
            return;
        }

        let option = if properties.did_parse {
            message_cursor.get_token()
        } else {
            properties.token
        };
        if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_DISPLAY) {
            let mut acting_display_id = display_manager_active_display_id();
            let selector = parse_display_selector(
                response,
                message_cursor,
                acting_display_id,
                true,
                display_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_display_id) = selector.resolved() {
                    acting_display_id = selector_display_id;
                } else {
                    return;
                }
            }

            display_serialize(response, acting_display_id, properties.flags, display_manager);
            response.write(format_args!("\n"));
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_SPACE) {
            let mut acting_space_id = space_manager_active_space(window_manager);
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                true,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_space_id) = selector.resolved() {
                    acting_space_id = selector_space_id;
                } else {
                    return;
                }
            }

            display_serialize(
                response,
                space_display_id(acting_space_id),
                properties.flags,
                display_manager,
            );
            response.write(format_args!("\n"));
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_WINDOW) {
            let mut acting_window_id = window_manager_focused_window(window_manager);
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
                display_serialize(
                    response,
                    window_display_id(acting_window),
                    properties.flags,
                    display_manager,
                );
                response.write(format_args!("\n"));
            } else {
                daemon_fail!(
                    response,
                    "could not find window to retrieve display details.\n"
                );
            }
        } else if option.is_valid() {
            daemon_fail_with_unknown_option_given_to_command_for_domain(
                response,
                message_cursor.bytes(),
                option,
                command,
                domain,
            );
        } else {
            display_manager_query_displays(response, properties.flags, display_manager);
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_QUERY_SPACES) {
        let token = message_cursor.get_token();
        let properties = parse_properties(
            response,
            message_cursor.bytes_mut(),
            token,
            &SPACE_PROPERTY_VAL,
            &SPACE_PROPERTY_STR,
        );
        if properties.did_error {
            return;
        }

        let option = if properties.did_parse {
            message_cursor.get_token()
        } else {
            properties.token
        };
        if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_DISPLAY) {
            let mut acting_display_id = display_manager_active_display_id();
            let selector = parse_display_selector(
                response,
                message_cursor,
                acting_display_id,
                true,
                display_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_display_id) = selector.resolved() {
                    acting_display_id = selector_display_id;
                } else {
                    return;
                }
            }

            if !space_manager_query_spaces_for_display(
                response,
                acting_display_id,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
            ) {
                daemon_fail!(response, "could not retrieve spaces for display.\n");
            }
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_SPACE) {
            let mut acting_space_id = space_manager_active_space(window_manager);
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                true,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_space_id) = selector.resolved() {
                    acting_space_id = selector_space_id;
                } else {
                    return;
                }
            }

            if !space_manager_query_space(
                response,
                acting_space_id,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
            ) {
                daemon_fail!(response, "could not retrieve space details.\n");
            }
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_WINDOW) {
            let mut acting_window_id = window_manager_focused_window(window_manager);
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
                space_manager_query_spaces_for_window(
                    response,
                    acting_window,
                    properties.flags,
                    display_manager,
                    window_manager,
                    space_manager,
                );
            } else {
                daemon_fail!(response, "could not find window to retrieve space details.\n");
            }
        } else if option.is_valid() {
            daemon_fail_with_unknown_option_given_to_command_for_domain(
                response,
                message_cursor.bytes(),
                option,
                command,
                domain,
            );
        } else if !space_manager_query_spaces_for_displays(
            response,
            properties.flags,
            display_manager,
            window_manager,
            space_manager,
        ) {
            daemon_fail!(response, "could not retrieve spaces for displays.\n");
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_QUERY_WINDOWS) {
        let token = message_cursor.get_token();
        let properties = parse_properties(
            response,
            message_cursor.bytes_mut(),
            token,
            &WINDOW_PROPERTY_VAL,
            &WINDOW_PROPERTY_STR,
        );
        if properties.did_error {
            return;
        }

        let option = if properties.did_parse {
            message_cursor.get_token()
        } else {
            properties.token
        };
        if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_DISPLAY) {
            let mut acting_display_id = display_manager_active_display_id();
            let selector = parse_display_selector(
                response,
                message_cursor,
                acting_display_id,
                true,
                display_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_display_id) = selector.resolved() {
                    acting_display_id = selector_display_id;
                } else {
                    return;
                }
            }

            window_manager_query_windows_for_display(
                response,
                acting_display_id,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_SPACE) {
            let mut acting_space_id = space_manager_active_space(window_manager);
            let selector = parse_space_selector(
                response,
                message_cursor,
                acting_space_id,
                true,
                space_manager,
            );

            if selector.token.is_valid() {
                if let Some(selector_space_id) = selector.resolved() {
                    acting_space_id = selector_space_id;
                } else {
                    return;
                }
            }

            window_manager_query_windows_for_spaces(
                response,
                &[acting_space_id],
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        } else if token_equals(option, message_cursor.bytes(), ARGUMENT_QUERY_WINDOW) {
            let mut acting_window_id = window_manager_focused_window(window_manager);
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
                window_serialize(
                    response,
                    acting_window,
                    properties.flags,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                );
                response.write(format_args!("\n"));
            } else {
                daemon_fail!(response, "could not retrieve window details.\n");
            }
        } else if option.is_valid() {
            daemon_fail_with_unknown_option_given_to_command_for_domain(
                response,
                message_cursor.bytes(),
                option,
                command,
                domain,
            );
        } else {
            window_manager_query_windows_for_displays(
                response,
                properties.flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        }
    } else {
        daemon_fail_with_unknown_command_for_domain(
            response,
            message_cursor.bytes(),
            command,
            domain,
        );
    }
}

pub(crate) fn parse_rule(
    response: &mut Response,
    message_cursor: &mut MessageCursor,
    rule: &mut Rule,
    token: Token,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let mut unsupported_exclusion: Option<usize> = None;
    let mut did_parse = true;
    let mut has_filter = false;

    let mut token = token;
    while token.is_valid() {
        'iteration: {
            let Some(pair) = parse_key_value_pair(message_cursor.bytes_mut(), token.start) else {
                response.fail_pieces(&[
                    FailurePiece::Text("invalid key-value pair '"),
                    FailurePiece::Bytes(c_string_at(message_cursor.bytes(), token.start)),
                    FailurePiece::Text("'\n"),
                ]);
                did_parse = false;
                break 'iteration;
            };

            let key = c_string_at(message_cursor.bytes(), pair.key).to_vec();
            let value = c_string_at(message_cursor.bytes(), pair.value).to_vec();

            if key == ARGUMENT_RULE_KEY_LABEL.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }
                rule.label = Some(String::from_utf8_lossy(&value).into_owned());
            } else if key == ARGUMENT_RULE_KEY_SCRATCHPAD.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let mut valid = true;
                for index in 0..RESERVED_WINDOW_IDENTIFIERS.len() {
                    if value == RESERVED_WINDOW_IDENTIFIERS[index].as_bytes() {
                        valid = false;
                        break;
                    }
                }

                if valid {
                    rule.effects.scratchpad = Some(String::from_utf8_lossy(&value).into_owned());
                    rule.effects.manage = RULE_PROP_OFF;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_APP.as_bytes() {
                has_filter = true;
                rule.app = Some(String::from_utf8_lossy(&value).into_owned());
                if pair.exclusion {
                    rule.flags |= RuleFlag::APP_EXCLUDE.0;
                }
                rule.app_regex =
                    PosixRegex::compile(&CString::new(value.clone()).unwrap_or_default());
                if rule.app_regex.is_none() {
                    daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_TITLE.as_bytes() {
                has_filter = true;
                rule.title = Some(String::from_utf8_lossy(&value).into_owned());
                if pair.exclusion {
                    rule.flags |= RuleFlag::TITLE_EXCLUDE.0;
                }
                rule.title_regex =
                    PosixRegex::compile(&CString::new(value.clone()).unwrap_or_default());
                if rule.title_regex.is_none() {
                    daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_ROLE.as_bytes() {
                has_filter = true;
                rule.role = Some(String::from_utf8_lossy(&value).into_owned());
                if pair.exclusion {
                    rule.flags |= RuleFlag::ROLE_EXCLUDE.0;
                }
                rule.role_regex =
                    PosixRegex::compile(&CString::new(value.clone()).unwrap_or_default());
                if rule.role_regex.is_none() {
                    daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_SUBROLE.as_bytes() {
                has_filter = true;
                rule.subrole = Some(String::from_utf8_lossy(&value).into_owned());
                if pair.exclusion {
                    rule.flags |= RuleFlag::SUBROLE_EXCLUDE.0;
                }
                rule.subrole_regex =
                    PosixRegex::compile(&CString::new(value.clone()).unwrap_or_default());
                if rule.subrole_regex.is_none() {
                    daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_DISPLAY.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let mut value_start = pair.value;
                if message_cursor.bytes()[value_start] == ARGUMENT_RULE_VALUE_SPACE {
                    value_start += 1;
                    rule.effects.flags |= RuleEffectsFlag::FOLLOW_SPACE.0;
                }

                let acting_display_id = display_manager_active_display_id();
                let mut value_cursor = message_cursor.cursor_at(value_start);
                let selector = parse_display_selector(
                    response,
                    &mut value_cursor,
                    acting_display_id,
                    false,
                    display_manager,
                );
                if let Some(selector_display_id) = selector.resolved() {
                    rule.effects.display_id = selector_display_id;
                } else {
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_SPACE.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let mut value_start = pair.value;
                if message_cursor.bytes()[value_start] == ARGUMENT_RULE_VALUE_SPACE {
                    value_start += 1;
                    rule.effects.flags |= RuleEffectsFlag::FOLLOW_SPACE.0;
                }

                let acting_space_id = space_manager_active_space(window_manager);
                let mut value_cursor = message_cursor.cursor_at(value_start);
                let selector = parse_space_selector(
                    response,
                    &mut value_cursor,
                    acting_space_id,
                    false,
                    space_manager,
                );
                if let Some(selector_space_id) = selector.resolved() {
                    rule.effects.space_id = selector_space_id;
                } else {
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_GRID.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let subject = CString::new(value.clone()).unwrap_or_default();
                let grid = rule.effects.grid.as_mut_ptr() as *mut libc::c_int;
                let converted = unsafe {
                    libc::sscanf(
                        subject.as_ptr(),
                        ARGUMENT_RULE_VALUE_GRID.as_ptr(),
                        grid,
                        grid.add(1),
                        grid.add(2),
                        grid.add(3),
                        grid.add(4),
                        grid.add(5),
                    )
                };
                if converted != 6 {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_OPACITY.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                let subject = CString::new(value.clone()).unwrap_or_default();
                let converted = unsafe {
                    libc::sscanf(
                        subject.as_ptr(),
                        SCAN_ONE_FLOAT.as_ptr(),
                        &mut rule.effects.opacity as *mut libc::c_float,
                    )
                };
                if converted == 1 && in_range_ii(rule.effects.opacity, 0.0f32, 1.0f32) {
                    rule.effects.flags |= RuleEffectsFlag::OPACITY.0;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_MANAGE.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_COMMON_VAL_ON.as_bytes() {
                    rule.effects.manage = RULE_PROP_ON;
                } else if value == ARGUMENT_COMMON_VAL_OFF.as_bytes() {
                    rule.effects.manage = RULE_PROP_OFF;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_STICKY.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_COMMON_VAL_ON.as_bytes() {
                    rule.effects.sticky = RULE_PROP_ON;
                } else if value == ARGUMENT_COMMON_VAL_OFF.as_bytes() {
                    rule.effects.sticky = RULE_PROP_OFF;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_MFF.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_COMMON_VAL_ON.as_bytes() {
                    rule.effects.mff = RULE_PROP_ON;
                } else if value == ARGUMENT_COMMON_VAL_OFF.as_bytes() {
                    rule.effects.mff = RULE_PROP_OFF;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_SUB_LAYER.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_WINDOW_LAYER_BELOW.as_bytes() {
                    rule.effects.layer = LAYER_BELOW;
                    rule.effects.flags |= RuleEffectsFlag::LAYER.0;
                } else if value == ARGUMENT_WINDOW_LAYER_NORMAL.as_bytes() {
                    rule.effects.layer = LAYER_NORMAL;
                    rule.effects.flags |= RuleEffectsFlag::LAYER.0;
                } else if value == ARGUMENT_WINDOW_LAYER_ABOVE.as_bytes() {
                    rule.effects.layer = LAYER_ABOVE;
                    rule.effects.flags |= RuleEffectsFlag::LAYER.0;
                } else if value == ARGUMENT_WINDOW_LAYER_AUTO.as_bytes() {
                    rule.effects.layer = LAYER_AUTO;
                    rule.effects.flags |= RuleEffectsFlag::LAYER.0;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else if key == ARGUMENT_RULE_KEY_FULLSCR.as_bytes() {
                if pair.exclusion {
                    unsupported_exclusion = Some(pair.key);
                }

                if value == ARGUMENT_COMMON_VAL_ON.as_bytes() {
                    rule.effects.fullscreen = RULE_PROP_ON;
                } else if value == ARGUMENT_COMMON_VAL_OFF.as_bytes() {
                    rule.effects.fullscreen = RULE_PROP_OFF;
                } else {
                    daemon_fail_with_invalid_value_for_key(response, &value, &key);
                    did_parse = false;
                }
            } else {
                response.fail_pieces(&[
                    FailurePiece::Text("unknown key '"),
                    FailurePiece::Bytes(&key),
                    FailurePiece::Text("'\n"),
                ]);
                did_parse = false;
            }
        }

        token = message_cursor.get_token();
    }

    if !has_filter {
        daemon_fail!(
            response,
            "missing required key-value pair 'app[!]=..' or 'title[!]=..'\n"
        );
        did_parse = false;
    }

    if let Some(unsupported_exclusion) = unsupported_exclusion {
        response.fail_pieces(&[
            FailurePiece::Text("unsupported token '!' (exclusion) given for key '"),
            FailurePiece::Bytes(c_string_at(message_cursor.bytes(), unsupported_exclusion)),
            FailurePiece::Text("'\n"),
        ]);
        did_parse = false;
    }

    did_parse
}

pub(crate) fn handle_domain_rule(
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
    let command = message_cursor.get_token();
    if token_equals(command, message_cursor.bytes(), COMMAND_RULE_ADD) {
        let mut rule = Rule::default();

        let mut token = message_cursor.get_token();
        if token_equals(token, message_cursor.bytes(), ARGUMENT_RULE_ONE_SHOT) {
            rule.flags |= RuleFlag::ONE_SHOT.0;
            token = message_cursor.get_token();
        }

        if parse_rule(
            response,
            message_cursor,
            &mut rule,
            token,
            display_manager,
            window_manager,
            space_manager,
        ) {
            rule_add(rule, window_manager);
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_RULE_APPLY) {
        let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
        if let TokenType::Int(int_value) = value.type_of_value {
            if !rule_reapply_by_index(
                int_value,
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            ) {
                daemon_fail!(response, "rule with index '{}' not found.\n", int_value);
            }
        } else if let TokenType::String = value.type_of_value {
            if !rule_reapply_by_label(
                c_string_at(message_cursor.bytes(), value.token.start),
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            ) {
                let mut rule = Rule::default();
                if parse_rule(
                    response,
                    message_cursor,
                    &mut rule,
                    value.token,
                    display_manager,
                    window_manager,
                    space_manager,
                ) {
                    rule_apply(
                        &rule,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    );
                }
            }
        } else if let TokenType::Invalid = value.type_of_value {
            rule_reapply_all(
                process_manager,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
                mission_control_mode,
            );
        } else {
            response.fail_pieces(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(value.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for RULE_SEL\n"),
            ]);
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_RULE_REM) {
        let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
        if let TokenType::Int(int_value) = value.type_of_value {
            if !rule_remove_by_index(int_value, window_manager) {
                daemon_fail!(response, "rule with index '{}' not found.\n", int_value);
            }
        } else if let TokenType::String = value.type_of_value {
            if !rule_remove_by_label(
                c_string_at(message_cursor.bytes(), value.token.start),
                window_manager,
            ) {
                response.fail_pieces(&[
                    FailurePiece::Text("rule with label '"),
                    FailurePiece::Bytes(c_string_at(message_cursor.bytes(), value.token.start)),
                    FailurePiece::Text("' not found.\n"),
                ]);
            }
        } else {
            response.fail_pieces(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(value.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for RULE_SEL\n"),
            ]);
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_RULE_LS) {
        window_manager_query_window_rules(response, display_manager, window_manager);
    } else {
        daemon_fail_with_unknown_command_for_domain(
            response,
            message_cursor.bytes(),
            command,
            domain,
        );
    }
}

pub(crate) fn handle_domain_signal(
    response: &mut Response,
    domain: Token,
    message_cursor: &mut MessageCursor,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) {
    let command = message_cursor.get_token();
    if token_equals(command, message_cursor.bytes(), COMMAND_SIGNAL_ADD) {
        let mut unsupported_exclusion: Option<usize> = None;
        let mut did_parse = true;
        let mut has_command = false;
        let mut has_signal_type = false;
        let mut signal_type = SignalType::Unknown;
        let mut signal = Signal {
            app: None,
            title: None,
            app_regex_exclude: false,
            title_regex_exclude: false,
            app_regex: None,
            title_regex: None,
            active: SignalProp::Undefined,
            command: None,
            label: None,
        };

        let mut token = message_cursor.get_token();
        while token.is_valid() {
            'iteration: {
                let Some(pair) = parse_key_value_pair(message_cursor.bytes_mut(), token.start)
                else {
                    response.fail_pieces(&[
                        FailurePiece::Text("invalid key-value pair '"),
                        FailurePiece::Bytes(c_string_at(message_cursor.bytes(), token.start)),
                        FailurePiece::Text("'\n"),
                    ]);
                    did_parse = false;
                    break 'iteration;
                };

                let key = c_string_at(message_cursor.bytes(), pair.key).to_vec();
                let value = c_string_at(message_cursor.bytes(), pair.value).to_vec();

                if key == ARGUMENT_SIGNAL_KEY_LABEL.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }
                    signal.label = Some(String::from_utf8_lossy(&value).into_owned());
                } else if key == ARGUMENT_SIGNAL_KEY_APP.as_bytes() {
                    signal.app = Some(String::from_utf8_lossy(&value).into_owned());
                    signal.app_regex_exclude = pair.exclusion;
                    signal.app_regex =
                        PosixRegex::compile(&CString::new(value.clone()).unwrap_or_default());
                    if signal.app_regex.is_none() {
                        daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else if key == ARGUMENT_SIGNAL_KEY_TITLE.as_bytes() {
                    signal.title = Some(String::from_utf8_lossy(&value).into_owned());
                    signal.title_regex_exclude = pair.exclusion;
                    signal.title_regex =
                        PosixRegex::compile(&CString::new(value.clone()).unwrap_or_default());
                    if signal.title_regex.is_none() {
                        daemon_fail_with_invalid_regex_pattern_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else if key == ARGUMENT_SIGNAL_KEY_ACTIVE.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }

                    if value == ARGUMENT_SIGNAL_VALUE_YES.as_bytes() {
                        signal.active = SignalProp::Yes;
                    } else if value == ARGUMENT_SIGNAL_VALUE_NO.as_bytes() {
                        signal.active = SignalProp::No;
                    } else {
                        daemon_fail_with_invalid_value_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else if key == ARGUMENT_SIGNAL_KEY_ACTION.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }

                    has_command = true;
                    signal.command = Some(String::from_utf8_lossy(&value).into_owned());
                } else if key == ARGUMENT_SIGNAL_KEY_EVENT.as_bytes() {
                    if pair.exclusion {
                        unsupported_exclusion = Some(pair.key);
                    }

                    has_signal_type = true;
                    signal_type = signal_type_from_string(&value);
                    if signal_type == SignalType::Unknown {
                        daemon_fail_with_invalid_value_for_key(response, &value, &key);
                        did_parse = false;
                    }
                } else {
                    response.fail_pieces(&[
                        FailurePiece::Text("unknown key '"),
                        FailurePiece::Bytes(&key),
                        FailurePiece::Text("'\n"),
                    ]);
                    did_parse = false;
                }
            }

            token = message_cursor.get_token();
        }

        if !has_signal_type {
            daemon_fail!(response, "missing required key-value pair 'event=..'\n");
            did_parse = false;
        }

        if !has_command {
            daemon_fail!(response, "missing required key-value pair 'action=..'\n");
            did_parse = false;
        }

        if let Some(unsupported_exclusion) = unsupported_exclusion {
            response.fail_pieces(&[
                FailurePiece::Text("unsupported token '!' (exclusion) given for key '"),
                FailurePiece::Bytes(c_string_at(message_cursor.bytes(), unsupported_exclusion)),
                FailurePiece::Text("'\n"),
            ]);
            did_parse = false;
        }

        if did_parse {
            event_signal_add(signal_type, signal, signal_event);
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_SIGNAL_REM) {
        let value = token_to_value(message_cursor.get_token(), message_cursor.bytes());
        if let TokenType::Int(int_value) = value.type_of_value {
            if !event_signal_remove_by_index(int_value, signal_event) {
                daemon_fail!(response, "signal with index '{}' not found.\n", int_value);
            }
        } else if let TokenType::String = value.type_of_value {
            if !event_signal_remove(
                c_string_at(message_cursor.bytes(), value.token.start),
                signal_event,
            ) {
                response.fail_pieces(&[
                    FailurePiece::Text("signal with label '"),
                    FailurePiece::Bytes(c_string_at(message_cursor.bytes(), value.token.start)),
                    FailurePiece::Text("' not found.\n"),
                ]);
            }
        } else {
            response.fail_pieces(&[
                FailurePiece::Text("value '"),
                FailurePiece::Bytes(value.token.bytes(message_cursor.bytes())),
                FailurePiece::Text("' is not a valid option for SIGNAL_SEL\n"),
            ]);
        }
    } else if token_equals(command, message_cursor.bytes(), COMMAND_SIGNAL_LS) {
        event_signal_list(response, signal_event);
    } else {
        daemon_fail_with_unknown_command_for_domain(
            response,
            message_cursor.bytes(),
            command,
            domain,
        );
    }
}

pub(crate) fn handle_message(
    response: &mut Response,
    message: &mut [u8],
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let mut message_cursor = MessageCursor::new(message);
    let domain = message_cursor.get_token();
    if token_equals(domain, message_cursor.bytes(), DOMAIN_CONFIG) {
        handle_domain_config(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_DISPLAY) {
        handle_domain_display(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mission_control_mode,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_SPACE) {
        handle_domain_space(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_WINDOW) {
        handle_domain_window(
            response,
            domain,
            &mut message_cursor,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_QUERY) {
        handle_domain_query(
            response,
            domain,
            &mut message_cursor,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_RULE) {
        handle_domain_rule(
            response,
            domain,
            &mut message_cursor,
            process_manager,
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    } else if token_equals(domain, message_cursor.bytes(), DOMAIN_SIGNAL) {
        handle_domain_signal(response, domain, &mut message_cursor, signal_event);
    } else {
        response.fail_pieces(&[
            FailurePiece::Text("unknown domain '"),
            FailurePiece::Bytes(domain.bytes(message_cursor.bytes())),
            FailurePiece::Text("'\n"),
        ]);
    }
}

pub(crate) fn message_loop_run() {
    let message_loop = MESSAGE_LOOP.wait();
    for stream in message_loop.listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };

        event_loop_post(Event::DaemonMessage(stream));
    }
}

pub(crate) fn message_loop_begin(socket_path: &Path) -> bool {
    let mut socket_address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    socket_address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let socket_path_bytes = socket_path.as_os_str().as_bytes();
    let socket_path_length = c_string_at(socket_path_bytes, 0)
        .len()
        .min(socket_address.sun_path.len() - 1);
    for index in 0..socket_path_length {
        socket_address.sun_path[index] = socket_path_bytes[index] as libc::c_char;
    }
    let _ = std::fs::remove_file(socket_path);

    let socket_file_descriptor = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if socket_file_descriptor == -1 {
        return false;
    }
    let socket = unsafe { OwnedFd::from_raw_fd(socket_file_descriptor) };

    if unsafe {
        libc::bind(
            socket_file_descriptor,
            &socket_address as *const libc::sockaddr_un as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        )
    } == -1
    {
        return false;
    }

    if std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600)).is_err() {
        return false;
    }

    if unsafe { libc::listen(socket_file_descriptor, libc::SOMAXCONN) } == -1 {
        return false;
    }

    unsafe {
        libc::fcntl(
            socket_file_descriptor,
            libc::F_SETFD,
            libc::FD_CLOEXEC | libc::fcntl(socket_file_descriptor, libc::F_GETFD),
        )
    };

    let listener = UnixListener::from(socket);
    if let Ok(thread) = std::thread::Builder::new().spawn(message_loop_run) {
        let _ = MESSAGE_LOOP.set(MessageLoop { listener, thread });
    }

    true
}

pub(crate) fn format_float_with_six_decimals_as_printf_does(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    format!("{:.6}", value)
}
