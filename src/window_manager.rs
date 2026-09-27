#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::ffi::CString;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering, compiler_fence};
use std::sync::{Arc, Mutex};

use crate::application::{
    Application, application_create, application_destroy, application_focused_window,
    application_observe, application_unobserve, application_window_list,
};
use crate::display::{display_bounds_constrained, display_space_id, display_space_list};
use crate::display_manager::{
    DisplayManager, display_manager_active_display_list, display_manager_display_is_animating,
};
use crate::event_loop::{Event, event_loop_post};
use crate::ffi::accessibility::{
    _AXUIElementCreateWithRemoteToken, AXUIElement, AXUIElementCopyAttributeValue,
    AXUIElementCreateSystemWide, AXUIElementPerformAction, AXUIElementRef,
    AXUIElementSetAttributeValue, AXUIElementSetMessagingTimeout, AXValueCreate, AXValueType,
    ax_window_id, kAXCloseButtonAttribute, kAXErrorSuccess, kAXFullscreenAttribute,
    kAXMinimizedAttribute, kAXPositionAttribute, kAXPressAction, kAXRaiseAction, kAXRoleAttribute,
    kAXSizeAttribute, kAXWindowRole, with_enhanced_user_interface_disabled,
};
use crate::ffi::carbon_process::{
    CoreDockSendNotification, GetProcessPID, ProcessSerialNumber, psn_equals,
};
use crate::ffi::core_foundation::{
    CFData, CFDataCreateMutable, CFDataGetMutableBytePtr, CFDataIncreaseLength, CFEqual, CFIndex,
    CFMutableData, CFRetained, CFType, CGPoint, CGRect, CGSize, as_cftype,
    cfarray_borrow_value_at_index, cfarray_count, k_com_apple_expose_front_awake, kCFBooleanFalse,
    kCFBooleanTrue, sls_window_disable_shadow, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGAffineTransformConcat, CGAffineTransformMakeScale, CGAffineTransformMakeTranslation,
    CGContextClearRect, CGContextDrawImage, CGContextFlush, CGDisplayBounds, CGImage,
    CGRectContainsPoint, CGRegionCreateEmptyRegion, CGSNewRegionWithRect,
    CGWarpMouseCursorPosition,
};
use crate::ffi::core_video::{
    CVDisplayLink, CVDisplayLinkCreateWithActiveCGDisplays, CVDisplayLinkSetOutputCallback,
    CVDisplayLinkStart, CVDisplayLinkStop, CVOptionFlags, CVReturn, CVTimeStamp, kCVReturnSuccess,
};
use crate::ffi::libsystem::{PROC_PIDPATHINFO_MAXSIZE, proc_name};
use crate::ffi::mach_port::{bootstrap_look_up, mach_send};
use crate::ffi::skylight::{
    _SLPSGetFrontProcess, _SLPSSetFrontProcessWithOptions, SLPSPostEventRecordTo,
    SLSConnectionGetPID, SLSCopyAssociatedWindows, SLSDisableUpdate, SLSFindWindowAndOwner,
    SLSGetCurrentCursorLocation, SLSGetWindowAlpha, SLSGetWindowBounds, SLSHWCaptureWindowList,
    SLSNewConnection, SLSNewWindowWithOpaqueShapeAndContext, SLSReenableUpdate,
    SLSReleaseConnection, SLSReleaseWindow, SLSSetMenuBarInsetAndAlpha, SLSSetWindowAlpha,
    SLSSetWindowLevel, SLSSetWindowOpacity, SLSSetWindowResolution, SLSSetWindowSubLevel,
    SLSSpaceSetFrontPSN, SLSTransactionCommit, SLSTransactionCreate,
    SLSTransactionOrderWindowGroup, SLSTransactionSetWindowAlpha,
    SLSTransactionSetWindowSystemAlpha, SLSTransactionSetWindowTransform, SLSWindowIsOrderedIn,
    SLSWindowIteratorAdvance, SLSWindowIteratorGetParentID, SLSWindowIteratorGetWindowID,
    SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows, SLWindowContextCreate,
};
use crate::globals::{BOOTSTRAP_PORT, CONNECTION, CV_HOST_CLOCK_FREQUENCY};
use crate::handles::{DisplayId, NodeId, ProcessId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::misc::helpers::{
    AnimationEasingType, RgbaColor, cgimage_restore_alpha, clampf_range, rgba_color_from_hex,
    string_copy, string_equals,
};
use crate::misc::log::{g_verbose, or_null};
use crate::misc::macros::{
    DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST, LAYER_AUTO, LAYER_BELOW, LAYER_NORMAL, ResizeHandle,
    TYPE_ABS, TYPE_REL, in_range_ii, lerp, max,
};
use crate::misc::regex::{RegexMatch, regex_match};
use crate::misc::response::Response;
use crate::misc::table::Table;
use crate::mission_control::MissionControlMode;
use crate::mouse_handler::{
    MOUSE_EVENT_MASK, MOUSE_EVENT_MASK_FFM, mouse_handler_begin, mouse_handler_end,
};
use crate::process_manager::{PROCESS_TABLE, Process, ProcessManager};
use crate::rule::{
    RULE_PROP_OFF, RULE_PROP_ON, Rule, RuleEffects, RuleEffectsFlag, RuleFlag,
    rule_combine_effects, rule_serialize,
};
use crate::sa::{
    scripting_addition_order_window, scripting_addition_order_window_in,
    scripting_addition_scale_window, scripting_addition_set_layer, scripting_addition_set_opacity,
    scripting_addition_set_shadow, scripting_addition_set_sticky,
    scripting_addition_swap_window_proxy_in, scripting_addition_swap_window_proxy_out,
};
use crate::space::{
    space_is_fullscreen, space_is_user, space_is_visible, space_window_list,
    space_window_list_for_connection,
};
use crate::space_manager::{
    SpaceManager, space_manager_active_space, space_manager_find_view, space_manager_focus_space,
    space_manager_mark_view_invalid, space_manager_move_window_to_space,
    space_manager_refresh_application_windows, space_manager_refresh_view,
    space_manager_tile_window_on_space, space_manager_tile_window_on_space_with_insertion_point,
    space_manager_untile_window,
};
use crate::state::MouseDragState;
use crate::view::{
    AnimationContext, NODE_MAX_WINDOW_COUNT, ViewFlag, ViewType, WindowAnimation, WindowCapture,
    WindowNodeChild, WindowNodeSplit, WindowProxy, WindowProxyCoreGraphicsObjects, area_make_pair,
    insert_feedback_destroy, insert_feedback_show, view_add_window_node,
    view_add_window_node_with_insertion_point, view_find_window_list, view_find_window_node,
    view_find_window_node_in_direction, view_flush, view_is_dirty, view_remove_window_node,
    view_stack_window_node, view_update, window_node_capture_windows, window_node_contains_window,
    window_node_fence, window_node_find_first_leaf, window_node_find_last_leaf,
    window_node_find_next_leaf, window_node_find_prev_leaf, window_node_flush, window_node_get_gap,
    window_node_get_ratio, window_node_get_split, window_node_is_leaf, window_node_is_left_child,
    window_node_swap_window_list, window_node_update,
};
use crate::window::{
    Window, WindowFlag, WindowRuleFlag, window_ax_frame, window_can_minimize, window_can_move,
    window_can_resize, window_check_flag, window_check_rule_flag, window_clear_flag,
    window_clear_rule_flag, window_create, window_destroy, window_display_id, window_is_fullscreen,
    window_is_real, window_is_standard, window_is_sticky, window_is_undersized, window_is_unknown,
    window_level, window_level_is_standard, window_nonax_serialize, window_observe, window_role_ts,
    window_serialize, window_set_flag, window_set_rule_flag, window_space, window_sub_level,
    window_subrole_ts, window_title_ts, window_unobserve,
};
use crate::workspace::{
    WORKSPACE_CONTEXT, workspace_application_is_observable,
    workspace_application_observe_activation_policy, workspace_is_macos_monterey,
    workspace_is_macos_sequoia, workspace_is_macos_sonoma, workspace_is_macos_tahoe,
    workspace_is_macos_ventura,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowOpError {
    Success,
    InvalidSrcView,
    InvalidSrcNode,
    InvalidDstView,
    InvalidDstNode,
    InvalidOperation,
    SameWindow,
    CantMinimize,
    AlreadyMinimized,
    MinimizeFailed,
    NotMinimized,
    DeminimizeFailed,
    MaxStack,
    SameStack,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum PurifyMode {
    #[default]
    Disabled = 0,
    Managed = 1,
    Always = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum FfmMode {
    #[default]
    Disabled = 0,
    Autofocus = 1,
    Autoraise = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum WindowOriginMode {
    #[default]
    Default = 0,
    Focused = 1,
    Cursor = 2,
}

pub(crate) struct Scratchpad {
    pub(crate) label: String,
    pub(crate) window_id: WindowId,
}

pub(crate) struct WindowManager {
    pub(crate) system_element: AXUIElementRef,
    pub(crate) application: Table<ProcessId, Application>,
    pub(crate) window: Table<WindowId, Window>,
    pub(crate) managed_window: Table<WindowId, SpaceId>,
    pub(crate) window_lost_focused_event: Table<WindowId, ()>,
    pub(crate) application_lost_front_switched_event: Table<ProcessId, ()>,
    pub(crate) window_animations_table: Arc<Mutex<Table<WindowId, (Arc<AnimationContext>, usize)>>>,
    pub(crate) insert_feedback: Table<WindowId, (SpaceId, NodeId)>,
    pub(crate) rules: Vec<Rule>,
    pub(crate) applications_to_refresh: Vec<ProcessId>,
    pub(crate) focused_window_id: WindowId,
    pub(crate) focused_window_process_serial_number: ProcessSerialNumber,
    pub(crate) last_window_id: WindowId,
    pub(crate) enable_mff: bool,
    pub(crate) ffm_mode: FfmMode,
    pub(crate) purify_mode: PurifyMode,
    pub(crate) window_origin_mode: WindowOriginMode,
    pub(crate) enable_window_opacity: bool,
    pub(crate) menubar_opacity: f32,
    pub(crate) active_window_opacity: f32,
    pub(crate) normal_window_opacity: f32,
    pub(crate) window_opacity_duration: f32,
    pub(crate) window_animation_duration: f32,
    pub(crate) window_animation_easing: AnimationEasingType,
    pub(crate) insert_feedback_color: RgbaColor,
    pub(crate) scratchpad_window: Vec<Scratchpad>,
}

#[repr(C)]
pub(crate) struct JankyBordersEvent {
    pub(crate) event: u32,
    pub(crate) count: u32,
    pub(crate) proxy_window_id: [u32; 512],
    pub(crate) real_window_id: [u32; 512],
}

const _: () = assert!(core::mem::size_of::<JankyBordersEvent>() == 4104);

#[allow(non_upper_case_globals)]
pub(crate) const kCPSAllWindows: u32 = 0x100;
#[allow(non_upper_case_globals)]
pub(crate) const kCPSUserGenerated: u32 = 0x200;
#[allow(non_upper_case_globals)]
pub(crate) const kCPSNoWindows: u32 = 0x400;

pub(crate) static PURIFY_MODE_STR: [&str; 3] = ["on", "float", "off"];

pub(crate) static FFM_MODE_STR: [&str; 3] = ["disabled", "autofocus", "autoraise"];

pub(crate) static WINDOW_ORIGIN_MODE_STR: [&str; 3] = ["default", "focused", "cursor"];

pub(crate) fn hash_wm_window_id(key: &WindowId) -> u64 {
    key.0 as u64
}

pub(crate) fn hash_wm_process_id(key: &ProcessId) -> u64 {
    key.0 as u32 as u64
}

pub(crate) fn window_manager_is_window_eligible(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(window) = window_manager.window.find(&window_id) else {
        return false;
    };

    let result = window.is_root
        && (window_is_real(window) || window_check_rule_flag(window, WindowRuleFlag::MANAGED));
    result
}

pub(crate) fn window_manager_query_window_rules(
    response: &mut Response,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    response.write(format_args!("["));
    for index in 0..window_manager.rules.len() as i32 {
        let rule = &window_manager.rules[index as usize];
        rule_serialize(response, rule, index, display_manager);
        if index < window_manager.rules.len() as i32 - 1 {
            response.write(format_args!(","));
        }
    }
    response.write(format_args!("]\n"));
}

pub(crate) fn window_manager_query_windows_for_spaces(
    response: &mut Response,
    space_list: &[SpaceId],
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let window_list =
        space_window_list_for_connection(space_list, 0, true, window_manager).unwrap_or_default();

    response.write(format_args!("["));
    for index in 0..window_list.len() as i32 {
        let window = window_manager_find_window(window_manager, window_list[index as usize]);
        if let Some(window_id) = window {
            window_serialize(
                response,
                window_id,
                flags,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
        } else {
            window_nonax_serialize(
                response,
                window_list[index as usize],
                flags,
                display_manager,
            );
        }
        if index < window_list.len() as i32 - 1 {
            response.write(format_args!(","));
        }
    }
    response.write(format_args!("]\n"));
}

pub(crate) fn window_manager_query_windows_for_display(
    response: &mut Response,
    display_id: DisplayId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let space_list = display_space_list(display_id).unwrap_or_default();
    window_manager_query_windows_for_spaces(
        response,
        &space_list,
        flags,
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    );
}

pub(crate) fn window_manager_query_windows_for_displays(
    response: &mut Response,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let display_list = display_manager_active_display_list();

    let mut space_list: Vec<SpaceId> = Vec::new();

    for index in 0..display_list.len() {
        let Some(list) = display_space_list(display_list[index]) else {
            continue;
        };

        //
        // NOTE(asmvik): display_space_list(..) uses a linear allocator,
        // and so we only need to track the beginning of the first list along
        // with the total number of spaces that have been allocated.
        //

        space_list.extend(list);
    }

    window_manager_query_windows_for_spaces(
        response,
        &space_list,
        flags,
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
    );
}

pub(crate) fn window_manager_rule_matches_window(
    rule: &Rule,
    window_id: WindowId,
    window_title: &str,
    window_role: &str,
    window_subrole: &str,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(application_process_id) = window_manager
        .window
        .find(&window_id)
        .and_then(|window| window.application)
    else {
        return false;
    };
    let Some(application) = window_manager.application.find(&application_process_id) else {
        return false;
    };
    let application_name = CString::new(application.name.as_bytes()).unwrap();

    let regex_match_app = if RuleFlag(rule.flags).contains(RuleFlag::APP_EXCLUDE) {
        RegexMatch::Yes
    } else {
        RegexMatch::No
    };
    if regex_match(rule.app_regex.as_ref(), &application_name) == regex_match_app {
        return false;
    }

    let window_title = CString::new(window_title).unwrap();
    let regex_match_title = if RuleFlag(rule.flags).contains(RuleFlag::TITLE_EXCLUDE) {
        RegexMatch::Yes
    } else {
        RegexMatch::No
    };
    if regex_match(rule.title_regex.as_ref(), &window_title) == regex_match_title {
        return false;
    }

    let window_role = CString::new(window_role).unwrap();
    let regex_match_role = if RuleFlag(rule.flags).contains(RuleFlag::ROLE_EXCLUDE) {
        RegexMatch::Yes
    } else {
        RegexMatch::No
    };
    if regex_match(rule.role_regex.as_ref(), &window_role) == regex_match_role {
        return false;
    }

    let window_subrole = CString::new(window_subrole).unwrap();
    let regex_match_subrole = if RuleFlag(rule.flags).contains(RuleFlag::SUBROLE_EXCLUDE) {
        RegexMatch::Yes
    } else {
        RegexMatch::No
    };
    if regex_match(rule.subrole_regex.as_ref(), &window_subrole) == regex_match_subrole {
        return false;
    }

    true
}

pub(crate) fn window_manager_apply_manage_rule_effects_to_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    effects: &RuleEffects,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if effects.manage == RULE_PROP_ON {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_rule_flag(window, WindowRuleFlag::MANAGED);
        }
        window_manager_make_window_floating(
            space_manager,
            window_manager,
            window_id,
            false,
            true,
            display_manager,
            mouse_drag_state,
        );
    } else if effects.manage == RULE_PROP_OFF {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_clear_rule_flag(window, WindowRuleFlag::MANAGED);
        }
        window_manager_make_window_floating(
            space_manager,
            window_manager,
            window_id,
            true,
            true,
            display_manager,
            mouse_drag_state,
        );
    }
}

pub(crate) fn window_manager_apply_rule_effects_to_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    effects: &RuleEffects,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    if effects.space_id.0 != 0 || effects.display_id.0 != 0 {
        let window_is_in_native_fullscreen = window_manager
            .window
            .find(&window_id)
            .is_some_and(window_is_fullscreen);

        if !window_is_in_native_fullscreen && !space_is_fullscreen(window_space(window_id)) {
            let space_id = if effects.space_id.0 != 0 {
                effects.space_id
            } else {
                display_space_id(effects.display_id)
            };
            window_manager_send_window_to_space(
                space_manager,
                window_manager,
                window_id,
                space_id,
                true,
                process_manager,
                display_manager,
                mouse_drag_state,
            );
            if RuleEffectsFlag(effects.flags).contains(RuleEffectsFlag::FOLLOW_SPACE)
                || effects.fullscreen == RULE_PROP_ON
            {
                space_manager_focus_space(space_id, window_manager, mission_control_mode);
            }
        }
    }

    if effects.sticky == RULE_PROP_ON {
        window_manager_make_window_sticky(
            space_manager,
            window_manager,
            window_id,
            true,
            display_manager,
            mouse_drag_state,
        );
    } else if effects.sticky == RULE_PROP_OFF {
        window_manager_make_window_sticky(
            space_manager,
            window_manager,
            window_id,
            false,
            display_manager,
            mouse_drag_state,
        );
    }

    if effects.mff == RULE_PROP_ON {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_rule_flag(window, WindowRuleFlag::MFF);
            window_set_rule_flag(window, WindowRuleFlag::MFF_VALUE);
        }
    } else if effects.mff == RULE_PROP_OFF {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_rule_flag(window, WindowRuleFlag::MFF);
            window_clear_rule_flag(window, WindowRuleFlag::MFF_VALUE);
        }
    }

    if RuleEffectsFlag(effects.flags).contains(RuleEffectsFlag::LAYER) {
        window_manager_set_window_layer(window_id, effects.layer, window_manager);
    }

    if RuleEffectsFlag(effects.flags).contains(RuleEffectsFlag::OPACITY)
        && in_range_ii(effects.opacity, 0.0f32, 1.0f32)
    {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window.opacity = effects.opacity;
        }
        window_manager_set_opacity(window_manager, window_id, effects.opacity);
    }

    if effects.fullscreen == RULE_PROP_ON {
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            let window_element_ref = window.element_ref;
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window_element_ref,
                    kAXFullscreenAttribute(),
                    as_cftype(kCFBooleanTrue()),
                )
            };
            window_set_rule_flag(window, WindowRuleFlag::FULLSCREEN);
        }
    }

    if let Some(effects_scratchpad) = effects.scratchpad.as_deref() {
        let scratchpad = string_copy(effects_scratchpad);
        window_manager_set_scratchpad_for_window(
            window_manager,
            window_id,
            scratchpad,
            process_manager,
            display_manager,
            space_manager,
            mouse_drag_state,
        );
    }

    if effects.grid[0] != 0 && effects.grid[1] != 0 {
        window_manager_apply_grid(
            space_manager,
            window_manager,
            window_id,
            effects.grid[0],
            effects.grid[1],
            effects.grid[2],
            effects.grid[3],
            effects.grid[4],
            effects.grid[5],
            display_manager,
        );
    }
}

pub(crate) fn window_manager_apply_manage_rules_to_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    window_title: &str,
    window_role: &str,
    window_subrole: &str,
    one_shot_rules: bool,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut matched = false;
    let mut effects = RuleEffects::default();

    let mut rules = std::mem::take(&mut window_manager.rules);

    for index in 0..rules.len() {
        if one_shot_rules || !RuleFlag(rules[index].flags).contains(RuleFlag::ONE_SHOT) {
            if window_manager_rule_matches_window(
                &rules[index],
                window_id,
                window_title,
                window_role,
                window_subrole,
                window_manager,
            ) {
                if rules[index].effects.manage == RULE_PROP_ON {
                    if !RuleFlag(rules[index].flags).contains(RuleFlag::ROLE_VALID)
                        && !string_equals(Some(window_role), Some("AXWindow"))
                    {
                        continue;
                    }
                    if !RuleFlag(rules[index].flags).contains(RuleFlag::SUBROLE_VALID)
                        && !string_equals(Some(window_subrole), Some("AXStandardWindow"))
                    {
                        continue;
                    }
                }

                matched = true;
                rule_combine_effects(&rules[index].effects, &mut effects);

                if RuleFlag(rules[index].flags).contains(RuleFlag::ONE_SHOT) {
                    let mut rule_flags = RuleFlag(rules[index].flags);
                    rule_flags.insert(RuleFlag::ONE_SHOT_REMOVE);
                    rules[index].flags = rule_flags.0;
                }
            }
        }
    }

    window_manager.rules = rules;

    if matched {
        window_manager_apply_manage_rule_effects_to_window(
            space_manager,
            window_manager,
            window_id,
            &effects,
            display_manager,
            mouse_drag_state,
        );
    }
}

pub(crate) fn window_manager_apply_rules_to_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    window_title: &str,
    window_role: &str,
    window_subrole: &str,
    one_shot_rules: bool,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let mut matched = false;
    let mut effects = RuleEffects::default();

    let mut rules = std::mem::take(&mut window_manager.rules);

    for index in 0..rules.len() {
        if one_shot_rules || !RuleFlag(rules[index].flags).contains(RuleFlag::ONE_SHOT) {
            if window_manager_rule_matches_window(
                &rules[index],
                window_id,
                window_title,
                window_role,
                window_subrole,
                window_manager,
            ) {
                let window_is_managed_by_rule = window_manager
                    .window
                    .find(&window_id)
                    .is_some_and(|window| window_check_rule_flag(window, WindowRuleFlag::MANAGED));
                if !window_is_managed_by_rule {
                    if !RuleFlag(rules[index].flags).contains(RuleFlag::ROLE_VALID)
                        && !string_equals(Some(window_role), Some("AXWindow"))
                    {
                        continue;
                    }
                    if !RuleFlag(rules[index].flags).contains(RuleFlag::SUBROLE_VALID)
                        && !string_equals(Some(window_subrole), Some("AXStandardWindow"))
                    {
                        continue;
                    }
                }

                matched = true;
                rule_combine_effects(&rules[index].effects, &mut effects);

                if RuleFlag(rules[index].flags).contains(RuleFlag::ONE_SHOT) {
                    let mut rule_flags = RuleFlag(rules[index].flags);
                    rule_flags.insert(RuleFlag::ONE_SHOT_REMOVE);
                    rules[index].flags = rule_flags.0;
                }
            }
        }
    }

    window_manager.rules = rules;

    if matched {
        window_manager_apply_rule_effects_to_window(
            space_manager,
            window_manager,
            window_id,
            &effects,
            process_manager,
            display_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    }
}

pub(crate) fn window_manager_set_focus_follows_mouse(
    window_manager: &mut WindowManager,
    mode: FfmMode,
) {
    mouse_handler_end();

    if mode == FfmMode::Disabled {
        mouse_handler_begin(MOUSE_EVENT_MASK);
    } else {
        mouse_handler_begin(MOUSE_EVENT_MASK_FFM);
    }

    window_manager.ffm_mode = mode;
}

pub(crate) fn window_manager_set_window_opacity_enabled(
    window_manager: &mut WindowManager,
    enabled: bool,
) {
    window_manager.enable_window_opacity = enabled;
    for window_id in window_manager.window.keys_in_bucket_order() {
        if window_manager_is_window_eligible(window_id, window_manager) {
            let opacity = if enabled {
                match window_manager.window.find(&window_id) {
                    Some(window) => window.opacity,
                    None => continue,
                }
            } else {
                1.0f32
            };
            window_manager_set_opacity(window_manager, window_id, opacity);
        }
    }
}

pub(crate) fn window_manager_center_mouse(window_manager: &mut WindowManager, window_id: WindowId) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };

    if window_check_rule_flag(window, WindowRuleFlag::MFF) {
        if !window_check_rule_flag(window, WindowRuleFlag::MFF_VALUE) {
            return;
        }
    } else {
        if !window_manager.enable_mff {
            return;
        }
    }

    let window_frame = window.frame;

    let mut cursor = CGPoint::new(0.0, 0.0);
    unsafe { SLSGetCurrentCursorLocation(*CONNECTION.get().unwrap_or(&0), &mut cursor) };
    if CGRectContainsPoint(window_frame, cursor) {
        return;
    }

    let display_id = window_display_id(window_id);
    if display_id.0 == 0 {
        return;
    }

    let center = CGPoint::new(
        window_frame.origin.x + window_frame.size.width / 2.0,
        window_frame.origin.y + window_frame.size.height / 2.0,
    );

    let bounds = CGDisplayBounds(display_id.0);
    if !CGRectContainsPoint(bounds, center) {
        return;
    }

    CGWarpMouseCursorPosition(center);
}

pub(crate) fn window_manager_should_manage_window(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(window) = window_manager.window.find(&window_id) else {
        return false;
    };

    if !window.is_root {
        return false;
    }
    if window_check_flag(window, WindowFlag::FLOAT) {
        return false;
    }
    if window_is_sticky(window_id) {
        return false;
    }
    if window_check_flag(window, WindowFlag::MINIMIZE) {
        return false;
    }

    let application_is_hidden = match window.application {
        Some(application_process_id) => {
            match window_manager.application.find(&application_process_id) {
                Some(application) => application.is_hidden,
                None => return false,
            }
        }
        None => return false,
    };
    if application_is_hidden {
        return false;
    }

    (window_is_standard(window) && window_level_is_standard(window) && window_can_move(window))
        || window_check_rule_flag(window, WindowRuleFlag::MANAGED)
}

pub(crate) fn window_manager_find_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) -> Option<SpaceId> {
    window_manager.managed_window.find(&window_id).copied()
}

pub(crate) fn window_manager_remove_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) {
    window_manager.managed_window.remove(&window_id);
}

pub(crate) fn window_manager_add_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) {
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    if view.layout == ViewType::Float {
        return;
    }
    window_manager.managed_window.add(window_id, space_id);
    window_manager_purify_window(window_manager, window_id);
}

pub(crate) fn window_manager_adjust_window_ratio(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    type_of_change: i32,
    ratio: f32,
    space_manager: &mut SpaceManager,
) -> WindowOpError {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return WindowOpError::InvalidSrcView;
    };

    let node_id = view_find_window_node(space_manager, space_id, window_id);
    let parent_node_id = node_id.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let Some(parent_node_id) = parent_node_id else {
        return WindowOpError::InvalidSrcNode;
    };

    match type_of_change {
        TYPE_REL => {
            if let Some(view) = space_manager.view.find_mut(&space_id)
                && let Some(parent_node) = view.find_node_mut(parent_node_id)
            {
                parent_node.ratio = clampf_range(parent_node.ratio + ratio, 0.1f32, 0.9f32);
            }
        }
        TYPE_ABS => {
            if let Some(view) = space_manager.view.find_mut(&space_id)
                && let Some(parent_node) = view.find_node_mut(parent_node_id)
            {
                parent_node.ratio = clampf_range(ratio, 0.1f32, 0.9f32);
            }
        }
        _ => {}
    }

    window_node_update(space_manager, space_id, parent_node_id, window_manager);

    if space_is_visible(space_id) {
        window_node_flush(space_id, parent_node_id, window_manager, space_manager);
    } else if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }

    WindowOpError::Success
}

pub(crate) fn window_manager_move_window_relative(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    type_of_change: i32,
    delta_x: f32,
    delta_y: f32,
) -> WindowOpError {
    let view = window_manager_find_managed_window(window_manager, window_id);
    if view.is_some() {
        return WindowOpError::InvalidSrcView;
    }

    let Some(window) = window_manager.window.find(&window_id) else {
        return WindowOpError::Success;
    };
    let window_frame = window.frame;

    let mut delta_x = delta_x;
    let mut delta_y = delta_y;

    if type_of_change == TYPE_REL {
        delta_x = (delta_x as f64 + window_frame.origin.x) as f32;
        delta_y = (delta_y as f64 + window_frame.origin.y) as f32;
    }

    window_manager_animate_window(
        WindowCapture {
            window_id,
            x: delta_x,
            y: delta_y,
            width: window_frame.size.width as f32,
            height: window_frame.size.height as f32,
        },
        window_manager,
    );
    WindowOpError::Success
}

pub(crate) fn window_manager_resize_window_relative_internal(
    window_id: WindowId,
    frame: CGRect,
    direction: i32,
    delta_x: f32,
    delta_y: f32,
    animate: bool,
    window_manager: &mut WindowManager,
) {
    let x_modifier: i32 = if direction & ResizeHandle::LEFT.0 as i32 != 0 {
        -1
    } else if direction & ResizeHandle::RIGHT.0 as i32 != 0 {
        1
    } else {
        0
    };
    let y_modifier: i32 = if direction & ResizeHandle::TOP.0 as i32 != 0 {
        -1
    } else if direction & ResizeHandle::BOTTOM.0 as i32 != 0 {
        1
    } else {
        0
    };

    let frame_width = max(
        1.0f64,
        frame.size.width + (delta_x * x_modifier as f32) as f64,
    ) as f32;
    let frame_height = max(
        1.0f64,
        frame.size.height + (delta_y * y_modifier as f32) as f64,
    ) as f32;
    let frame_x = if direction & ResizeHandle::LEFT.0 as i32 != 0 {
        (frame.origin.x + frame.size.width - frame_width as f64) as f32
    } else {
        frame.origin.x as f32
    };
    let frame_y = if direction & ResizeHandle::TOP.0 as i32 != 0 {
        (frame.origin.y + frame.size.height - frame_height as f64) as f32
    } else {
        frame.origin.y as f32
    };

    if animate {
        window_manager_animate_window(
            WindowCapture {
                window_id,
                x: frame_x,
                y: frame_y,
                width: frame_width,
                height: frame_height,
            },
            window_manager,
        );
    } else {
        let Some(application_process_id) = window_manager
            .window
            .find(&window_id)
            .and_then(|window| window.application)
        else {
            return;
        };
        let Some(application) = window_manager.application.find(&application_process_id) else {
            return;
        };
        let application_element_ref = application.element_ref;

        with_enhanced_user_interface_disabled(unsafe { &*application_element_ref }, || {
            window_manager_move_window(window_id, frame_x, frame_y, window_manager);
            window_manager_resize_window(window_id, frame_width, frame_height, window_manager);
        });
    }
}

pub(crate) fn window_manager_resize_window_relative(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    direction: i32,
    delta_x: f32,
    delta_y: f32,
    animate: bool,
    display_manager: &mut DisplayManager,
    space_manager: &mut SpaceManager,
) -> WindowOpError {
    let view = window_manager_find_managed_window(window_manager, window_id);
    if let Some(space_id) = view {
        if direction == ResizeHandle::ABS.0 as i32 {
            return WindowOpError::InvalidOperation;
        }

        let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
            return WindowOpError::InvalidSrcNode;
        };

        let mut x_fence: Option<NodeId> = None;
        let mut y_fence: Option<NodeId> = None;

        if direction & ResizeHandle::TOP.0 as i32 != 0 {
            x_fence = window_node_fence(space_id, node_id, DIR_NORTH, space_manager);
        }
        if direction & ResizeHandle::BOTTOM.0 as i32 != 0 {
            x_fence = window_node_fence(space_id, node_id, DIR_SOUTH, space_manager);
        }
        if direction & ResizeHandle::LEFT.0 as i32 != 0 {
            y_fence = window_node_fence(space_id, node_id, DIR_WEST, space_manager);
        }
        if direction & ResizeHandle::RIGHT.0 as i32 != 0 {
            y_fence = window_node_fence(space_id, node_id, DIR_EAST, space_manager);
        }
        if x_fence.is_none() && y_fence.is_none() {
            return WindowOpError::InvalidDstNode;
        }

        if let Some(y_fence) = y_fence
            && let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(y_fence_node) = view.find_node_mut(y_fence)
        {
            let scaled_ratio = y_fence_node.ratio + delta_x / y_fence_node.area.width;
            y_fence_node.ratio = clampf_range(scaled_ratio, 0.1f32, 0.9f32);
        }

        if let Some(x_fence) = x_fence
            && let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(x_fence_node) = view.find_node_mut(x_fence)
        {
            let scaled_ratio = x_fence_node.ratio + delta_y / x_fence_node.area.height;
            x_fence_node.ratio = clampf_range(scaled_ratio, 0.1f32, 0.9f32);
        }

        view_update(space_manager, space_id, display_manager, window_manager);
        view_flush(space_manager, space_id, window_manager);
    } else {
        if direction == ResizeHandle::ABS.0 as i32 {
            if animate {
                let Some(window) = window_manager.window.find(&window_id) else {
                    return WindowOpError::Success;
                };
                let window_frame = window.frame;
                window_manager_animate_window(
                    WindowCapture {
                        window_id,
                        x: window_frame.origin.x as f32,
                        y: window_frame.origin.y as f32,
                        width: delta_x,
                        height: delta_y,
                    },
                    window_manager,
                );
            } else {
                let Some(application_process_id) = window_manager
                    .window
                    .find(&window_id)
                    .and_then(|window| window.application)
                else {
                    return WindowOpError::Success;
                };
                let Some(application) = window_manager.application.find(&application_process_id)
                else {
                    return WindowOpError::Success;
                };
                let application_element_ref = application.element_ref;

                with_enhanced_user_interface_disabled(unsafe { &*application_element_ref }, || {
                    window_manager_resize_window(window_id, delta_x, delta_y, window_manager);
                });
            }
        } else {
            let Some(window) = window_manager.window.find(&window_id) else {
                return WindowOpError::Success;
            };
            let frame = window_ax_frame(window);
            window_manager_resize_window_relative_internal(
                window_id,
                frame,
                direction,
                delta_x,
                delta_y,
                animate,
                window_manager,
            );
        }
    }

    WindowOpError::Success
}

pub(crate) fn window_manager_move_window(
    window_id: WindowId,
    x: f32,
    y: f32,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    let window_element_ref = window.element_ref;

    let mut position = CGPoint::new(x as f64, y as f64);
    let position_ref = unsafe {
        AXValueCreate(
            AXValueType::CGPoint,
            NonNull::from(&mut position).cast::<c_void>(),
        )
    };
    let Some(position_ref) = position_ref else {
        return;
    };

    unsafe {
        AXUIElementSetAttributeValue(
            &*window_element_ref,
            kAXPositionAttribute(),
            as_cftype(&*position_ref),
        )
    };
    drop(position_ref);
}

pub(crate) fn window_manager_resize_window(
    window_id: WindowId,
    width: f32,
    height: f32,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    let window_element_ref = window.element_ref;

    let mut size = CGSize::new(width as f64, height as f64);
    let size_ref = unsafe {
        AXValueCreate(
            AXValueType::CGSize,
            NonNull::from(&mut size).cast::<c_void>(),
        )
    };
    let Some(size_ref) = size_ref else {
        return;
    };

    unsafe {
        AXUIElementSetAttributeValue(
            &*window_element_ref,
            kAXSizeAttribute(),
            as_cftype(&*size_ref),
        )
    };
    drop(size_ref);
}

pub(crate) fn window_manager_notify_jankyborders(
    animation_list: &[WindowAnimation],
    event: u32,
    skip: bool,
    wait: bool,
) {
    let bootstrap_port = *BOOTSTRAP_PORT.get().unwrap_or(&0);
    let mut port: libc::mach_port_t = 0;
    if bootstrap_port != 0
        && unsafe { bootstrap_look_up(bootstrap_port, c"git.felix.jbevent".as_ptr(), &mut port) }
            == libc::KERN_SUCCESS
    {
        let mut data = JankyBordersEvent {
            event,
            count: 0,
            proxy_window_id: [0; 512],
            real_window_id: [0; 512],
        };

        for index in 0..animation_list.len() {
            if skip && animation_list[index].skip.load(Ordering::Relaxed) {
                continue;
            }

            if data.count as usize >= data.proxy_window_id.len() {
                break;
            }

            data.proxy_window_id[data.count as usize] =
                animation_list[index].proxy.id.load(Ordering::Relaxed);
            data.real_window_id[data.count as usize] = animation_list[index].window_id.0;

            data.count += 1;
        }

        mach_send(
            port,
            (&mut data as *mut JankyBordersEvent).cast::<c_void>(),
            core::mem::size_of::<JankyBordersEvent>() as u32,
        );
        if wait {
            unsafe { libc::usleep(20000) };
        }
    }
}

fn load_window_proxy_frame(proxy: &WindowProxy) -> CGRect {
    CGRect::new(
        CGPoint::new(
            f64::from_bits(proxy.frame_origin_x.load(Ordering::Relaxed)),
            f64::from_bits(proxy.frame_origin_y.load(Ordering::Relaxed)),
        ),
        CGSize::new(
            f64::from_bits(proxy.frame_size_width.load(Ordering::Relaxed)),
            f64::from_bits(proxy.frame_size_height.load(Ordering::Relaxed)),
        ),
    )
}

fn store_window_proxy_frame(proxy: &WindowProxy, frame: CGRect) {
    proxy
        .frame_origin_x
        .store(frame.origin.x.to_bits(), Ordering::Relaxed);
    proxy
        .frame_origin_y
        .store(frame.origin.y.to_bits(), Ordering::Relaxed);
    proxy
        .frame_size_width
        .store(frame.size.width.to_bits(), Ordering::Relaxed);
    proxy
        .frame_size_height
        .store(frame.size.height.to_bits(), Ordering::Relaxed);
}

pub(crate) fn window_manager_create_window_proxy(
    animation_connection: i32,
    alpha: f32,
    proxy: &WindowProxy,
) {
    let mut core_graphics_objects_guard = proxy.core_graphics_objects.lock().unwrap();
    let core_graphics_objects = &mut *core_graphics_objects_guard;
    let Some(image) = core_graphics_objects.image.as_deref() else {
        return;
    };

    let mut proxy_frame = load_window_proxy_frame(proxy);
    let mut frame_region: *mut CFType = core::ptr::null_mut();
    unsafe { CGSNewRegionWithRect(&mut proxy_frame, &mut frame_region) };
    let empty_region = unsafe { CGRegionCreateEmptyRegion() };

    let mut tags: u64 = 1u64 << 46;
    let mut proxy_window_id = proxy.id.load(Ordering::Relaxed);
    unsafe {
        SLSNewWindowWithOpaqueShapeAndContext(
            animation_connection,
            2,
            frame_region,
            empty_region,
            13 | (1 << 18),
            &mut tags,
            0.0,
            0.0,
            64,
            &mut proxy_window_id,
            core::ptr::null_mut(),
        )
    };
    proxy.id.store(proxy_window_id, Ordering::Relaxed);
    sls_window_disable_shadow(proxy_window_id);
    unsafe {
        SLSSetWindowOpacity(animation_connection, proxy_window_id, false);
        SLSSetWindowResolution(animation_connection, proxy_window_id, 2.0f32 as f64);
        SLSSetWindowAlpha(animation_connection, proxy_window_id, alpha);
        SLSSetWindowLevel(
            animation_connection,
            proxy_window_id,
            proxy.level.load(Ordering::Relaxed),
        );
        SLSSetWindowSubLevel(
            animation_connection,
            proxy_window_id,
            proxy.sub_level.load(Ordering::Relaxed),
        );
    }
    core_graphics_objects.context = unsafe {
        take_create_rule_result(SLWindowContextCreate(
            animation_connection,
            proxy_window_id,
            core::ptr::null(),
        ))
    };

    let frame = CGRect::new(CGPoint::new(0.0, 0.0), proxy_frame.size);
    CGContextClearRect(core_graphics_objects.context.as_deref(), frame);
    CGContextDrawImage(core_graphics_objects.context.as_deref(), frame, Some(image));
    CGContextFlush(core_graphics_objects.context.as_deref());
    drop(unsafe { take_create_rule_result(frame_region) });
    drop(unsafe { take_create_rule_result(empty_region) });
}

pub(crate) fn window_manager_destroy_window_proxy(animation_connection: i32, proxy: &WindowProxy) {
    let mut core_graphics_objects = proxy.core_graphics_objects.lock().unwrap();

    if let Some(image) = core_graphics_objects.image.take() {
        drop(image);
    }

    if let Some(context) = core_graphics_objects.context.take() {
        drop(context);
    }

    drop(core_graphics_objects);

    let proxy_window_id = proxy.id.load(Ordering::Relaxed);
    if proxy_window_id != 0 {
        unsafe { SLSReleaseWindow(animation_connection, proxy_window_id) };
        proxy.id.store(0, Ordering::Relaxed);
    }
}

pub(crate) fn window_manager_build_window_proxy_thread_proc(window_animation: &WindowAnimation) {
    let mut alpha = 1.0f32;
    unsafe {
        SLSGetWindowAlpha(
            window_animation.connection_id,
            window_animation.window_id.0,
            &mut alpha,
        )
    };
    window_animation
        .proxy
        .level
        .store(window_level(window_animation.window_id), Ordering::Relaxed);
    window_animation.proxy.sub_level.store(
        window_sub_level(window_animation.window_id),
        Ordering::Relaxed,
    );
    let mut proxy_frame = load_window_proxy_frame(&window_animation.proxy);
    unsafe {
        SLSGetWindowBounds(
            window_animation.connection_id,
            window_animation.window_id.0,
            &mut proxy_frame,
        )
    };
    store_window_proxy_frame(&window_animation.proxy, proxy_frame);
    window_animation
        .proxy
        .target_x
        .store((proxy_frame.origin.x as f32).to_bits(), Ordering::Relaxed);
    window_animation
        .proxy
        .target_y
        .store((proxy_frame.origin.y as f32).to_bits(), Ordering::Relaxed);
    window_animation
        .proxy
        .target_width
        .store((proxy_frame.size.width as f32).to_bits(), Ordering::Relaxed);
    window_animation.proxy.target_height.store(
        (proxy_frame.size.height as f32).to_bits(),
        Ordering::Relaxed,
    );

    let mut window_id = window_animation.window_id.0;
    let image_array = unsafe {
        take_create_rule_result(SLSHWCaptureWindowList(
            window_animation.connection_id,
            &mut window_id,
            1,
            (1 << 11) | (1 << 8),
        ))
    };
    if let Some(image_array) = image_array {
        let image = match unsafe { cfarray_borrow_value_at_index::<CGImage>(&image_array, 0) } {
            Some(image) => {
                if alpha == 1.0f32 {
                    Some(unsafe { CFRetained::retain(NonNull::from(image)) })
                } else {
                    cgimage_restore_alpha(image)
                }
            }
            None => None,
        };
        window_animation
            .proxy
            .core_graphics_objects
            .lock()
            .unwrap()
            .image = image;
        drop(image_array);
    } else {
        window_animation
            .proxy
            .core_graphics_objects
            .lock()
            .unwrap()
            .image = None;
    }

    window_manager_create_window_proxy(
        window_animation.connection_id,
        alpha,
        &window_animation.proxy,
    );
}

pub(crate) unsafe extern "C-unwind" fn window_manager_animate_window_list_thread_proc(
    link: NonNull<CVDisplayLink>,
    now: NonNull<CVTimeStamp>,
    output_time: NonNull<CVTimeStamp>,
    flags: CVOptionFlags,
    flags_out: NonNull<CVOptionFlags>,
    data: *mut c_void,
) -> CVReturn {
    let animation_context =
        ManuallyDrop::new(unsafe { Arc::from_raw(data.cast::<AnimationContext>()) });
    let animation_count = animation_context.animation_count;

    let current_clock = unsafe { output_time.as_ref().hostTime };
    if animation_context.animation_clock.load(Ordering::Relaxed) == 0 {
        animation_context
            .animation_clock
            .store(unsafe { now.as_ref().hostTime }, Ordering::Relaxed);
    }

    let mut interpolant = (current_clock
        .wrapping_sub(animation_context.animation_clock.load(Ordering::Relaxed)))
        as f64
        / (animation_context.animation_duration as f64
            * *CV_HOST_CLOCK_FREQUENCY.get().unwrap_or(&0.0));
    if interpolant <= 0.0 {
        interpolant = 0.0f32 as f64;
    }
    if interpolant >= 1.0 {
        interpolant = 1.0f32 as f64;
    }

    let eased_interpolant = animation_context.animation_easing.apply(interpolant as f32);

    let transaction = unsafe { SLSTransactionCreate(animation_context.animation_connection) };
    for index in 0..animation_count as usize {
        let window_animation = &animation_context.animation_list[index];
        if window_animation.skip.load(Ordering::Relaxed) {
            continue;
        }

        let proxy_frame = load_window_proxy_frame(&window_animation.proxy);
        let target_x = lerp(proxy_frame.origin.x, eased_interpolant, window_animation.x) as f32;
        let target_y = lerp(proxy_frame.origin.y, eased_interpolant, window_animation.y) as f32;
        let target_width = lerp(
            proxy_frame.size.width,
            eased_interpolant,
            window_animation.width,
        ) as f32;
        let target_height = lerp(
            proxy_frame.size.height,
            eased_interpolant,
            window_animation.height,
        ) as f32;
        window_animation
            .proxy
            .target_x
            .store(target_x.to_bits(), Ordering::Relaxed);
        window_animation
            .proxy
            .target_y
            .store(target_y.to_bits(), Ordering::Relaxed);
        window_animation
            .proxy
            .target_width
            .store(target_width.to_bits(), Ordering::Relaxed);
        window_animation
            .proxy
            .target_height
            .store(target_height.to_bits(), Ordering::Relaxed);

        let transform = CGAffineTransformMakeTranslation((-target_x) as f64, (-target_y) as f64);
        let scale = CGAffineTransformMakeScale(
            proxy_frame.size.width / target_width as f64,
            proxy_frame.size.height / target_height as f64,
        );
        unsafe {
            SLSTransactionSetWindowTransform(
                transaction,
                window_animation.proxy.id.load(Ordering::Relaxed),
                0,
                0,
                CGAffineTransformConcat(transform, scale),
            )
        };

        let mut alpha = 0.0f32;
        unsafe {
            SLSGetWindowAlpha(
                animation_context.animation_connection,
                window_animation.window_id.0,
                &mut alpha,
            )
        };
        if alpha != 0.0f32 {
            unsafe {
                SLSTransactionSetWindowAlpha(
                    transaction,
                    window_animation.proxy.id.load(Ordering::Relaxed),
                    alpha,
                )
            };
        }
    }
    unsafe { SLSTransactionCommit(transaction, 0) };
    drop(unsafe { take_create_rule_result(transaction) });
    if interpolant != 1.0 {
        return kCVReturnSuccess;
    }

    {
        let mut window_animations_table = animation_context.window_animations_table.lock().unwrap();
        unsafe { SLSDisableUpdate(animation_context.animation_connection) };
        window_manager_notify_jankyborders(
            &animation_context.animation_list[..animation_context.animation_count as usize],
            1326,
            true,
            true,
        );
        scripting_addition_swap_window_proxy_out(
            &animation_context.animation_list[..animation_context.animation_count as usize],
        );
        for index in 0..animation_count as usize {
            if animation_context.animation_list[index]
                .skip
                .load(Ordering::Relaxed)
            {
                continue;
            }

            window_animations_table.remove(&animation_context.animation_list[index].window_id);
            window_manager_destroy_window_proxy(
                animation_context.animation_connection,
                &animation_context.animation_list[index].proxy,
            );
        }
        unsafe { SLSReenableUpdate(animation_context.animation_connection) };
    }

    unsafe { SLSReleaseConnection(animation_context.animation_connection) };
    drop(unsafe { Arc::from_raw(data.cast::<AnimationContext>()) });

    unsafe { CVDisplayLinkStop(link.as_ref()) };
    drop(unsafe { CFRetained::from_raw(link) });

    kCVReturnSuccess
}

pub(crate) fn window_manager_animate_window_list_async(
    window_list: &[WindowCapture],
    window_manager: &mut WindowManager,
) {
    let window_count = window_list.len();

    let mut animation_connection: i32 = 0;
    unsafe { SLSNewConnection(0, &mut animation_connection) };

    let mut animation_list: Vec<WindowAnimation> = Vec::with_capacity(window_count);
    for index in 0..window_count {
        animation_list.push(WindowAnimation {
            window_id: window_list[index].window_id,
            x: window_list[index].x,
            y: window_list[index].y,
            width: window_list[index].width,
            height: window_list[index].height,
            connection_id: animation_connection,
            skip: AtomicBool::new(false),
            proxy: WindowProxy {
                id: AtomicU32::new(0),
                core_graphics_objects: Mutex::new(WindowProxyCoreGraphicsObjects {
                    context: None,
                    image: None,
                }),
                target_x: AtomicU32::new(0),
                target_y: AtomicU32::new(0),
                target_width: AtomicU32::new(0),
                target_height: AtomicU32::new(0),
                frame_origin_x: AtomicU64::new(0),
                frame_origin_y: AtomicU64::new(0),
                frame_size_width: AtomicU64::new(0),
                frame_size_height: AtomicU64::new(0),
                level: AtomicI32::new(0),
                sub_level: AtomicI32::new(0),
            },
        });
    }

    let animation_context = Arc::new(AnimationContext {
        animation_connection,
        animation_count: window_count as i32,
        animation_list,
        animation_duration: window_manager.window_animation_duration,
        animation_easing: window_manager.window_animation_easing,
        animation_clock: AtomicU64::new(0),
        window_animations_table: Arc::clone(&window_manager.window_animations_table),
    });

    let mut builders_to_spawn: Vec<usize> = Vec::new();

    unsafe { SLSDisableUpdate(animation_context.animation_connection) };
    {
        let mut window_animations_table = animation_context.window_animations_table.lock().unwrap();
        for index in 0..window_count {
            let window_animation = &animation_context.animation_list[index];
            let window_id = window_animation.window_id;

            match window_animations_table.remove(&window_id) {
                Some(existing_animation_handle) => {
                    let existing_animation =
                        &existing_animation_handle.0.animation_list[existing_animation_handle.1];
                    existing_animation.skip.store(true, Ordering::Release);

                    let existing_target_x =
                        f32::from_bits(existing_animation.proxy.target_x.load(Ordering::Relaxed));
                    let existing_target_y =
                        f32::from_bits(existing_animation.proxy.target_y.load(Ordering::Relaxed));
                    let existing_target_width = f32::from_bits(
                        existing_animation
                            .proxy
                            .target_width
                            .load(Ordering::Relaxed),
                    );
                    let existing_target_height = f32::from_bits(
                        existing_animation
                            .proxy
                            .target_height
                            .load(Ordering::Relaxed),
                    );

                    store_window_proxy_frame(
                        &window_animation.proxy,
                        CGRect::new(
                            CGPoint::new(
                                existing_target_x as i32 as f64,
                                existing_target_y as i32 as f64,
                            ),
                            CGSize::new(
                                existing_target_width as i32 as f64,
                                existing_target_height as i32 as f64,
                            ),
                        ),
                    );
                    window_animation.proxy.target_x.store(
                        (existing_target_x as i32 as f32).to_bits(),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.target_y.store(
                        (existing_target_y as i32 as f32).to_bits(),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.target_width.store(
                        (existing_target_width as i32 as f32).to_bits(),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.target_height.store(
                        (existing_target_height as i32 as f32).to_bits(),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.level.store(
                        existing_animation.proxy.level.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                    window_animation.proxy.sub_level.store(
                        existing_animation.proxy.sub_level.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                    let existing_image = existing_animation
                        .proxy
                        .core_graphics_objects
                        .lock()
                        .unwrap()
                        .image
                        .clone();
                    window_animation
                        .proxy
                        .core_graphics_objects
                        .lock()
                        .unwrap()
                        .image = existing_image;
                    compiler_fence(Ordering::SeqCst);

                    let mut alpha = 1.0f32;
                    unsafe {
                        SLSGetWindowAlpha(
                            animation_context.animation_connection,
                            window_animation.window_id.0,
                            &mut alpha,
                        )
                    };
                    window_manager_create_window_proxy(
                        animation_context.animation_connection,
                        alpha,
                        &window_animation.proxy,
                    );
                    window_manager_notify_jankyborders(
                        &animation_context.animation_list[index..index + 1],
                        1325,
                        true,
                        false,
                    );
                    window_manager_notify_jankyborders(
                        &existing_animation_handle.0.animation_list
                            [existing_animation_handle.1..existing_animation_handle.1 + 1],
                        1326,
                        false,
                        false,
                    );

                    let transaction =
                        unsafe { SLSTransactionCreate(animation_context.animation_connection) };
                    unsafe {
                        SLSTransactionOrderWindowGroup(
                            transaction,
                            window_animation.proxy.id.load(Ordering::Relaxed),
                            1,
                            window_animation.window_id.0,
                        );
                        SLSTransactionSetWindowSystemAlpha(
                            transaction,
                            existing_animation.proxy.id.load(Ordering::Relaxed),
                            0.0,
                        );
                        SLSTransactionCommit(transaction, 0);
                    }
                    drop(unsafe { take_create_rule_result(transaction) });

                    window_manager_destroy_window_proxy(
                        existing_animation.connection_id,
                        &existing_animation.proxy,
                    );
                    drop(existing_animation_handle);
                }
                None => {
                    builders_to_spawn.push(index);
                }
            }

            window_animations_table.add(window_id, (Arc::clone(&animation_context), index));
        }
    }

    std::thread::scope(|scope| {
        for index in builders_to_spawn.iter().copied() {
            let animation_context_for_builder = Arc::clone(&animation_context);
            let builder_result = std::thread::Builder::new().spawn_scoped(scope, move || {
                window_manager_build_window_proxy_thread_proc(
                    &animation_context_for_builder.animation_list[index],
                );
            });
            if builder_result.is_err() {
                window_manager_build_window_proxy_thread_proc(
                    &animation_context.animation_list[index],
                );
            }
        }
    });

    scripting_addition_swap_window_proxy_in(
        &animation_context.animation_list[..animation_context.animation_count as usize],
    );

    window_manager_notify_jankyborders(
        &animation_context.animation_list[..animation_context.animation_count as usize],
        1325,
        true,
        false,
    );

    for index in 0..window_count {
        let window_animation = &animation_context.animation_list[index];
        let window_id = window_animation.window_id;
        let x = window_animation.x;
        let y = window_animation.y;
        let width = window_animation.width;
        let height = window_animation.height;
        window_manager_set_window_frame(window_id, x, y, width, height, window_manager);
    }

    let mut link: *mut CVDisplayLink = core::ptr::null_mut();
    unsafe { SLSReenableUpdate(animation_context.animation_connection) };
    unsafe { CVDisplayLinkCreateWithActiveCGDisplays(NonNull::from(&mut link)) };
    if let Some(link) = NonNull::new(link) {
        unsafe {
            CVDisplayLinkSetOutputCallback(
                link.as_ref(),
                Some(window_manager_animate_window_list_thread_proc),
                Arc::into_raw(animation_context).cast::<c_void>().cast_mut(),
            );
            CVDisplayLinkStart(link.as_ref());
        }
    }
}

pub(crate) fn window_manager_animate_window_list(
    window_list: &[WindowCapture],
    window_manager: &mut WindowManager,
) {
    if window_manager.window_animation_duration != 0.0f32 {
        window_manager_animate_window_list_async(window_list, window_manager);
    } else {
        for index in 0..window_list.len() {
            window_manager_set_window_frame(
                window_list[index].window_id,
                window_list[index].x,
                window_list[index].y,
                window_list[index].width,
                window_list[index].height,
                window_manager,
            );
        }
    }
}

pub(crate) fn window_manager_animate_window(
    capture: WindowCapture,
    window_manager: &mut WindowManager,
) {
    if window_manager.window_animation_duration != 0.0f32 {
        window_manager_animate_window_list_async(core::slice::from_ref(&capture), window_manager);
    } else {
        window_manager_set_window_frame(
            capture.window_id,
            capture.x,
            capture.y,
            capture.width,
            capture.height,
            window_manager,
        );
    }
}

pub(crate) fn window_manager_set_window_frame(
    window_id: WindowId,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    window_manager: &mut WindowManager,
) {
    //
    // NOTE(asmvik): Attempting to check the window frame cache to prevent unnecessary movement and resize calls to the AX API
    // is not reliable because it is possible to perform operations that should be applied, at a higher rate than the AX API events
    // are received, causing our cache to become out of date and incorrectly guard against some changes that **should** be applied.
    // This causes the window layout to **not** be modified the way we expect.
    //
    // A possible solution is to use the faster CG window notifications, as they are **a lot** more responsive, and can be used to
    // track changes to the window frame in real-time without delay.
    //

    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    let window_element_ref = window.element_ref;
    let Some(application) = window.application.and_then(|application_process_id| {
        window_manager.application.find(&application_process_id)
    }) else {
        return;
    };
    let application_element_ref = application.element_ref;

    with_enhanced_user_interface_disabled(unsafe { &*application_element_ref }, || {
        let mut position = CGPoint::new(x as f64, y as f64);
        let position_ref = unsafe {
            AXValueCreate(
                AXValueType::CGPoint,
                NonNull::from(&mut position).cast::<c_void>(),
            )
        };

        let mut size = CGSize::new(width as f64, height as f64);
        let size_ref = unsafe {
            AXValueCreate(
                AXValueType::CGSize,
                NonNull::from(&mut size).cast::<c_void>(),
            )
        };

        // NOTE(asmvik): Due to macOS constraints (visible screen-area), we might need to resize the window *before* moving it.
        if let Some(size_ref) = &size_ref {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window_element_ref,
                    kAXSizeAttribute(),
                    as_cftype(&**size_ref),
                )
            };
        }

        if let Some(position_ref) = position_ref {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window_element_ref,
                    kAXPositionAttribute(),
                    as_cftype(&*position_ref),
                )
            };
            drop(position_ref);
        }

        // NOTE(asmvik): Due to macOS constraints (visible screen-area), we might need to resize the window *after* moving it.
        if let Some(size_ref) = size_ref {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window_element_ref,
                    kAXSizeAttribute(),
                    as_cftype(&*size_ref),
                )
            };
            drop(size_ref);
        }
    });
}

pub(crate) fn window_manager_set_purify_mode(window_manager: &mut WindowManager, mode: PurifyMode) {
    window_manager.purify_mode = mode;
    for window_id in window_manager.window.keys_in_bucket_order() {
        if window_manager_is_window_eligible(window_id, window_manager) {
            window_manager_purify_window(window_manager, window_id);
        }
    }
}

pub(crate) fn window_manager_set_opacity(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    opacity: f32,
) -> bool {
    let mut opacity = opacity;

    if opacity == 0.0f32 {
        if window_manager.enable_window_opacity {
            opacity = if window_id == window_manager.focused_window_id {
                window_manager.active_window_opacity
            } else {
                window_manager.normal_window_opacity
            };
        } else {
            opacity = 1.0f32;
        }
    }

    scripting_addition_set_opacity(window_id, opacity, window_manager.window_opacity_duration)
}

pub(crate) fn window_manager_set_window_opacity(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    opacity: f32,
) {
    if !window_manager.enable_window_opacity {
        return;
    }
    if !window_manager_is_window_eligible(window_id, window_manager) {
        return;
    }
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    if window.opacity != 0.0f32 {
        return;
    }

    window_manager_set_opacity(window_manager, window_id, opacity);
}

pub(crate) fn window_manager_set_menubar_opacity(window_manager: &mut WindowManager, opacity: f32) {
    window_manager.menubar_opacity = opacity;
    unsafe { SLSSetMenuBarInsetAndAlpha(*CONNECTION.get().unwrap(), 0.0, 1.0, opacity) };
}

pub(crate) fn window_manager_set_active_window_opacity(
    window_manager: &mut WindowManager,
    opacity: f32,
) {
    window_manager.active_window_opacity = opacity;
    let window = window_manager_focused_window(window_manager);
    if let Some(window_id) = window {
        window_manager_set_window_opacity(
            window_manager,
            window_id,
            window_manager.active_window_opacity,
        );
    }
}

pub(crate) fn window_manager_set_normal_window_opacity(
    window_manager: &mut WindowManager,
    opacity: f32,
) {
    window_manager.normal_window_opacity = opacity;
    for window_id in window_manager.window.keys_in_bucket_order() {
        if window_id == window_manager.focused_window_id {
            continue;
        }
        if window_manager_is_window_eligible(window_id, window_manager) {
            window_manager_set_window_opacity(
                window_manager,
                window_id,
                window_manager.normal_window_opacity,
            );
        }
    }
}

pub(crate) fn window_manager_adjust_layer(
    window_id: WindowId,
    layer: i32,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    if window.layer != LAYER_AUTO {
        return;
    }

    scripting_addition_set_layer(window_id, layer);
}

pub(crate) fn window_manager_set_window_layer(
    window_id: WindowId,
    layer: i32,
    window_manager: &mut WindowManager,
) -> bool {
    let mut parent_layer = layer;
    let mut child_layer = layer;

    if layer == LAYER_AUTO {
        parent_layer = if window_manager_find_managed_window(window_manager, window_id).is_some() {
            LAYER_BELOW
        } else {
            LAYER_NORMAL
        };
        child_layer = LAYER_NORMAL;
    }

    let Some(window) = window_manager.window.find_mut(&window_id) else {
        return false;
    };
    window.layer = layer;
    let result = scripting_addition_set_layer(window_id, parent_layer);
    if !result {
        return false;
    }

    let connection = *CONNECTION.get().unwrap();
    let Some(window_list) =
        (unsafe { take_create_rule_result(SLSCopyAssociatedWindows(connection, window_id.0)) })
    else {
        return result;
    };

    let window_count = cfarray_count(&window_list) as i32;
    let Some(query) = (unsafe {
        take_create_rule_result(SLSWindowQueryWindows(
            connection,
            &*window_list,
            window_count,
        ))
    }) else {
        return result;
    };
    let Some(iterator) =
        (unsafe { take_create_rule_result(SLSWindowQueryResultCopyWindows(&*query)) })
    else {
        return result;
    };

    let mut relation_count: i32 = 0;
    let mut parent_list: Vec<u32> = vec![0; window_count as usize];
    let mut child_list: Vec<u32> = vec![0; window_count as usize];

    while unsafe { SLSWindowIteratorAdvance(&*iterator) } {
        let parent_window_id = unsafe { SLSWindowIteratorGetParentID(&*iterator) };
        let child_window_id = unsafe { SLSWindowIteratorGetWindowID(&*iterator) };
        if relation_count < window_count {
            parent_list[relation_count as usize] = parent_window_id;
            child_list[relation_count as usize] = child_window_id;
        }
        relation_count += 1;
    }

    let mut check_list: Vec<u32> = Vec::with_capacity(window_count as usize);
    check_list.push(window_id.0);

    let mut index = 0;
    while index < check_list.len() {
        for inner_index in 0..window_count as usize {
            if parent_list[inner_index] != check_list[index] {
                continue;
            }
            scripting_addition_set_layer(WindowId(child_list[inner_index]), child_layer);
            if check_list.len() < window_count as usize {
                check_list.push(child_list[inner_index]);
            }
        }
        index += 1;
    }

    drop(query);
    drop(iterator);
    drop(window_list);

    result
}

pub(crate) fn window_manager_purify_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) {
    let value: i32;

    if window_manager.purify_mode == PurifyMode::Disabled {
        value = 1;
    } else if window_manager.purify_mode == PurifyMode::Managed {
        value = if window_manager_find_managed_window(window_manager, window_id).is_some() {
            0
        } else {
            1
        };
    } else
    /*if (wm->purify_mode == PURIFY_ALWAYS) */
    {
        value = 0;
    }

    if scripting_addition_set_shadow(window_id, value != 0) {
        let Some(window) = window_manager.window.find_mut(&window_id) else {
            return;
        };
        if value != 0 {
            window_set_flag(window, WindowFlag::SHADOW);
        } else {
            window_clear_flag(window, WindowFlag::SHADOW);
        }
    }
}

pub(crate) fn window_manager_find_rank_of_window_in_list(
    window_id: WindowId,
    window_list: &[WindowId],
) -> i32 {
    let mut rank: i32 = 0;
    for index in 0..window_list.len() {
        if window_list[index] == window_id {
            return rank;
        } else {
            rank += 1;
        }
    }

    i32::MAX
}

pub(crate) fn window_manager_find_window_on_space_by_rank_filtering_window(
    window_manager: &mut WindowManager,
    space_id: SpaceId,
    rank: i32,
    filter_window_id: WindowId,
) -> Option<WindowId> {
    let window_list = space_window_list(space_id, false, window_manager)?;

    let mut result: Option<WindowId> = None;
    let mut inner_index: i32 = 0;
    for index in 0..window_list.len() {
        if window_list[index] == filter_window_id {
            continue;
        }

        let Some(window) = window_manager_find_window(window_manager, window_list[index]) else {
            continue;
        };

        inner_index += 1;
        if inner_index == rank {
            result = Some(window);
            break;
        }
    }

    result
}

pub(crate) fn window_manager_window_connection_is_jankyborders(window_connection_id: i32) -> bool {
    static PROCESS_NAME: Mutex<[u8; PROC_PIDPATHINFO_MAXSIZE]> =
        Mutex::new([0u8; PROC_PIDPATHINFO_MAXSIZE]);
    let mut process_name = PROCESS_NAME.lock().unwrap();

    let mut window_process_id: libc::pid_t = 0;
    unsafe { SLSConnectionGetPID(window_connection_id, &mut window_process_id) };
    unsafe {
        proc_name(
            window_process_id,
            process_name.as_mut_ptr().cast::<c_void>(),
            process_name.len() as u32,
        )
    };

    let end = process_name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(process_name.len());
    &process_name[..end] == b"borders"
}

pub(crate) fn window_manager_find_window_at_point_filtering_window(
    window_manager: &mut WindowManager,
    point: CGPoint,
    filter_window_id: WindowId,
) -> Option<WindowId> {
    let connection = *CONNECTION.get().unwrap();
    let mut point = point;
    let mut window_point = CGPoint::new(0.0, 0.0);
    let mut window_id: u32 = 0;
    let mut window_connection_id: i32 = 0;

    unsafe {
        SLSFindWindowAndOwner(
            connection,
            filter_window_id.0 as i32,
            -1,
            0,
            &mut point,
            &mut window_point,
            &mut window_id,
            &mut window_connection_id,
        )
    };
    if connection == window_connection_id {
        unsafe {
            SLSFindWindowAndOwner(
                connection,
                window_id as i32,
                -1,
                0,
                &mut point,
                &mut window_point,
                &mut window_id,
                &mut window_connection_id,
            )
        };
    }

    if window_manager_window_connection_is_jankyborders(window_connection_id) {
        unsafe {
            SLSFindWindowAndOwner(
                connection,
                window_id as i32,
                -1,
                0,
                &mut point,
                &mut window_point,
                &mut window_id,
                &mut window_connection_id,
            )
        };
        if connection == window_connection_id {
            unsafe {
                SLSFindWindowAndOwner(
                    connection,
                    window_id as i32,
                    -1,
                    0,
                    &mut point,
                    &mut window_point,
                    &mut window_id,
                    &mut window_connection_id,
                )
            };
        }
    }

    window_manager_find_window(window_manager, WindowId(window_id))
}

pub(crate) fn window_manager_find_window_at_point(
    window_manager: &mut WindowManager,
    point: CGPoint,
) -> Option<WindowId> {
    let connection = *CONNECTION.get().unwrap();
    let mut point = point;
    let mut window_point = CGPoint::new(0.0, 0.0);
    let mut window_id: u32 = 0;
    let mut window_connection_id: i32 = 0;

    unsafe {
        SLSFindWindowAndOwner(
            connection,
            0,
            1,
            0,
            &mut point,
            &mut window_point,
            &mut window_id,
            &mut window_connection_id,
        )
    };
    if connection == window_connection_id {
        unsafe {
            SLSFindWindowAndOwner(
                connection,
                window_id as i32,
                -1,
                0,
                &mut point,
                &mut window_point,
                &mut window_id,
                &mut window_connection_id,
            )
        };
    }

    if window_manager_window_connection_is_jankyborders(window_connection_id) {
        unsafe {
            SLSFindWindowAndOwner(
                connection,
                window_id as i32,
                -1,
                0,
                &mut point,
                &mut window_point,
                &mut window_id,
                &mut window_connection_id,
            )
        };
        if connection == window_connection_id {
            unsafe {
                SLSFindWindowAndOwner(
                    connection,
                    window_id as i32,
                    -1,
                    0,
                    &mut point,
                    &mut window_point,
                    &mut window_id,
                    &mut window_connection_id,
                )
            };
        }
    }

    window_manager_find_window(window_manager, WindowId(window_id))
}

pub(crate) fn window_manager_find_window_below_cursor(
    window_manager: &mut WindowManager,
) -> Option<WindowId> {
    let mut cursor = CGPoint::new(0.0, 0.0);
    unsafe { SLSGetCurrentCursorLocation(*CONNECTION.get().unwrap(), &mut cursor) };
    window_manager_find_window_at_point(window_manager, cursor)
}

pub(crate) fn window_manager_find_closest_managed_window_in_direction(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    direction: i32,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return None;
    };

    let Some(closest_node_id) = view_find_window_node_in_direction(
        space_manager,
        space_id,
        node_id,
        direction,
        window_manager,
    ) else {
        return None;
    };

    let closest_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(closest_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, closest_window_id)
}

pub(crate) fn window_manager_find_prev_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return None;
    };

    let Some(previous_node_id) = window_node_find_prev_leaf(space_id, node_id, space_manager)
    else {
        return None;
    };

    let previous_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(previous_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, previous_window_id)
}

pub(crate) fn window_manager_find_next_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return None;
    };

    let Some(next_node_id) = window_node_find_next_leaf(space_id, node_id, space_manager) else {
        return None;
    };

    let next_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(next_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, next_window_id)
}

pub(crate) fn window_manager_find_first_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let first_node_id = window_node_find_first_leaf(space_id, ROOT_NODE_ID, space_manager);

    let first_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(first_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, first_window_id)
}

pub(crate) fn window_manager_find_last_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let last_node_id = window_node_find_last_leaf(space_id, ROOT_NODE_ID, space_manager);

    let last_window_id = space_manager
        .view
        .find(&space_id)?
        .find_node(last_node_id)?
        .window_order[0];
    window_manager_find_window(window_manager, last_window_id)
}

pub(crate) fn window_manager_find_recent_managed_window(
    window_manager: &mut WindowManager,
) -> Option<WindowId> {
    let Some(window_id) = window_manager_find_window(window_manager, window_manager.last_window_id)
    else {
        return None;
    };

    let Some(_space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    Some(window_id)
}

fn window_node_stack_of_window_in_active_view(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<(
    [WindowId; NODE_MAX_WINDOW_COUNT],
    [WindowId; NODE_MAX_WINDOW_COUNT],
    i32,
)> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return None;
    };

    let node = space_manager.view.find(&space_id)?.find_node(node_id)?;
    Some((node.window_list, node.window_order, node.window_count))
}

pub(crate) fn window_manager_find_prev_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    for index in 1..window_count {
        if window_list[index as usize] == window_id {
            return window_manager_find_window(window_manager, window_list[(index - 1) as usize]);
        }
    }

    None
}

pub(crate) fn window_manager_find_next_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    for index in 0..window_count - 1 {
        if window_list[index as usize] == window_id {
            return window_manager_find_window(window_manager, window_list[(index + 1) as usize]);
        }
    }

    None
}

pub(crate) fn window_manager_find_first_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 {
        window_manager_find_window(window_manager, window_list[0])
    } else {
        None
    }
}

pub(crate) fn window_manager_find_last_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 {
        window_manager_find_window(window_manager, window_list[(window_count - 1) as usize])
    } else {
        None
    }
}

pub(crate) fn window_manager_find_recent_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (_window_list, window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 {
        window_manager_find_window(window_manager, window_order[1])
    } else {
        None
    }
}

pub(crate) fn window_manager_find_window_in_stack(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    index: i32,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let (window_list, _window_order, window_count) = window_node_stack_of_window_in_active_view(
        space_manager,
        window_manager,
        window_id,
        display_manager,
    )?;

    if window_count > 1 && in_range_ii(index, 1, window_count) {
        window_manager_find_window(window_manager, window_list[(index - 1) as usize])
    } else {
        None
    }
}

pub(crate) fn window_manager_find_largest_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let mut best_id: u32 = 0;
    let mut best_area: u32 = 0;

    let mut node = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(node_id) = node {
        let (node_area, node_first_window_id) = {
            let leaf_node = space_manager.view.find(&space_id)?.find_node(node_id)?;
            (leaf_node.area, leaf_node.window_order[0])
        };
        let area = (node_area.width * node_area.height) as u32;
        if area > best_area {
            best_id = node_first_window_id.0;
            best_area = area;
        }
        node = window_node_find_next_leaf(space_id, node_id, space_manager);
    }

    if best_id != 0 {
        window_manager_find_window(window_manager, WindowId(best_id))
    } else {
        None
    }
}

pub(crate) fn window_manager_find_smallest_managed_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
) -> Option<WindowId> {
    let space_id = space_manager_find_view(
        space_manager,
        space_manager_active_space(window_manager),
        display_manager,
        window_manager,
    );
    if space_manager.view.find(&space_id).is_none() {
        return None;
    }

    let mut best_id: u32 = 0;
    let mut best_area: u32 = u32::MAX;

    let mut node = Some(window_node_find_first_leaf(
        space_id,
        ROOT_NODE_ID,
        space_manager,
    ));
    while let Some(node_id) = node {
        let (node_area, node_first_window_id) = {
            let leaf_node = space_manager.view.find(&space_id)?.find_node(node_id)?;
            (leaf_node.area, leaf_node.window_order[0])
        };
        let area = (node_area.width * node_area.height) as u32;
        if area <= best_area {
            best_id = node_first_window_id.0;
            best_area = area;
        }
        node = window_node_find_next_leaf(space_id, node_id, space_manager);
    }

    if best_id != 0 {
        window_manager_find_window(window_manager, WindowId(best_id))
    } else {
        None
    }
}

fn window_node_sibling(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
    parent_node_id: NodeId,
) -> Option<NodeId> {
    let node_is_left_child = window_node_is_left_child(space_id, node_id, space_manager);
    let parent_node = space_manager
        .view
        .find(&space_id)?
        .find_node(parent_node_id)?;
    if node_is_left_child {
        parent_node.right
    } else {
        parent_node.left
    }
}

fn window_node_first_window_in_order(
    space_manager: &SpaceManager,
    space_id: SpaceId,
    node_id: NodeId,
) -> Option<WindowId> {
    Some(
        space_manager
            .view
            .find(&space_id)?
            .find_node(node_id)?
            .window_order[0],
    )
}

pub(crate) fn window_manager_find_sibling_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let sibling_node_id = window_node_sibling(space_manager, space_id, node_id, parent_node_id)?;
    if !window_node_is_leaf(space_id, sibling_node_id, space_manager) {
        return None;
    }

    let sibling_window_id =
        window_node_first_window_in_order(space_manager, space_id, sibling_node_id)?;
    window_manager_find_window(window_manager, sibling_window_id)
}

pub(crate) fn window_manager_find_first_nephew_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let sibling_node_id = window_node_sibling(space_manager, space_id, node_id, parent_node_id)?;
    let sibling_left_node_id = space_manager
        .view
        .find(&space_id)?
        .find_node(sibling_node_id)?
        .left;
    if window_node_is_leaf(space_id, sibling_node_id, space_manager)
        || !sibling_left_node_id
            .is_some_and(|left| window_node_is_leaf(space_id, left, space_manager))
    {
        return None;
    }

    let nephew_window_id =
        window_node_first_window_in_order(space_manager, space_id, sibling_left_node_id?)?;
    window_manager_find_window(window_manager, nephew_window_id)
}

pub(crate) fn window_manager_find_second_nephew_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let sibling_node_id = window_node_sibling(space_manager, space_id, node_id, parent_node_id)?;
    let sibling_right_node_id = space_manager
        .view
        .find(&space_id)?
        .find_node(sibling_node_id)?
        .right;
    if window_node_is_leaf(space_id, sibling_node_id, space_manager)
        || !sibling_right_node_id
            .is_some_and(|right| window_node_is_leaf(space_id, right, space_manager))
    {
        return None;
    }

    let nephew_window_id =
        window_node_first_window_in_order(space_manager, space_id, sibling_right_node_id?)?;
    window_manager_find_window(window_manager, nephew_window_id)
}

pub(crate) fn window_manager_find_uncle_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(_node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let grandparent = space_manager
        .view
        .find(&space_id)?
        .find_node(parent_node_id)?
        .parent;
    let Some(grandparent_node_id) = grandparent else {
        return None;
    };

    let uncle_node_id =
        window_node_sibling(space_manager, space_id, parent_node_id, grandparent_node_id)?;
    if !window_node_is_leaf(space_id, uncle_node_id, space_manager) {
        return None;
    }

    let uncle_window_id =
        window_node_first_window_in_order(space_manager, space_id, uncle_node_id)?;
    window_manager_find_window(window_manager, uncle_window_id)
}

pub(crate) fn window_manager_find_first_cousin_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(_node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let grandparent = space_manager
        .view
        .find(&space_id)?
        .find_node(parent_node_id)?
        .parent;
    let Some(grandparent_node_id) = grandparent else {
        return None;
    };

    let uncle_node_id =
        window_node_sibling(space_manager, space_id, parent_node_id, grandparent_node_id)?;
    let uncle_left_node_id = space_manager
        .view
        .find(&space_id)?
        .find_node(uncle_node_id)?
        .left;
    if window_node_is_leaf(space_id, uncle_node_id, space_manager)
        || !uncle_left_node_id
            .is_some_and(|left| window_node_is_leaf(space_id, left, space_manager))
    {
        return None;
    }

    let cousin_window_id =
        window_node_first_window_in_order(space_manager, space_id, uncle_left_node_id?)?;
    window_manager_find_window(window_manager, cousin_window_id)
}

pub(crate) fn window_manager_find_second_cousin_for_managed_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) -> Option<WindowId> {
    let Some(space_id) = window_manager_find_managed_window(window_manager, window_id) else {
        return None;
    };

    let node = view_find_window_node(space_manager, space_id, window_id);
    let parent = node.and_then(|node_id| {
        space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.find_node(node_id))
            .and_then(|node| node.parent)
    });
    let (Some(_node_id), Some(parent_node_id)) = (node, parent) else {
        return None;
    };

    let grandparent = space_manager
        .view
        .find(&space_id)?
        .find_node(parent_node_id)?
        .parent;
    let Some(grandparent_node_id) = grandparent else {
        return None;
    };

    let uncle_node_id =
        window_node_sibling(space_manager, space_id, parent_node_id, grandparent_node_id)?;
    let uncle_right_node_id = space_manager
        .view
        .find(&space_id)?
        .find_node(uncle_node_id)?
        .right;
    if window_node_is_leaf(space_id, uncle_node_id, space_manager)
        || !uncle_right_node_id
            .is_some_and(|right| window_node_is_leaf(space_id, right, space_manager))
    {
        return None;
    }

    let cousin_window_id =
        window_node_first_window_in_order(space_manager, space_id, uncle_right_node_id?)?;
    window_manager_find_window(window_manager, cousin_window_id)
}

pub(crate) fn window_manager_make_key_window(
    window_process_serial_number: &ProcessSerialNumber,
    window_id: WindowId,
) {
    //
    // :SynthesizedEvent
    //
    // NOTE(asmvik): These events will be picked up by an event-tap
    // registered at the "Annotated Session" location; specifying that an
    // event-tap is placed at the point where session events have been
    // annotated to flow to an application.
    //

    let mut window_process_serial_number = *window_process_serial_number;
    let mut event_bytes = [0u8; 0x100];

    event_bytes[..0xf8].fill(0);
    event_bytes[0x04] = 0xf8;
    event_bytes[0x3a] = 0x10;
    event_bytes[0x3c..0x40].copy_from_slice(&window_id.0.to_ne_bytes());
    event_bytes[0x20..0x30].fill(0xff);

    event_bytes[0x08] = 0x01;
    unsafe { SLPSPostEventRecordTo(&mut window_process_serial_number, event_bytes.as_mut_ptr()) };

    event_bytes[0x08] = 0x02;
    unsafe { SLPSPostEventRecordTo(&mut window_process_serial_number, event_bytes.as_mut_ptr()) };
}

pub(crate) fn window_manager_focus_window_without_raise(
    window_process_serial_number: &ProcessSerialNumber,
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    if psn_equals(
        window_process_serial_number,
        &window_manager.focused_window_process_serial_number,
    ) {
        let mut event_bytes = [0u8; 0x100];

        event_bytes[..0xf8].fill(0);
        event_bytes[0x04] = 0xf8;
        event_bytes[0x08] = 0x0d;

        event_bytes[0x8a] = 0x02;
        event_bytes[0x3c..0x40].copy_from_slice(&window_manager.focused_window_id.0.to_ne_bytes());
        unsafe {
            SLPSPostEventRecordTo(
                &mut window_manager.focused_window_process_serial_number,
                event_bytes.as_mut_ptr(),
            )
        };

        //
        // @hack
        // Artificially delay the activation by 40ms. This is necessary
        // because some applications appear to be confused if both of
        // the events appear instantaneously.
        //

        unsafe { libc::usleep(40000) };

        let mut target_process_serial_number = *window_process_serial_number;
        event_bytes[0x8a] = 0x01;
        event_bytes[0x3c..0x40].copy_from_slice(&window_id.0.to_ne_bytes());
        unsafe {
            SLPSPostEventRecordTo(&mut target_process_serial_number, event_bytes.as_mut_ptr())
        };
    }

    let mut target_process_serial_number = *window_process_serial_number;
    unsafe {
        _SLPSSetFrontProcessWithOptions(
            &mut target_process_serial_number,
            window_id.0,
            kCPSUserGenerated,
        )
    };
    window_manager_make_key_window(window_process_serial_number, window_id);
}

pub(crate) fn window_manager_focus_window_with_raise(
    window_process_serial_number: &ProcessSerialNumber,
    window_id: WindowId,
    window_ref: AXUIElementRef,
) {
    let mut target_process_serial_number = *window_process_serial_number;
    unsafe {
        _SLPSSetFrontProcessWithOptions(
            &mut target_process_serial_number,
            window_id.0,
            kCPSUserGenerated,
        )
    };
    window_manager_make_key_window(window_process_serial_number, window_id);
    if let Some(window_element) = unsafe { window_ref.as_ref() } {
        unsafe { AXUIElementPerformAction(window_element, kAXRaiseAction()) };
    }
}

fn window_manager_focus_window_with_raise_resolving_its_application(
    window_manager: &WindowManager,
    window_id: WindowId,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    let Some(application) = window.application.and_then(|application_process_id| {
        window_manager.application.find(&application_process_id)
    }) else {
        return;
    };

    window_manager_focus_window_with_raise(
        &application.process_serial_number,
        window.id,
        window.element_ref,
    );
}

pub(crate) fn window_manager_focused_application(
    window_manager: &mut WindowManager,
) -> Option<ProcessId> {
    let mut process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };
    unsafe { _SLPSGetFrontProcess(&mut process_serial_number) };

    let mut process_id: libc::pid_t = 0;
    unsafe { GetProcessPID(&process_serial_number, &mut process_id) };

    window_manager_find_application(window_manager, ProcessId(process_id))
}

pub(crate) fn window_manager_focused_window(
    window_manager: &mut WindowManager,
) -> Option<WindowId> {
    let Some(application_process_id) = window_manager_focused_application(window_manager) else {
        return None;
    };
    let Some(application) = window_manager.application.find(&application_process_id) else {
        return None;
    };

    let window_id = application_focused_window(application);
    window_manager_find_window(window_manager, window_id)
}

pub(crate) fn window_manager_find_lost_front_switched_event(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) -> bool {
    window_manager
        .application_lost_front_switched_event
        .find(&process_id)
        .is_some()
}

pub(crate) fn window_manager_remove_lost_front_switched_event(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) {
    window_manager
        .application_lost_front_switched_event
        .remove(&process_id);
}

pub(crate) fn window_manager_add_lost_front_switched_event(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) {
    window_manager
        .application_lost_front_switched_event
        .add(process_id, ());
}

pub(crate) fn window_manager_find_lost_focused_event(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) -> bool {
    window_manager
        .window_lost_focused_event
        .find(&window_id)
        .is_some()
}

pub(crate) fn window_manager_remove_lost_focused_event(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) {
    window_manager.window_lost_focused_event.remove(&window_id);
}

pub(crate) fn window_manager_add_lost_focused_event(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) {
    window_manager.window_lost_focused_event.add(window_id, ());
}

pub(crate) fn window_manager_find_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) -> Option<WindowId> {
    window_manager
        .window
        .find(&window_id)
        .map(|window| window.id)
}

pub(crate) fn window_manager_remove_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) -> Option<Window> {
    window_manager.window.remove(&window_id)
}

pub(crate) fn window_manager_add_window(window_manager: &mut WindowManager, window: Window) {
    window_manager.window.add(window.id, window);
}

pub(crate) fn window_manager_find_application(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) -> Option<ProcessId> {
    window_manager
        .application
        .find(&process_id)
        .map(|application| application.process_id)
}

pub(crate) fn window_manager_remove_application(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) -> Option<Application> {
    window_manager.application.remove(&process_id)
}

pub(crate) fn window_manager_add_application(
    window_manager: &mut WindowManager,
    application: Application,
) {
    window_manager
        .application
        .add(application.process_id, application);
}

pub(crate) fn window_manager_find_application_windows(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) -> Vec<WindowId> {
    let mut window_list: Vec<WindowId> = Vec::with_capacity(window_manager.window.len() as usize);

    for window in window_manager.window.values() {
        if window.application == Some(process_id) {
            window_list.push(window.id);
        }
    }

    window_list
}

pub(crate) fn window_manager_create_and_add_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    process_id: ProcessId,
    window_ref: AXUIElementRef,
    window_id: WindowId,
    one_shot_rules: bool,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> Option<WindowId> {
    let mut window = window_create(process_id, window_ref, window_id, window_manager);

    let window_title = window_title_ts(&window);
    let window_role = window_role_ts(&window);
    let window_subrole = window_subrole_ts(&window);
    let application_name = window
        .application
        .and_then(|application_process_id| window_manager.application.find(&application_process_id))
        .map(|application| Arc::clone(&application.name));
    crate::debug!(
        "{}:{} {} - {} ({}:{}:{})\n",
        "window_manager_create_and_add_window",
        window.id.0 as i32,
        or_null(application_name.as_deref()),
        window_title,
        window_role,
        window_subrole,
        window.is_root as i32
    );

    if window_is_unknown(&window) {
        crate::debug!(
            "{}: ignoring AXUnknown window {} {}\n",
            "window_manager_create_and_add_window",
            or_null(application_name.as_deref()),
            window.id.0 as i32
        );
        window_manager_remove_lost_focused_event(window_manager, window.id);
        window_destroy(window);
        return None;
    }

    //
    // NOTE(asmvik): Attempt to track **all** windows.
    //

    if !window_observe(&mut window, window_manager) {
        crate::debug!(
            "{}: could not observe {} {}\n",
            "window_manager_create_and_add_window",
            or_null(application_name.as_deref()),
            window.id.0 as i32
        );
        window_manager_remove_lost_focused_event(window_manager, window.id);
        window_unobserve(&mut window, window_manager);
        window_destroy(window);
        return None;
    }

    if window_manager_find_lost_focused_event(window_manager, window.id) {
        event_loop_post(Event::WindowFocused(window.id));
        window_manager_remove_lost_focused_event(window_manager, window.id);
    }

    let window_id = window.id;
    let window_is_root = window.is_root;
    window_manager_add_window(window_manager, window);

    //
    // NOTE(asmvik): However, only **root windows** are eligible for management.
    //

    if window_is_root {
        //
        // NOTE(asmvik): A lot of windows misreport their accessibility role, so we allow the user
        // to specify rules to make sure that we do in fact manage these windows properly.
        //
        // This part of the rule must be applied at this stage (prior to other rule properties), and if
        // no such rule matches this window, it will be ignored if it does not have a role of kAXWindowRole.
        //

        window_manager_apply_manage_rules_to_window(
            space_manager,
            window_manager,
            window_id,
            &window_title,
            &window_role,
            &window_subrole,
            one_shot_rules,
            display_manager,
            mouse_drag_state,
        );

        if window_manager_is_window_eligible(window_id, window_manager) {
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                window.is_eligible = true;
            }
            window_manager_apply_rules_to_window(
                space_manager,
                window_manager,
                window_id,
                &window_title,
                &window_role,
                &window_subrole,
                one_shot_rules,
                process_manager,
                display_manager,
                mouse_drag_state,
                mission_control_mode,
            );
            window_manager_purify_window(window_manager, window_id);
            window_manager_set_window_opacity(
                window_manager,
                window_id,
                window_manager.normal_window_opacity,
            );

            let application_is_hidden = window_manager
                .application
                .find(&process_id)
                .is_some_and(|application| application.is_hidden);
            if application_is_hidden {
                return Some(window_id);
            }

            let Some(window) = window_manager.window.find_mut(&window_id) else {
                return Some(window_id);
            };
            if window_check_flag(window, WindowFlag::MINIMIZE) {
                return Some(window_id);
            }
            if window_check_flag(window, WindowFlag::FULLSCREEN) {
                return Some(window_id);
            }
            if window_check_rule_flag(window, WindowRuleFlag::MANAGED) {
                return Some(window_id);
            }

            if window_check_rule_flag(window, WindowRuleFlag::FULLSCREEN) {
                window_clear_rule_flag(window, WindowRuleFlag::FULLSCREEN);
                return Some(window_id);
            }

            if window_is_sticky(window.id)
                || !window_can_move(window)
                || !window_is_standard(window)
                || !window_level_is_standard(window)
                || (!window_can_resize(window) && window_is_undersized(window))
            {
                window_set_flag(window, WindowFlag::FLOAT);
            }
        } else {
            crate::debug!(
                "{} ignoring incorrectly marked window {} {}\n",
                "window_manager_create_and_add_window",
                or_null(application_name.as_deref()),
                window_id.0 as i32
            );
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                window_set_flag(window, WindowFlag::FLOAT);
            }

            //
            // NOTE(asmvik): Print window information when debug_output is enabled.
            // Useful for identifying and creating rules if this window should in fact be managed.
            //

            if g_verbose() {
                let mut response = Response::to_standard_output();
                response.write(format_args!("window info: \n"));
                window_serialize(
                    &mut response,
                    window_id,
                    0,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                );
                response.write(format_args!("\n"));
            }
        }
    } else {
        crate::debug!(
            "{} ignoring child window {} {}\n",
            "window_manager_create_and_add_window",
            or_null(application_name.as_deref()),
            window_id.0 as i32
        );
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_flag(window, WindowFlag::FLOAT);
        }

        //
        // NOTE(asmvik): Print window information when debug_output is enabled.
        //

        if g_verbose() {
            let mut response = Response::to_standard_output();
            response.write(format_args!("window info: \n"));
            window_serialize(
                &mut response,
                window_id,
                0,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            );
            response.write(format_args!("\n"));
        }
    }

    Some(window_id)
}

pub(crate) fn window_manager_add_application_windows(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    process_id: ProcessId,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> Vec<WindowId> {
    let Some(application) = window_manager.application.find(&process_id) else {
        return Vec::new();
    };
    let Some(window_list) = application_window_list(application) else {
        return Vec::new();
    };

    let window_count = cfarray_count(&window_list) as i32;
    let mut list: Vec<WindowId> = Vec::with_capacity(window_count as usize);

    for index in 0..window_count {
        let Some(window_ref) = (unsafe {
            cfarray_borrow_value_at_index::<AXUIElement>(&window_list, index as CFIndex)
        }) else {
            continue;
        };

        let window_id = ax_window_id(window_ref);
        if window_id == 0
            || window_manager_find_window(window_manager, WindowId(window_id)).is_some()
        {
            continue;
        }

        let retained_window_ref =
            CFRetained::into_raw(unsafe { CFRetained::retain(NonNull::from(window_ref)) })
                .as_ptr()
                .cast_const();
        let window = window_manager_create_and_add_window(
            space_manager,
            window_manager,
            process_id,
            retained_window_ref,
            WindowId(window_id),
            true,
            process_manager,
            display_manager,
            mouse_drag_state,
            mission_control_mode,
        );
        if let Some(window) = window {
            list.push(window);
        }
    }

    let mut rule_length = window_manager.rules.len() as i32;
    let mut index: i32 = 0;
    while index < rule_length {
        if RuleFlag(window_manager.rules[index as usize].flags).contains(RuleFlag::ONE_SHOT_REMOVE)
        {
            window_manager.rules.swap_remove(index as usize);
            index -= 1;
            rule_length -= 1;
        }
        index += 1;
    }

    drop(window_list);
    list
}

pub(crate) fn window_manager_existing_application_window_list(
    process_id: Option<ProcessId>,
    window_manager: &mut WindowManager,
) -> Option<Vec<WindowId>> {
    let display_list = display_manager_active_display_list();

    let mut space_list: Option<Vec<SpaceId>> = None;

    for index in 0..display_list.len() {
        let Some(list) = display_space_list(display_list[index]) else {
            continue;
        };

        //
        // NOTE(asmvik): display_space_list(..) uses a linear allocator,
        // and so we only need to track the beginning of the first list along
        // with the total number of windows that have been allocated.
        //

        space_list.get_or_insert_with(Vec::new).extend(list);
    }

    let Some(space_list) = space_list else {
        return None;
    };

    let connection_id = match process_id {
        Some(process_id) => match window_manager.application.find(&process_id) {
            Some(application) => application.connection,
            None => return None,
        },
        None => 0,
    };
    space_window_list_for_connection(&space_list, connection_id, true, window_manager)
}

pub(crate) fn window_manager_add_existing_application_windows(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    process_id: ProcessId,
    refresh_index: i32,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) -> bool {
    let mut result = false;

    let Some(global_window_list) =
        window_manager_existing_application_window_list(Some(process_id), window_manager)
    else {
        return result;
    };
    let global_window_count = global_window_list.len() as i32;

    let Some(application) = window_manager.application.find(&process_id) else {
        return result;
    };
    let application_name = Arc::clone(&application.name);
    let application_process_id = application.process_id;
    let window_list_ref = application_window_list(application);
    let window_count = match &window_list_ref {
        Some(window_list_ref) => cfarray_count(window_list_ref) as i32,
        None => 0,
    };

    let mut empty_count = 0;
    if let Some(window_list_ref) = &window_list_ref {
        for index in 0..window_count {
            let window_ref = unsafe {
                cfarray_borrow_value_at_index::<AXUIElement>(window_list_ref, index as CFIndex)
            };
            let window_id = window_ref.map_or(0, |window_ref| ax_window_id(window_ref));

            //
            // @cleanup
            //
            // :Workaround
            //
            // NOTE(asmvik): The AX API appears to always include a single element for Finder that returns an empty window id.
            // This is likely the desktop window. Other similar cases should be handled the same way; simply ignore the window when
            // we attempt to do an equality check to see if we have correctly discovered the number of windows to track.
            //

            if window_id == 0 {
                empty_count += 1;
                continue;
            }

            if window_manager_find_window(window_manager, WindowId(window_id)).is_none()
                && let Some(window_ref) = window_ref
            {
                let retained_window_ref =
                    CFRetained::into_raw(unsafe { CFRetained::retain(NonNull::from(window_ref)) })
                        .as_ptr()
                        .cast_const();
                window_manager_create_and_add_window(
                    space_manager,
                    window_manager,
                    process_id,
                    retained_window_ref,
                    WindowId(window_id),
                    false,
                    process_manager,
                    display_manager,
                    mouse_drag_state,
                    mission_control_mode,
                );
            }
        }
    }

    if global_window_count != window_count - empty_count {
        if refresh_index == -1 {
            let mut missing_window = false;
            let mut application_window_list: Vec<WindowId> = Vec::new();

            for index in 0..global_window_count as usize {
                let window = window_manager_find_window(window_manager, global_window_list[index]);
                if window.is_none() {
                    missing_window = true;
                    application_window_list.push(global_window_list[index]);
                }
            }

            if missing_window {
                crate::debug!(
                    "{}: {} has {} windows that are not yet resolved, attempting workaround\n",
                    "window_manager_add_existing_application_windows",
                    application_name,
                    application_window_list.len() as i32
                );

                //
                // NOTE(asmvik): MacOS API does not return AXUIElementRef of windows on inactive spaces.
                // However, we can just brute-force the element_id and create the AXUIElementRef ourselves.
                //
                // :Attribution
                // https://github.com/decodism
                // https://github.com/lwouis/alt-tab-macos/issues/1324#issuecomment-2631035482
                //

                if let Some(data_ref) = CFDataCreateMutable(None, 0x14) {
                    CFDataIncreaseLength(Some(&*data_ref), 0x14);

                    let data = CFDataGetMutableBytePtr(Some(&*data_ref));
                    unsafe {
                        core::ptr::write_unaligned(
                            data.add(0x0).cast::<u32>(),
                            application_process_id.0 as u32,
                        );
                        core::ptr::write_unaligned(data.add(0x8).cast::<u32>(), 0x636f636f);
                    }

                    for element_id in 0u64..0x7fff {
                        let application_window_list_length = application_window_list.len() as i32;
                        if application_window_list_length == 0 {
                            break;
                        }

                        unsafe {
                            core::ptr::write_unaligned(data.add(0xc).cast::<u64>(), element_id)
                        };
                        let element_ref = unsafe {
                            take_create_rule_result(_AXUIElementCreateWithRemoteToken(
                                (&*data_ref as *const CFMutableData).cast::<CFData>(),
                            ))
                        };

                        let mut role: *const CFType = core::ptr::null();
                        if let Some(element_ref) = &element_ref {
                            unsafe {
                                AXUIElementCopyAttributeValue(
                                    element_ref,
                                    kAXRoleAttribute(),
                                    NonNull::from(&mut role),
                                )
                            };
                        }
                        let role = unsafe { take_create_rule_result(role) };

                        if let Some(role) = role {
                            if CFEqual(Some(&*role), Some(as_cftype(kAXWindowRole()))) {
                                let element_window_id = element_ref
                                    .as_deref()
                                    .map_or(0, |element_ref| ax_window_id(element_ref));
                                let mut matched = false;

                                if element_window_id != 0 {
                                    for inner_index in 0..application_window_list_length as usize {
                                        if application_window_list[inner_index]
                                            == WindowId(element_window_id)
                                        {
                                            matched = true;
                                            application_window_list.swap_remove(inner_index);
                                            break;
                                        }
                                    }
                                }

                                if matched && let Some(element_ref) = element_ref {
                                    window_manager_create_and_add_window(
                                        space_manager,
                                        window_manager,
                                        process_id,
                                        CFRetained::into_raw(element_ref).as_ptr().cast_const(),
                                        WindowId(element_window_id),
                                        false,
                                        process_manager,
                                        display_manager,
                                        mouse_drag_state,
                                        mission_control_mode,
                                    );
                                } else {
                                    drop(element_ref);
                                }
                            }

                            drop(role);
                        }
                    }

                    drop(data_ref);
                }
            }

            if application_window_list.len() > 0 {
                crate::debug!(
                    "{}: workaround failed to resolve all windows for {}\n",
                    "window_manager_add_existing_application_windows",
                    application_name
                );
                window_manager.applications_to_refresh.push(process_id);
            } else {
                crate::debug!(
                    "{}: workaround resolved all windows for {}\n",
                    "window_manager_add_existing_application_windows",
                    application_name
                );
            }
        } else {
            let mut missing_window = false;

            for index in 0..global_window_count as usize {
                let window = window_manager_find_window(window_manager, global_window_list[index]);
                if window.is_none() {
                    missing_window = true;
                    break;
                }
            }

            if !missing_window {
                crate::debug!(
                    "{}: all windows for {} are now resolved\n",
                    "window_manager_add_existing_application_windows",
                    application_name
                );
                window_manager
                    .applications_to_refresh
                    .swap_remove(refresh_index as usize);
                result = true;
            }
        }
    } else if refresh_index != -1 {
        crate::debug!(
            "{}: all windows for {} are now resolved\n",
            "window_manager_add_existing_application_windows",
            application_name
        );
        window_manager
            .applications_to_refresh
            .swap_remove(refresh_index as usize);
        result = true;
    }

    drop(window_list_ref);

    result
}

pub(crate) fn window_manager_set_window_insertion(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    direction: i32,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> WindowOpError {
    let space_id = window_space(window_id);
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return WindowOpError::InvalidSrcView;
    };
    if view.layout != ViewType::Bsp {
        return WindowOpError::InvalidSrcView;
    }

    let Some(node_id) = view_find_window_node(space_manager, space_id, window_id) else {
        return WindowOpError::InvalidSrcNode;
    };

    let insertion_point = space_manager
        .view
        .find(&space_id)
        .map_or(WindowId(0), |view| view.insertion_point);
    if insertion_point.0 != 0 && insertion_point != window_id {
        let insert_node = view_find_window_node(space_manager, space_id, insertion_point);
        if let Some(insert_node_id) = insert_node {
            insert_feedback_destroy(space_id, insert_node_id, window_manager, space_manager);
            if let Some(view) = space_manager.view.find_mut(&space_id)
                && let Some(insert_node) = view.find_node_mut(insert_node_id)
            {
                insert_node.split = WindowNodeSplit::None;
                insert_node.child = WindowNodeChild::None;
                insert_node.insert_direction = 0;
            }
        }
    }

    let Some(node_insert_direction) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
        .map(|node| node.insert_direction)
    else {
        return WindowOpError::InvalidSrcNode;
    };
    if direction == node_insert_direction {
        insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&space_id) {
            if let Some(node) = view.find_node_mut(node_id) {
                node.split = WindowNodeSplit::None;
                node.child = WindowNodeChild::None;
                node.insert_direction = 0;
            }
            view.insertion_point = WindowId(0);
        }
        return WindowOpError::Success;
    }

    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return WindowOpError::InvalidSrcView;
    };
    let Some(node) = view.find_node_mut(node_id) else {
        return WindowOpError::InvalidSrcNode;
    };

    if direction == DIR_NORTH {
        node.split = WindowNodeSplit::X;
        node.child = WindowNodeChild::First;
    } else if direction == DIR_EAST {
        node.split = WindowNodeSplit::Y;
        node.child = WindowNodeChild::Second;
    } else if direction == DIR_SOUTH {
        node.split = WindowNodeSplit::X;
        node.child = WindowNodeChild::Second;
    } else if direction == DIR_WEST {
        node.split = WindowNodeSplit::Y;
        node.child = WindowNodeChild::First;
    }

    node.insert_direction = direction;
    let node_first_window_id = node.window_order[0];
    view.insertion_point = node_first_window_id;
    insert_feedback_show(space_id, node_id, window_manager, space_manager);

    WindowOpError::Success
}

pub(crate) fn window_manager_stack_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    a_window: WindowId,
    b_window: WindowId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) -> WindowOpError {
    if a_window == b_window {
        return WindowOpError::SameWindow;
    }

    let Some(a_view) = window_manager_find_managed_window(window_manager, a_window) else {
        return WindowOpError::InvalidSrcNode;
    };

    let b_view = window_manager_find_managed_window(window_manager, b_window);
    if let Some(b_view) = b_view {
        space_manager_untile_window(
            space_manager,
            b_view,
            b_window,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        window_manager_remove_managed_window(window_manager, b_window);
        window_manager_purify_window(window_manager, b_window);
    } else if window_manager
        .window
        .find(&b_window)
        .is_some_and(|window| window_check_flag(window, WindowFlag::FLOAT))
    {
        if !window_manager_is_window_eligible(b_window, window_manager) {
            return WindowOpError::InvalidSrcNode;
        }
        let Some(window) = window_manager.window.find_mut(&b_window) else {
            return WindowOpError::InvalidSrcNode;
        };
        window_clear_flag(window, WindowFlag::FLOAT);
        if window_check_flag(window, WindowFlag::STICKY) {
            window_manager_make_window_sticky(
                space_manager,
                window_manager,
                b_window,
                false,
                display_manager,
                mouse_drag_state,
            );
        }
    }

    let Some(a_node) = view_find_window_node(space_manager, a_view, a_window) else {
        return WindowOpError::InvalidSrcNode;
    };
    let Some(a_node_window_count) = space_manager
        .view
        .find(&a_view)
        .and_then(|view| view.find_node(a_node))
        .map(|node| node.window_count)
    else {
        return WindowOpError::InvalidSrcNode;
    };
    if a_node_window_count + 1 >= NODE_MAX_WINDOW_COUNT as i32 {
        return WindowOpError::MaxStack;
    }

    view_stack_window_node(a_view, a_node, b_window, space_manager);
    window_manager_add_managed_window(window_manager, b_window, space_manager, a_view);
    window_manager_adjust_layer(b_window, LAYER_BELOW, window_manager);
    let Some(a_node_second_window_in_order) = space_manager
        .view
        .find(&a_view)
        .and_then(|view| view.find_node(a_node))
        .map(|node| node.window_order[1])
    else {
        return WindowOpError::InvalidSrcNode;
    };
    scripting_addition_order_window(b_window, 1, a_node_second_window_in_order);

    let Some(area) = space_manager.view.find(&a_view).and_then(|view| {
        let node = view.find_node(a_node)?;
        match node.zoom {
            Some(zoom) => view.find_node(zoom).map(|zoom_node| zoom_node.area),
            None => Some(node.area),
        }
    }) else {
        return WindowOpError::InvalidSrcNode;
    };
    window_manager_animate_window(
        WindowCapture {
            window_id: b_window,
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        },
        window_manager,
    );
    WindowOpError::Success
}

pub(crate) fn window_manager_warp_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    a_window: WindowId,
    b_window: WindowId,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) -> WindowOpError {
    if a_window == b_window {
        return WindowOpError::SameWindow;
    }

    let a_space_id = window_space(a_window);
    let a_view =
        space_manager_find_view(space_manager, a_space_id, display_manager, window_manager);
    let Some(a_view_layout) = space_manager.view.find(&a_view).map(|view| view.layout) else {
        return WindowOpError::InvalidSrcView;
    };
    if a_view_layout != ViewType::Bsp {
        return WindowOpError::InvalidSrcView;
    }

    let b_space_id = window_space(b_window);
    let b_view =
        space_manager_find_view(space_manager, b_space_id, display_manager, window_manager);
    let Some(b_view_layout) = space_manager.view.find(&b_view).map(|view| view.layout) else {
        return WindowOpError::InvalidDstView;
    };
    if b_view_layout != ViewType::Bsp {
        return WindowOpError::InvalidDstView;
    }

    let Some(a_node) = view_find_window_node(space_manager, a_view, a_window) else {
        return WindowOpError::InvalidSrcNode;
    };

    let Some(b_node) = view_find_window_node(space_manager, b_view, b_window) else {
        return WindowOpError::InvalidDstNode;
    };

    if (a_view, a_node) == (b_view, b_node) {
        return WindowOpError::SameStack;
    }

    let Some((a_node_parent, a_node_window_count)) = space_manager
        .view
        .find(&a_view)
        .and_then(|view| view.find_node(a_node))
        .map(|node| (node.parent, node.window_count))
    else {
        return WindowOpError::InvalidSrcNode;
    };
    let Some(b_node_parent) = space_manager
        .view
        .find(&b_view)
        .and_then(|view| view.find_node(b_node))
        .map(|node| node.parent)
    else {
        return WindowOpError::InvalidDstNode;
    };

    if a_node_parent.is_some()
        && b_node_parent.is_some()
        && (a_view, a_node_parent) == (b_view, b_node_parent)
        && a_node_window_count == 1
    {
        let b_view_insertion_point = space_manager
            .view
            .find(&b_view)
            .map_or(WindowId(0), |view| view.insertion_point);
        if window_node_contains_window(b_view, b_node, b_view_insertion_point, space_manager) {
            if let Some(b_node_parent) = b_node_parent
                && let Some(view) = space_manager.view.find_mut(&b_view)
                && let Some((b_node_split, b_node_child)) =
                    view.find_node(b_node).map(|node| (node.split, node.child))
                && let Some(parent) = view.find_node_mut(b_node_parent)
            {
                parent.split = b_node_split;
                parent.child = b_node_child;
            }

            view_remove_window_node(
                space_manager,
                a_view,
                a_window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_remove_managed_window(window_manager, a_window);
            window_manager_add_managed_window(window_manager, a_window, space_manager, b_view);
            let a_node_add = view_add_window_node_with_insertion_point(
                space_manager,
                b_view,
                a_window,
                b_window,
                display_manager,
                window_manager,
            );

            let mut window_list: Vec<WindowCapture> = Vec::new();
            if let Some(a_node_add) = a_node_add {
                window_node_capture_windows(
                    b_view,
                    a_node_add,
                    &mut window_list,
                    window_manager,
                    space_manager,
                );
            }
            window_manager_animate_window_list(&window_list, window_manager);
        } else {
            let a_view_insertion_point = space_manager
                .view
                .find(&a_view)
                .map_or(WindowId(0), |view| view.insertion_point);
            if window_node_contains_window(a_view, a_node, a_view_insertion_point, space_manager)
                && let Some(view) = space_manager.view.find_mut(&a_view)
            {
                view.insertion_point = b_window;
            }

            window_node_swap_window_list(a_view, a_node, b_view, b_node, space_manager);

            let mut window_list: Vec<WindowCapture> = Vec::new();
            window_node_capture_windows(
                a_view,
                a_node,
                &mut window_list,
                window_manager,
                space_manager,
            );
            window_node_capture_windows(
                b_view,
                b_node,
                &mut window_list,
                window_manager,
                space_manager,
            );
            window_manager_animate_window_list(&window_list, window_manager);
        }
    } else {
        if a_view == b_view {
            //
            // :NaturalWarp
            //
            // NOTE(asmvik): Precalculate both target areas and select the one that has the closest distance to the source area.
            // This allows the warp to feel more natural in terms of where the window is placed on screen, however, this is only utilized
            // for warp operations where both operands belong to the same space. There may be a better system to handle this if/when multiple
            // monitors should be supported.
            //

            let Some(b_node_area) = space_manager
                .view
                .find(&b_view)
                .and_then(|view| view.find_node(b_node))
                .map(|node| node.area)
            else {
                return WindowOpError::InvalidDstNode;
            };
            let (candidate_first_child_area, candidate_second_child_area) = area_make_pair(
                window_node_get_split(space_manager, b_view, b_node),
                window_node_get_gap(space_manager, b_view),
                window_node_get_ratio(b_view, b_node, space_manager),
                b_node_area,
            );

            let Some(a_node_area) = space_manager
                .view
                .find(&a_view)
                .and_then(|view| view.find_node(a_node))
                .map(|node| node.area)
            else {
                return WindowOpError::InvalidSrcNode;
            };
            let source_node_center_point = CGPoint::new(
                (0.5f32 + a_node_area.x + a_node_area.width / 2.0f32) as i32 as f64,
                (0.5f32 + a_node_area.y + a_node_area.height / 2.0f32) as i32 as f64,
            );
            let distance_to_candidate_first_child = ((source_node_center_point.x
                - ((0.5f32
                    + candidate_first_child_area.x
                    + candidate_first_child_area.width / 2.0f32) as i32) as f64)
                as f32)
                .powf(2.0f32)
                + ((source_node_center_point.y
                    - ((0.5f32
                        + candidate_first_child_area.y
                        + candidate_first_child_area.height / 2.0f32) as i32)
                        as f64) as f32)
                    .powf(2.0f32);
            let distance_to_candidate_second_child = ((source_node_center_point.x
                - ((0.5f32
                    + candidate_second_child_area.x
                    + candidate_second_child_area.width / 2.0f32) as i32) as f64)
                as f32)
                .powf(2.0f32)
                + ((source_node_center_point.y
                    - ((0.5f32
                        + candidate_second_child_area.y
                        + candidate_second_child_area.height / 2.0f32)
                        as i32) as f64) as f32)
                    .powf(2.0f32);

            let b_node_child =
                if distance_to_candidate_first_child < distance_to_candidate_second_child {
                    WindowNodeChild::First
                } else if distance_to_candidate_first_child > distance_to_candidate_second_child {
                    WindowNodeChild::Second
                } else if window_node_is_left_child(a_view, a_node, space_manager) {
                    WindowNodeChild::First
                } else {
                    WindowNodeChild::Second
                };
            if let Some(view) = space_manager.view.find_mut(&b_view)
                && let Some(node) = view.find_node_mut(b_node)
            {
                node.child = b_node_child;
            }

            let a_node_remove = view_remove_window_node(
                space_manager,
                a_view,
                a_window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            let a_node_add = view_add_window_node_with_insertion_point(
                space_manager,
                b_view,
                a_window,
                b_window,
                display_manager,
                window_manager,
            );

            let mut window_list: Vec<WindowCapture> = Vec::new();
            if let Some(a_node_remove) = a_node_remove {
                window_node_capture_windows(
                    a_view,
                    a_node_remove,
                    &mut window_list,
                    window_manager,
                    space_manager,
                );
            }

            if let Some(a_node_add) = a_node_add {
                let a_node_add_parent = space_manager
                    .view
                    .find(&b_view)
                    .and_then(|view| view.find_node(a_node_add))
                    .and_then(|node| node.parent);
                if a_node_remove != Some(a_node_add) && a_node_remove != a_node_add_parent {
                    window_node_capture_windows(
                        b_view,
                        a_node_add,
                        &mut window_list,
                        window_manager,
                        space_manager,
                    );
                }
            }

            window_manager_animate_window_list(&window_list, window_manager);
        } else {
            if window_manager.focused_window_id == a_window {
                let next = window_manager_find_window_on_space_by_rank_filtering_window(
                    window_manager,
                    a_view,
                    1,
                    a_window,
                );
                if let Some(next) = next {
                    window_manager_focus_window_with_raise_resolving_its_application(
                        window_manager,
                        next,
                    );
                } else {
                    unsafe {
                        _SLPSSetFrontProcessWithOptions(
                            &mut process_manager.finder_process_serial_number,
                            0,
                            kCPSNoWindows,
                        )
                    };
                }
            }

            //
            // :NaturalWarp
            //
            // TODO(asmvik): Warp operations with operands that belong to different monitors does not yet implement a heuristic to select
            // the target area that feels the most natural in terms of where the window is placed on screen. Is it possible to do better when
            // warping between spaces that belong to the same monitor as well??
            //

            space_manager_untile_window(
                space_manager,
                a_view,
                a_window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_remove_managed_window(window_manager, a_window);
            window_manager_add_managed_window(window_manager, a_window, space_manager, b_view);
            space_manager_move_window_to_space(b_view, a_window);
            space_manager_tile_window_on_space_with_insertion_point(
                space_manager,
                a_window,
                b_view,
                b_window,
                display_manager,
                window_manager,
            );
        }
    }

    WindowOpError::Success
}

pub(crate) fn window_manager_swap_window(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    a_window: WindowId,
    b_window: WindowId,
    display_manager: &mut DisplayManager,
) -> WindowOpError {
    if a_window == b_window {
        return WindowOpError::SameWindow;
    }

    let a_space_id = window_space(a_window);
    let a_view =
        space_manager_find_view(space_manager, a_space_id, display_manager, window_manager);

    let b_space_id = window_space(b_window);
    let b_view =
        space_manager_find_view(space_manager, b_space_id, display_manager, window_manager);

    let Some(a_node) = view_find_window_node(space_manager, a_view, a_window) else {
        return WindowOpError::InvalidSrcNode;
    };

    let Some(b_node) = view_find_window_node(space_manager, b_view, b_window) else {
        return WindowOpError::InvalidDstNode;
    };

    if (a_view, a_node) == (b_view, b_node) {
        let mut a_list_index = 0;
        let mut a_order_index = 0;

        let mut b_list_index = 0;
        let mut b_order_index = 0;

        let Some(view) = space_manager.view.find_mut(&a_view) else {
            return WindowOpError::InvalidSrcNode;
        };
        let Some(node) = view.find_node_mut(a_node) else {
            return WindowOpError::InvalidSrcNode;
        };

        for index in 0..node.window_count as usize {
            if node.window_list[index] == a_window {
                a_list_index = index;
            } else if node.window_list[index] == b_window {
                b_list_index = index;
            }

            if node.window_order[index] == a_window {
                a_order_index = index;
            } else if node.window_order[index] == b_window {
                b_order_index = index;
            }
        }

        node.window_list[a_list_index] = b_window;
        node.window_order[a_order_index] = b_window;

        node.window_list[b_list_index] = a_window;
        node.window_order[b_order_index] = a_window;

        if a_window == window_manager.focused_window_id {
            window_manager_focus_window_with_raise_resolving_its_application(
                window_manager,
                b_window,
            );
        } else if b_window == window_manager.focused_window_id {
            window_manager_focus_window_with_raise_resolving_its_application(
                window_manager,
                a_window,
            );
        }

        return WindowOpError::Success;
    }

    let Some(a_view_layout) = space_manager.view.find(&a_view).map(|view| view.layout) else {
        return WindowOpError::InvalidSrcView;
    };
    if a_view_layout != ViewType::Bsp {
        return WindowOpError::InvalidSrcView;
    }
    let Some(b_view_layout) = space_manager.view.find(&b_view).map(|view| view.layout) else {
        return WindowOpError::InvalidDstView;
    };
    if b_view_layout != ViewType::Bsp {
        return WindowOpError::InvalidDstView;
    }

    let a_view_insertion_point = space_manager
        .view
        .find(&a_view)
        .map_or(WindowId(0), |view| view.insertion_point);
    let b_view_insertion_point = space_manager
        .view
        .find(&b_view)
        .map_or(WindowId(0), |view| view.insertion_point);
    if window_node_contains_window(a_view, a_node, a_view_insertion_point, space_manager) {
        if let Some(view) = space_manager.view.find_mut(&a_view) {
            view.insertion_point = b_window;
        }
    } else if window_node_contains_window(b_view, b_node, b_view_insertion_point, space_manager)
        && let Some(view) = space_manager.view.find_mut(&b_view)
    {
        view.insertion_point = a_window;
    }

    let a_visible = space_is_visible(a_view);
    let b_visible = space_is_visible(b_view);

    if a_view != b_view {
        let Some((a_node_window_list, a_node_window_count)) = space_manager
            .view
            .find(&a_view)
            .and_then(|view| view.find_node(a_node))
            .map(|node| (node.window_list, node.window_count))
        else {
            return WindowOpError::InvalidSrcNode;
        };
        for index in 0..a_node_window_count as usize {
            let window = window_manager_find_window(window_manager, a_node_window_list[index]);
            window_manager_remove_managed_window(window_manager, a_node_window_list[index]);
            if let Some(window) = window {
                space_manager_move_window_to_space(b_view, window);
                window_manager_add_managed_window(window_manager, window, space_manager, b_view);
            }
        }

        let Some((b_node_window_list, b_node_window_count)) = space_manager
            .view
            .find(&b_view)
            .and_then(|view| view.find_node(b_node))
            .map(|node| (node.window_list, node.window_count))
        else {
            return WindowOpError::InvalidDstNode;
        };
        for index in 0..b_node_window_count as usize {
            let window = window_manager_find_window(window_manager, b_node_window_list[index]);
            window_manager_remove_managed_window(window_manager, b_node_window_list[index]);
            if let Some(window) = window {
                space_manager_move_window_to_space(a_view, window);
                window_manager_add_managed_window(window_manager, window, space_manager, a_view);
            }
        }

        if a_visible && !b_visible && a_window == window_manager.focused_window_id {
            window_manager_focus_window_with_raise_resolving_its_application(
                window_manager,
                b_window,
            );
        } else if b_visible && !a_visible && b_window == window_manager.focused_window_id {
            window_manager_focus_window_with_raise_resolving_its_application(
                window_manager,
                a_window,
            );
        }
    }

    window_node_swap_window_list(a_view, a_node, b_view, b_node, space_manager);
    let mut window_list: Vec<WindowCapture> = Vec::new();

    if a_visible {
        window_node_capture_windows(
            a_view,
            a_node,
            &mut window_list,
            window_manager,
            space_manager,
        );
    } else if let Some(view) = space_manager.view.find_mut(&a_view) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }

    if b_visible {
        window_node_capture_windows(
            b_view,
            b_node,
            &mut window_list,
            window_manager,
            space_manager,
        );
    } else if let Some(view) = space_manager.view.find_mut(&b_view) {
        view.set_flag(ViewFlag::IS_DIRTY);
    }

    window_manager_animate_window_list(&window_list, window_manager);
    WindowOpError::Success
}

pub(crate) fn window_manager_minimize_window(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> WindowOpError {
    let Some(window) = window_manager.window.find(&window_id) else {
        return WindowOpError::CantMinimize;
    };

    if !window_can_minimize(window) {
        return WindowOpError::CantMinimize;
    }
    if window_check_flag(window, WindowFlag::MINIMIZE) {
        return WindowOpError::AlreadyMinimized;
    }

    let result = unsafe {
        AXUIElementSetAttributeValue(
            &*window.element_ref,
            kAXMinimizedAttribute(),
            as_cftype(kCFBooleanTrue()),
        )
    };
    if result == kAXErrorSuccess {
        WindowOpError::Success
    } else {
        WindowOpError::MinimizeFailed
    }
}

pub(crate) fn window_manager_deminimize_window(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> WindowOpError {
    let Some(window) = window_manager.window.find(&window_id) else {
        return WindowOpError::NotMinimized;
    };

    if !window_check_flag(window, WindowFlag::MINIMIZE) {
        return WindowOpError::NotMinimized;
    }

    let result = unsafe {
        AXUIElementSetAttributeValue(
            &*window.element_ref,
            kAXMinimizedAttribute(),
            as_cftype(kCFBooleanFalse()),
        )
    };
    if result == kAXErrorSuccess {
        WindowOpError::Success
    } else {
        WindowOpError::DeminimizeFailed
    }
}

pub(crate) fn window_manager_close_window(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(window) = window_manager.window.find(&window_id) else {
        return false;
    };

    let mut button: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXCloseButtonAttribute(),
            NonNull::from(&mut button),
        )
    };
    let Some(button) = (unsafe { take_create_rule_result(button) }) else {
        return false;
    };

    unsafe {
        AXUIElementPerformAction(
            &*(&*button as *const CFType).cast::<AXUIElement>(),
            kAXPressAction(),
        )
    };
    drop(button);

    true
}

pub(crate) fn window_manager_send_window_to_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    destination_space_id: SpaceId,
    moved_by_rule: bool,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let source_space_id = window_space(window_id);
    if source_space_id == destination_space_id {
        return;
    }

    if space_is_visible(source_space_id)
        && (moved_by_rule || window_manager.focused_window_id == window_id)
    {
        let next = window_manager_find_window_on_space_by_rank_filtering_window(
            window_manager,
            source_space_id,
            1,
            window_id,
        );
        if let Some(next) = next {
            window_manager_focus_window_with_raise_resolving_its_application(window_manager, next);
        } else {
            unsafe {
                _SLPSSetFrontProcessWithOptions(
                    &mut process_manager.finder_process_serial_number,
                    0,
                    kCPSNoWindows,
                )
            };
        }
    }

    let view = window_manager_find_managed_window(window_manager, window_id);
    if let Some(space_id) = view {
        space_manager_untile_window(
            space_manager,
            space_id,
            window_id,
            display_manager,
            window_manager,
            mouse_drag_state,
        );
        window_manager_remove_managed_window(window_manager, window_id);
        window_manager_purify_window(window_manager, window_id);
    }

    space_manager_move_window_to_space(destination_space_id, window_id);
    if let Some(application) = window_manager
        .window
        .find(&window_id)
        .and_then(|window| window.application)
        .and_then(|application_process_id| window_manager.application.find(&application_process_id))
    {
        unsafe {
            SLSSpaceSetFrontPSN(
                *CONNECTION.get().unwrap(),
                destination_space_id.0,
                application.process_serial_number,
            )
        };
    }

    if window_manager_should_manage_window(window_id, window_manager) {
        let view = space_manager_tile_window_on_space(
            space_manager,
            window_id,
            destination_space_id,
            display_manager,
            window_manager,
        );
        window_manager_add_managed_window(window_manager, window_id, space_manager, view);
    }
}

pub(crate) fn window_manager_apply_grid(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    rows: u32,
    columns: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    display_manager: &mut DisplayManager,
) -> WindowOpError {
    let mut x = x;
    let mut y = y;
    let mut width = width;
    let mut height = height;

    let view = window_manager_find_managed_window(window_manager, window_id);
    if view.is_some() {
        return WindowOpError::InvalidSrcView;
    }

    let display_id = window_display_id(window_id);
    if display_id.0 == 0 {
        return WindowOpError::InvalidSrcView;
    }

    if x >= columns {
        x = columns.wrapping_sub(1);
    }
    if y >= rows {
        y = rows.wrapping_sub(1);
    }
    if width <= 0 {
        width = 1;
    }
    if height <= 0 {
        height = 1;
    }
    if width > columns.wrapping_sub(x) {
        width = columns.wrapping_sub(x);
    }
    if height > rows.wrapping_sub(y) {
        height = rows.wrapping_sub(y);
    }

    let mut bounds = display_bounds_constrained(display_id, false, display_manager);
    let display_view = space_manager_find_view(
        space_manager,
        display_space_id(display_id),
        display_manager,
        window_manager,
    );

    if let Some(view) = space_manager.view.find(&display_view) {
        let enable_gap = view.check_flag(ViewFlag::ENABLE_GAP);

        if view.check_flag(ViewFlag::ENABLE_PADDING) {
            bounds.origin.x += view.left_padding as f64;
            bounds.size.width -= (view.left_padding + view.right_padding) as f64;
            bounds.origin.y += view.top_padding as f64;
            bounds.size.height -= (view.top_padding + view.bottom_padding) as f64;
        }

        if enable_gap {
            let gap = window_node_get_gap(space_manager, display_view);

            if x > 0 {
                bounds.origin.x += gap as f64;
                bounds.size.width -= gap as f64;
            }

            if y > 0 {
                bounds.origin.y += gap as f64;
                bounds.size.height -= gap as f64;
            }

            if columns > x.wrapping_add(width) {
                bounds.size.width -= gap as f64;
            }
            if rows > y.wrapping_add(height) {
                bounds.size.height -= gap as f64;
            }
        }
    }

    let column_width = (bounds.size.width / columns as f64) as f32;
    let row_height = (bounds.size.height / rows as f64) as f32;
    let frame_x = (bounds.origin.x + bounds.size.width
        - (column_width * columns.wrapping_sub(x) as f32) as f64) as f32;
    let frame_y = (bounds.origin.y + bounds.size.height
        - (row_height * rows.wrapping_sub(y) as f32) as f64) as f32;
    let frame_width = column_width * width as f32;
    let frame_height = row_height * height as f32;

    window_manager_animate_window(
        WindowCapture {
            window_id,
            x: frame_x,
            y: frame_y,
            width: frame_width,
            height: frame_height,
        },
        window_manager,
    );
    WindowOpError::Success
}

pub(crate) fn window_manager_make_window_floating(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    should_float: bool,
    force: bool,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if !window_manager_is_window_eligible(window_id, window_manager) {
        return;
    }

    if !force {
        let Some(window) = window_manager.window.find(&window_id) else {
            return;
        };
        if !window_is_standard(window)
            || !window_level_is_standard(window)
            || !window_can_move(window)
        {
            if !window_check_rule_flag(window, WindowRuleFlag::MANAGED) {
                return;
            }
        }
    }

    if should_float {
        let view = window_manager_find_managed_window(window_manager, window_id);
        if let Some(space_id) = view {
            space_manager_untile_window(
                space_manager,
                space_id,
                window_id,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_remove_managed_window(window_manager, window_id);
            window_manager_purify_window(window_manager, window_id);
        }
        if let Some(window) = window_manager.window.find_mut(&window_id) {
            window_set_flag(window, WindowFlag::FLOAT);
        }
    } else {
        let Some(window) = window_manager.window.find_mut(&window_id) else {
            return;
        };
        window_clear_flag(window, WindowFlag::FLOAT);

        if !window_check_flag(window, WindowFlag::STICKY) {
            if (window_manager_should_manage_window(window_id, window_manager))
                && (window_manager_find_managed_window(window_manager, window_id).is_none())
            {
                let view = space_manager_tile_window_on_space(
                    space_manager,
                    window_id,
                    space_manager_active_space(window_manager),
                    display_manager,
                    window_manager,
                );
                window_manager_add_managed_window(window_manager, window_id, space_manager, view);
            }
        }
    }
}

pub(crate) fn window_manager_make_window_sticky(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    window_id: WindowId,
    should_sticky: bool,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if !window_manager_is_window_eligible(window_id, window_manager) {
        return;
    }

    if should_sticky {
        if scripting_addition_set_sticky(window_id, true) {
            let view = window_manager_find_managed_window(window_manager, window_id);
            if let Some(space_id) = view {
                space_manager_untile_window(
                    space_manager,
                    space_id,
                    window_id,
                    display_manager,
                    window_manager,
                    mouse_drag_state,
                );
                window_manager_remove_managed_window(window_manager, window_id);
                window_manager_purify_window(window_manager, window_id);
            }
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                window_set_flag(window, WindowFlag::STICKY);
            }
        }
    } else {
        if scripting_addition_set_sticky(window_id, false) {
            let Some(window) = window_manager.window.find_mut(&window_id) else {
                return;
            };
            window_clear_flag(window, WindowFlag::STICKY);

            if !window_check_flag(window, WindowFlag::FLOAT) {
                if (window_manager_should_manage_window(window_id, window_manager))
                    && (window_manager_find_managed_window(window_manager, window_id).is_none())
                {
                    let view = space_manager_tile_window_on_space(
                        space_manager,
                        window_id,
                        space_manager_active_space(window_manager),
                        display_manager,
                        window_manager,
                    );
                    window_manager_add_managed_window(
                        window_manager,
                        window_id,
                        space_manager,
                        view,
                    );
                }
            }
        }
    }
}

pub(crate) fn window_manager_toggle_window_shadow(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };

    let shadow = !window_check_flag(window, WindowFlag::SHADOW);
    if scripting_addition_set_shadow(window_id, shadow) {
        let Some(window) = window_manager.window.find_mut(&window_id) else {
            return;
        };
        if shadow {
            window_set_flag(window, WindowFlag::SHADOW);
        } else {
            window_clear_flag(window, WindowFlag::SHADOW);
        }
    }
}

pub(crate) fn window_manager_wait_for_native_fullscreen_transition(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    if workspace_is_macos_monterey()
        || workspace_is_macos_ventura()
        || workspace_is_macos_sonoma()
        || workspace_is_macos_sequoia()
        || workspace_is_macos_tahoe()
    {
        while !space_is_user(space_manager_active_space(window_manager)) {
            //
            // NOTE(asmvik): Window has exited native-fullscreen mode.
            // We need to spin lock until the display is finished animating
            // because we are not actually able to interact with the window.
            //
            // The display_manager API does not work on macOS Monterey.
            //

            unsafe { libc::usleep(100000) };
        }
    } else {
        let display_id = window_display_id(window_id);

        loop {
            //
            // NOTE(asmvik): Window has exited native-fullscreen mode.
            // We need to spin lock until the display is finished animating
            // because we are not actually able to interact with the window.
            //

            unsafe { libc::usleep(100000) };

            if !display_manager_display_is_animating(display_id) {
                break;
            }
        }
    }
}

pub(crate) fn window_manager_toggle_window_native_fullscreen(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    let space_id = window_space(window_id).0 as u32;

    //
    // NOTE(asmvik): The window must become the focused window
    // before we can change its fullscreen attribute. We focus the
    // window and spin lock until a potential space animation has finished.
    //

    window_manager_focus_window_with_raise_resolving_its_application(window_manager, window_id);
    while space_id as u64 != space_manager_active_space(window_manager).0 {
        unsafe { libc::usleep(100000) };
    }

    if let Some(window) = window_manager.window.find(&window_id) {
        if !window_is_fullscreen(window) {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window.element_ref,
                    kAXFullscreenAttribute(),
                    as_cftype(kCFBooleanTrue()),
                )
            };
        } else {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window.element_ref,
                    kAXFullscreenAttribute(),
                    as_cftype(kCFBooleanFalse()),
                )
            };
        }
    }

    //
    // NOTE(asmvik): We toggled the fullscreen attribute and must
    // now spin lock until the post-exit space animation has finished.
    //

    window_manager_wait_for_native_fullscreen_transition(window_id, window_manager);
}

pub(crate) fn window_manager_toggle_window_zoom_parent(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let view = window_manager_find_managed_window(window_manager, window_id);
    let Some(space_id) = view else {
        return;
    };
    if space_manager
        .view
        .find(&space_id)
        .is_none_or(|view| view.layout != ViewType::Bsp)
    {
        return;
    }

    let node = view_find_window_node(space_manager, space_id, window_id);
    debug_assert!(node.is_some());
    let Some(node_id) = node else {
        return;
    };

    let Some((node_parent, node_zoom)) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
        .map(|node| (node.parent, node.zoom))
    else {
        return;
    };

    if node_parent.is_none() {
        return;
    }

    if node_zoom == node_parent {
        if let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(node) = view.find_node_mut(node_id)
        {
            node.zoom = None;
        }
        if space_is_visible(space_id) {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        } else if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.set_flag(ViewFlag::IS_DIRTY);
        }
    } else {
        if let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(node) = view.find_node_mut(node_id)
        {
            node.zoom = node_parent;
        }
        if space_is_visible(space_id) {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        } else if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.set_flag(ViewFlag::IS_DIRTY);
        }
    }
}

pub(crate) fn window_manager_toggle_window_zoom_fullscreen(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let view = window_manager_find_managed_window(window_manager, window_id);
    let Some(space_id) = view else {
        return;
    };
    if space_manager
        .view
        .find(&space_id)
        .is_none_or(|view| view.layout != ViewType::Bsp)
    {
        return;
    }

    let node = view_find_window_node(space_manager, space_id, window_id);
    debug_assert!(node.is_some());
    let Some(node_id) = node else {
        return;
    };

    if node_id == ROOT_NODE_ID {
        return;
    }

    let Some(node_zoom) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
        .map(|node| node.zoom)
    else {
        return;
    };

    if node_zoom == Some(ROOT_NODE_ID) {
        if let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(node) = view.find_node_mut(node_id)
        {
            node.zoom = None;
        }
        if space_is_visible(space_id) {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        } else if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.set_flag(ViewFlag::IS_DIRTY);
        }
    } else {
        if let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(node) = view.find_node_mut(node_id)
        {
            node.zoom = Some(ROOT_NODE_ID);
        }
        if space_is_visible(space_id) {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        } else if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.set_flag(ViewFlag::IS_DIRTY);
        }
    }
}

pub(crate) fn window_manager_toggle_window_windowed_fullscreen(
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let display_id = window_display_id(window_id);
    if display_id.0 == 0 {
        return;
    }

    let Some(window) = window_manager.window.find_mut(&window_id) else {
        return;
    };

    if window_check_flag(window, WindowFlag::WINDOWED) {
        window_clear_flag(window, WindowFlag::WINDOWED);
        let windowed_frame = window.windowed_frame;
        window_manager_animate_window(
            WindowCapture {
                window_id,
                x: windowed_frame.origin.x as f32,
                y: windowed_frame.origin.y as f32,
                width: windowed_frame.size.width as f32,
                height: windowed_frame.size.height as f32,
            },
            window_manager,
        );
    } else {
        window_set_flag(window, WindowFlag::WINDOWED);
        window.windowed_frame = window.frame;
        let bounds = display_bounds_constrained(display_id, true, display_manager);
        window_manager_animate_window(
            WindowCapture {
                window_id,
                x: bounds.origin.x as f32,
                y: bounds.origin.y as f32,
                width: bounds.size.width as f32,
                height: bounds.size.height as f32,
            },
            window_manager,
        );
    }
}

pub(crate) fn window_manager_toggle_window_expose(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    window_manager_focus_window_with_raise_resolving_its_application(window_manager, window_id);
    unsafe { CoreDockSendNotification(k_com_apple_expose_front_awake(), 0) };
}

pub(crate) fn window_manager_toggle_window_pip(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let display_id = window_display_id(window_id);
    if display_id.0 == 0 {
        return;
    }

    let space_id = display_space_id(display_id);
    let display_view =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);

    let mut bounds = display_bounds_constrained(display_id, false, display_manager);
    if let Some(view) = space_manager.view.find(&display_view)
        && view.check_flag(ViewFlag::ENABLE_PADDING)
    {
        bounds.origin.x += view.left_padding as f64;
        bounds.size.width -= (view.left_padding + view.right_padding) as f64;
        bounds.origin.y += view.top_padding as f64;
        bounds.size.height -= (view.top_padding + view.bottom_padding) as f64;
    }

    scripting_addition_scale_window(
        window_id,
        bounds.origin.x as f32,
        bounds.origin.y as f32,
        bounds.size.width as f32,
        bounds.size.height as f32,
    );
}

pub(crate) fn window_manager_find_scratchpad_window(
    window_manager: &mut WindowManager,
    label: &[u8],
) -> Option<WindowId> {
    for index in 0..window_manager.scratchpad_window.len() {
        if window_manager.scratchpad_window[index].label.as_bytes() == label {
            return Some(window_manager.scratchpad_window[index].window_id);
        }
    }

    None
}

pub(crate) fn window_manager_toggle_scratchpad_window_by_label(
    window_manager: &mut WindowManager,
    label: &[u8],
    process_manager: &mut ProcessManager,
) -> bool {
    let window = window_manager_find_scratchpad_window(window_manager, label);
    match window {
        Some(window_id) => {
            window_manager_toggle_scratchpad_window(window_manager, window_id, 0, process_manager)
        }
        None => false,
    }
}

pub(crate) fn window_manager_toggle_scratchpad_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    forced_mode: i32,
    process_manager: &mut ProcessManager,
) -> bool {
    let space_id = space_manager_active_space(window_manager);
    if space_id.0 == 0 {
        return false;
    }

    // TODO(asmvik): Both functions use the same underlying API and could be combined in a single function to reduce redundant work.
    let visible_space = window_space(window_id) == space_id || window_is_sticky(window_id);

    let mut ordered_in: u8 = 0;
    unsafe { SLSWindowIsOrderedIn(*CONNECTION.get().unwrap(), window_id.0, &mut ordered_in) };

    let mode = match forced_mode {
        1 | 2 | 3 => forced_mode,
        _ => {
            if visible_space && ordered_in != 0 {
                1
            } else if visible_space && ordered_in == 0 {
                2
            } else {
                3
            }
        }
    };

    if mode == 1 {
        let next = window_manager_find_window_on_space_by_rank_filtering_window(
            window_manager,
            space_id,
            1,
            window_id,
        );
        if let Some(next) = next {
            window_manager_focus_window_with_raise_resolving_its_application(window_manager, next);
        } else {
            unsafe {
                _SLPSSetFrontProcessWithOptions(
                    &mut process_manager.finder_process_serial_number,
                    0,
                    kCPSNoWindows,
                )
            };
        }
        scripting_addition_order_window(window_id, 0, WindowId(0));
    } else if mode == 2 {
        scripting_addition_order_window(window_id, 1, WindowId(0));
        window_manager_focus_window_with_raise_resolving_its_application(window_manager, window_id);
    } else {
        space_manager_move_window_to_space(space_id, window_id);
        scripting_addition_order_window(window_id, 1, WindowId(0));
        window_manager_focus_window_with_raise_resolving_its_application(window_manager, window_id);
    }

    true
}

pub(crate) fn window_manager_set_scratchpad_for_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    label: String,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> bool {
    let existing_window = window_manager_find_scratchpad_window(window_manager, label.as_bytes());
    if existing_window.is_some() {
        return false;
    }

    window_manager_remove_scratchpad_for_window(
        window_manager,
        window_id,
        false,
        process_manager,
        display_manager,
        space_manager,
        mouse_drag_state,
    );
    let window_scratchpad = label.clone();
    window_manager
        .scratchpad_window
        .push(Scratchpad { label, window_id });
    if let Some(window) = window_manager.window.find_mut(&window_id) {
        window.scratchpad = Some(window_scratchpad);
    }
    window_manager_make_window_floating(
        space_manager,
        window_manager,
        window_id,
        true,
        false,
        display_manager,
        mouse_drag_state,
    );

    true
}

pub(crate) fn window_manager_remove_scratchpad_for_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    unfloat: bool,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) -> bool {
    for index in 0..window_manager.scratchpad_window.len() {
        if window_manager.scratchpad_window[index].window_id == window_id {
            if let Some(window) = window_manager.window.find_mut(&window_id) {
                window.scratchpad = None;
            }

            window_manager.scratchpad_window.swap_remove(index);

            if unfloat {
                window_manager_toggle_scratchpad_window(
                    window_manager,
                    window_id,
                    3,
                    process_manager,
                );
                window_manager_make_window_floating(
                    space_manager,
                    window_manager,
                    window_id,
                    false,
                    false,
                    display_manager,
                    mouse_drag_state,
                );
            }

            return true;
        }
    }

    false
}

pub(crate) fn window_manager_scratchpad_recover_windows(
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let Some(window_list) = window_manager_existing_application_window_list(None, window_manager)
    else {
        return;
    };

    if scripting_addition_order_window_in(&window_list) {
        space_manager_refresh_application_windows(
            space_manager,
            process_manager,
            display_manager,
            window_manager,
            mouse_drag_state,
            mission_control_mode,
        );
    }
}

pub(crate) fn window_manager_validate_windows_on_space(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_list: &[WindowId],
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let view_window_list = view_find_window_list(space_manager, space_id);

    for index in 0..view_window_list.len() {
        let mut found = false;

        for inner_index in 0..window_list.len() {
            if view_window_list[index] == window_list[inner_index] {
                found = true;
                break;
            }
        }

        if !found {
            let Some(window) = window_manager_find_window(window_manager, view_window_list[index])
            else {
                continue;
            };

            //
            // @cleanup
            //
            // :AXBatching
            //
            // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
            // making sure that each window is only moved and resized a single time, when the final layout has been computed.
            // This is necessary to make sure that we do not call the AX API for each modification to the tree.
            //

            view_remove_window_node(
                space_manager,
                space_id,
                window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_adjust_layer(window, LAYER_NORMAL, window_manager);
            window_manager_remove_managed_window(window_manager, window);
            window_manager_purify_window(window_manager, window);

            if let Some(view) = space_manager.view.find_mut(&space_id) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
        }
    }
}

pub(crate) fn window_manager_check_for_windows_on_space(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_list: &[WindowId],
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    for index in 0..window_list.len() {
        let window = window_manager_find_window(window_manager, window_list[index]);
        let Some(window) = window else {
            continue;
        };
        if !window_manager_should_manage_window(window, window_manager) {
            continue;
        }

        let existing_view = window_manager_find_managed_window(window_manager, window);
        let existing_view_layout = existing_view
            .and_then(|existing_space_id| space_manager.view.find(&existing_space_id))
            .map(|view| view.layout);
        if let Some(existing_space_id) = existing_view
            && existing_view_layout != Some(ViewType::Float)
            && existing_space_id != space_id
        {
            //
            // @cleanup
            //
            // :AXBatching
            //
            // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
            // making sure that each window is only moved and resized a single time, when the final layout has been computed.
            // This is necessary to make sure that we do not call the AX API for each modification to the tree.
            //

            view_remove_window_node(
                space_manager,
                existing_space_id,
                window,
                display_manager,
                window_manager,
                mouse_drag_state,
            );
            window_manager_adjust_layer(window, LAYER_NORMAL, window_manager);
            window_manager_remove_managed_window(window_manager, window);
            window_manager_purify_window(window_manager, window);
            if let Some(view) = space_manager.view.find_mut(&existing_space_id) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
        }

        if existing_view.is_none()
            || (existing_view_layout != Some(ViewType::Float) && existing_view != Some(space_id))
        {
            //
            // @cleanup
            //
            // :AXBatching
            //
            // NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
            // making sure that each window is only moved and resized a single time, when the final layout has been computed.
            // This is necessary to make sure that we do not call the AX API for each modification to the tree.
            //

            view_add_window_node(
                space_manager,
                space_id,
                window,
                display_manager,
                window_manager,
            );
            window_manager_adjust_layer(window, LAYER_BELOW, window_manager);
            window_manager_add_managed_window(window_manager, window, space_manager, space_id);
            if let Some(view) = space_manager.view.find_mut(&space_id) {
                view.set_flag(ViewFlag::IS_DIRTY);
            }
        }
    }
}

pub(crate) fn window_manager_validate_and_check_for_windows_on_space(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let view = space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    if space_manager
        .view
        .find(&view)
        .is_none_or(|view| view.layout == ViewType::Float)
    {
        return;
    }

    let window_list = space_window_list(space_id, false, window_manager).unwrap_or_default();
    window_manager_validate_windows_on_space(
        window_manager,
        space_manager,
        view,
        &window_list,
        display_manager,
        mouse_drag_state,
    );
    window_manager_check_for_windows_on_space(
        window_manager,
        space_manager,
        view,
        &window_list,
        display_manager,
        mouse_drag_state,
    );

    //
    // @cleanup
    //
    // :AXBatching
    //
    // NOTE(asmvik): Flush previously batched operations if the view is marked as dirty.
    // This is necessary to make sure that we do not call the AX API for each modification to the tree.
    //

    if space_is_visible(view) && view_is_dirty(space_manager, view) {
        window_node_flush(view, ROOT_NODE_ID, window_manager, space_manager);
        if let Some(view) = space_manager.view.find_mut(&view) {
            view.clear_flag(ViewFlag::IS_DIRTY);
        }
    }
}

pub(crate) fn window_manager_correct_for_mission_control_changes(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let display_list = display_manager_active_display_list();

    let animation_duration = window_manager.window_animation_duration;
    window_manager.window_animation_duration = 0.0f32;

    for index in 0..display_list.len() {
        let display_id = display_list[index];

        let Some(space_list) = display_space_list(display_id) else {
            continue;
        };

        let space_id = display_space_id(display_id);
        for inner_index in 0..space_list.len() {
            if space_list[inner_index] == space_id {
                window_manager_validate_and_check_for_windows_on_space(
                    space_manager,
                    window_manager,
                    space_id,
                    display_manager,
                    mouse_drag_state,
                );
            } else {
                space_manager_mark_view_invalid(
                    space_manager,
                    space_list[inner_index],
                    display_manager,
                    window_manager,
                );
            }
        }
    }

    window_manager.window_animation_duration = animation_duration;
}

pub(crate) fn window_manager_handle_display_add_and_remove(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some(space_list) = display_space_list(display_id) else {
        return;
    };

    for index in 0..space_list.len() {
        if space_is_user(space_list[index]) {
            let window_list = space_window_list(space_list[index], false, window_manager);
            if let Some(window_list) = window_list {
                let view = space_manager_find_view(
                    space_manager,
                    space_list[index],
                    display_manager,
                    window_manager,
                );
                if space_manager
                    .view
                    .find(&view)
                    .is_some_and(|view| view.layout != ViewType::Float)
                {
                    window_manager_check_for_windows_on_space(
                        window_manager,
                        space_manager,
                        view,
                        &window_list,
                        display_manager,
                        mouse_drag_state,
                    );
                }
            }
            break;
        }
    }

    let space_id = display_space_id(display_id);
    for index in 0..space_list.len() {
        if space_list[index] == space_id {
            space_manager_refresh_view(space_manager, space_id, display_manager, window_manager);
        } else {
            space_manager_mark_view_invalid(
                space_manager,
                space_list[index],
                display_manager,
                window_manager,
            );
        }
    }
}

pub(crate) fn window_manager_init(window_manager: &mut WindowManager) {
    window_manager.system_element = CFRetained::into_raw(unsafe { AXUIElementCreateSystemWide() })
        .as_ptr()
        .cast_const();
    unsafe { AXUIElementSetMessagingTimeout(&*window_manager.system_element, 1.0) };

    window_manager.ffm_mode = FfmMode::Disabled;
    window_manager.purify_mode = PurifyMode::Disabled;
    window_manager.window_origin_mode = WindowOriginMode::Default;
    window_manager.enable_mff = false;
    window_manager.enable_window_opacity = false;
    window_manager.menubar_opacity = 1.0f32;
    window_manager.active_window_opacity = 1.0f32;
    window_manager.normal_window_opacity = 1.0f32;
    window_manager.window_opacity_duration = 0.0f32;
    window_manager.window_animation_duration = 0.0f32;
    window_manager.window_animation_easing = AnimationEasingType::EaseOutCirc;
    window_manager.insert_feedback_color = rgba_color_from_hex(0xffd75f5f);

    window_manager.application = Table::new(150, hash_wm_process_id);
    window_manager.window = Table::new(150, hash_wm_window_id);
    window_manager.managed_window = Table::new(150, hash_wm_window_id);
    window_manager.window_lost_focused_event = Table::new(150, hash_wm_window_id);
    window_manager.application_lost_front_switched_event = Table::new(150, hash_wm_process_id);
    window_manager.window_animations_table =
        Arc::new(Mutex::new(Table::new(150, hash_wm_window_id)));
    window_manager.insert_feedback = Table::new(150, hash_wm_window_id);
}

pub(crate) fn window_manager_begin(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    objc2::rc::autoreleasepool(|_pool| {
        let process_list: Vec<Arc<Process>> = PROCESS_TABLE
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .values()
            .map(Arc::clone)
            .collect();

        for process in process_list {
            if workspace_application_is_observable(&process) {
                let mut application = application_create(&process);

                if application_observe(&mut application) {
                    let application_process_id = application.process_id;
                    window_manager_add_application(window_manager, application);
                    window_manager_add_existing_application_windows(
                        space_manager,
                        window_manager,
                        application_process_id,
                        -1,
                        process_manager,
                        display_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    );
                } else {
                    application_unobserve(&mut application);
                    application_destroy(application);
                }
            } else {
                crate::debug!(
                    "{}: {} ({}) is not observable, subscribing to activationPolicy changes\n",
                    "window_manager_begin",
                    process.name,
                    process.process_id.0
                );
                workspace_application_observe_activation_policy(
                    WORKSPACE_CONTEXT.get().unwrap(),
                    &process,
                );
            }
        }
    });

    let window = window_manager_focused_window(window_manager);
    if let Some(window_id) = window {
        window_manager.last_window_id = window_id;
        window_manager.focused_window_id = window_id;
        if let Some(application) = window_manager
            .window
            .find(&window_id)
            .and_then(|window| window.application)
            .and_then(|application_process_id| {
                window_manager.application.find(&application_process_id)
            })
        {
            window_manager.focused_window_process_serial_number = application.process_serial_number;
        }
        window_manager_set_window_opacity(
            window_manager,
            window_id,
            window_manager.active_window_opacity,
        );
    }
}
