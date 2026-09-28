#![allow(deprecated)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::application::Application;
use crate::ffi::accessibility::{
    AXUIElementCreateSystemWide, AXUIElementRef, AXUIElementSetMessagingTimeout,
};
use crate::ffi::carbon_process::ProcessSerialNumber;
use crate::ffi::core_foundation::CFRetained;
use crate::layout::group_header_style::{
    GroupHeaderStyle, group_header_style_with_its_initial_settings,
};
use crate::layout::settings::ViewLayout;
use crate::space::manager::SpaceManager;
use crate::support::color::{RgbaColor, rgba_color_from_packed_argb};
use crate::support::easing::AnimationEasingType;
use crate::support::handles::{NodeId, ProcessId, SpaceId, WindowId};
use crate::window::animator::WindowAnimator;
use crate::window::model::{
    Window, WindowFlag, WindowRuleFlag, is_window_a_standard_floating_or_dialog_window,
    is_window_a_standard_window, is_window_at_normal_window_level, is_window_movable,
    is_window_on_more_than_one_space,
};
use crate::window::rule::Rule;
use crate::window::scratchpad::Scratchpad;
use crate::window::shadow::apply_shadow_removal_mode_to_window;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowOperationOutcome {
    Success,
    InvalidSourceView,
    InvalidSourceNode,
    InvalidDestinationView,
    InvalidDestinationNode,
    InvalidOperation,
    SameWindow,
    CannotMinimize,
    AlreadyMinimized,
    MinimizeFailed,
    NotMinimized,
    DeminimizeFailed,
    StackIsFull,
    SameStack,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum ShadowRemovalMode {
    #[default]
    Never = 0,
    FromManagedWindows = 1,
    FromEveryWindow = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum FocusFollowsMouseMode {
    #[default]
    Disabled = 0,
    Autofocus = 1,
    Autoraise = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum WindowOriginDisplayMode {
    #[default]
    DisplayTheWindowOpenedOn = 0,
    FocusedDisplay = 1,
    DisplayUnderTheCursor = 2,
}

pub(crate) struct WindowManager {
    pub(crate) system_element: AXUIElementRef,
    pub(crate) application: BTreeMap<ProcessId, Application>,
    pub(crate) window: BTreeMap<WindowId, Window>,
    pub(crate) managed_window: BTreeMap<WindowId, SpaceId>,
    pub(crate) window_lost_focused_event: BTreeSet<WindowId>,
    pub(crate) application_lost_front_switched_event: BTreeSet<ProcessId>,
    pub(crate) window_animator: Arc<WindowAnimator>,
    pub(crate) insert_feedback: BTreeMap<WindowId, (SpaceId, NodeId)>,
    pub(crate) rules: Vec<Rule>,
    pub(crate) applications_to_refresh: Vec<ProcessId>,
    pub(crate) focused_window_id: WindowId,
    pub(crate) focused_window_process_serial_number: ProcessSerialNumber,
    pub(crate) last_window_id: WindowId,
    pub(crate) enable_mff: bool,
    pub(crate) focus_follows_mouse_mode: FocusFollowsMouseMode,
    pub(crate) shadow_removal_mode: ShadowRemovalMode,
    pub(crate) window_origin_display_mode: WindowOriginDisplayMode,
    pub(crate) enable_window_opacity: bool,
    pub(crate) menubar_opacity: f32,
    pub(crate) active_window_opacity: f32,
    pub(crate) normal_window_opacity: f32,
    pub(crate) window_opacity_duration: f32,
    pub(crate) window_animation_duration: f32,
    pub(crate) window_animation_easing: AnimationEasingType,
    pub(crate) insert_feedback_color: RgbaColor,
    pub(crate) insert_feedback_color_follows_the_system_accent_color: bool,
    pub(crate) scratchpad_window: Vec<Scratchpad>,
    pub(crate) group_header_style: GroupHeaderStyle,
    pub(crate) group_headers_are_hidden_during_mission_control: bool,
}

pub(crate) static SHADOW_REMOVAL_MODE_NAMES: [&str; 3] = ["on", "float", "off"];

pub(crate) static FOCUS_FOLLOWS_MOUSE_MODE_NAMES: [&str; 3] =
    ["disabled", "autofocus", "autoraise"];

pub(crate) static WINDOW_ORIGIN_DISPLAY_MODE_NAMES: [&str; 3] = ["default", "focused", "cursor"];

pub(crate) fn is_window_eligible_for_management(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(window) = window_manager.window.get(&window_id) else {
        return false;
    };

    let result = window.is_root
        && (is_window_a_standard_floating_or_dialog_window(window)
            || window.rule_flags.contains(WindowRuleFlag::MANAGE_FORCED_ON));
    result
}

pub(crate) fn should_window_be_managed(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> bool {
    let Some(window) = window_manager.window.get(&window_id) else {
        return false;
    };

    if !window.is_root {
        return false;
    }
    if window.flags.contains(WindowFlag::FLOATING) {
        return false;
    }
    if is_window_on_more_than_one_space(window_id) {
        return false;
    }
    if window.flags.contains(WindowFlag::MINIMIZED) {
        return false;
    }

    let application_is_hidden = match window.application {
        Some(application_process_id) => {
            match window_manager.application.get(&application_process_id) {
                Some(application) => application.is_hidden,
                None => return false,
            }
        }
        None => return false,
    };
    if application_is_hidden {
        return false;
    }

    (is_window_a_standard_window(window)
        && is_window_at_normal_window_level(window)
        && is_window_movable(window))
        || window.rule_flags.contains(WindowRuleFlag::MANAGE_FORCED_ON)
}

pub(crate) fn space_managing_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) -> Option<SpaceId> {
    window_manager.managed_window.get(&window_id).copied()
}

pub(crate) fn forget_managed_window(window_manager: &mut WindowManager, window_id: WindowId) {
    window_manager.managed_window.remove(&window_id);
}

pub(crate) fn record_managed_window_on_space_updating_its_shadow(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) {
    let Some(view) = space_manager.view.get(&space_id) else {
        return;
    };
    if view.layout == ViewLayout::Float {
        return;
    }
    window_manager
        .managed_window
        .entry(window_id)
        .or_insert(space_id);
    apply_shadow_removal_mode_to_window(window_manager, window_id);
}

pub(crate) fn has_front_switched_event_arrived_before_application_was_tracked(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) -> bool {
    window_manager
        .application_lost_front_switched_event
        .contains(&process_id)
}

pub(crate) fn forget_front_switched_event_that_arrived_before_application_was_tracked(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) {
    window_manager
        .application_lost_front_switched_event
        .remove(&process_id);
}

pub(crate) fn record_front_switched_event_that_arrived_before_application_was_tracked(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) {
    window_manager
        .application_lost_front_switched_event
        .insert(process_id);
}

pub(crate) fn has_focused_event_arrived_before_window_was_tracked(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) -> bool {
    window_manager
        .window_lost_focused_event
        .contains(&window_id)
}

pub(crate) fn forget_focused_event_that_arrived_before_window_was_tracked(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) {
    window_manager.window_lost_focused_event.remove(&window_id);
}

pub(crate) fn record_focused_event_that_arrived_before_window_was_tracked(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) {
    window_manager.window_lost_focused_event.insert(window_id);
}

pub(crate) fn tracked_window_with_id(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) -> Option<WindowId> {
    window_manager
        .window
        .get(&window_id)
        .map(|window| window.id)
}

pub(crate) fn stop_tracking_window(
    window_manager: &mut WindowManager,
    window_id: WindowId,
) -> Option<Window> {
    window_manager.window.remove(&window_id)
}

pub(crate) fn start_tracking_window(window_manager: &mut WindowManager, window: Window) {
    window_manager.window.entry(window.id).or_insert(window);
}

pub(crate) fn tracked_application_with_process_id(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) -> Option<ProcessId> {
    window_manager
        .application
        .get(&process_id)
        .map(|application| application.process_id)
}

pub(crate) fn stop_tracking_application(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) -> Option<Application> {
    window_manager.application.remove(&process_id)
}

pub(crate) fn start_tracking_application(
    window_manager: &mut WindowManager,
    application: Application,
) {
    window_manager
        .application
        .entry(application.process_id)
        .or_insert(application);
}

pub(crate) fn tracked_windows_of_application(
    window_manager: &mut WindowManager,
    process_id: ProcessId,
) -> Vec<WindowId> {
    window_manager
        .window
        .values()
        .filter(|window| window.application == Some(process_id))
        .map(|window| window.id)
        .collect()
}

pub(crate) fn initialize_window_manager(window_manager: &mut WindowManager) {
    window_manager.system_element = CFRetained::into_raw(unsafe { AXUIElementCreateSystemWide() })
        .as_ptr()
        .cast_const();
    unsafe { AXUIElementSetMessagingTimeout(&*window_manager.system_element, 1.0) };

    window_manager.focus_follows_mouse_mode = FocusFollowsMouseMode::Disabled;
    window_manager.shadow_removal_mode = ShadowRemovalMode::Never;
    window_manager.window_origin_display_mode = WindowOriginDisplayMode::DisplayTheWindowOpenedOn;
    window_manager.enable_mff = false;
    window_manager.enable_window_opacity = false;
    window_manager.menubar_opacity = 1.0f32;
    window_manager.active_window_opacity = 1.0f32;
    window_manager.normal_window_opacity = 1.0f32;
    window_manager.window_opacity_duration = 0.0f32;
    window_manager.window_animation_duration = 0.0f32;
    window_manager.window_animation_easing = AnimationEasingType::EaseOutCirc;
    window_manager.insert_feedback_color = rgba_color_from_packed_argb(0xffd75f5f);
    window_manager.insert_feedback_color_follows_the_system_accent_color = true;
    window_manager.group_header_style = group_header_style_with_its_initial_settings();
    window_manager.group_headers_are_hidden_during_mission_control = false;

    window_manager.application = BTreeMap::new();
    window_manager.window = BTreeMap::new();
    window_manager.managed_window = BTreeMap::new();
    window_manager.window_lost_focused_event = BTreeSet::new();
    window_manager.application_lost_front_switched_event = BTreeSet::new();
    window_manager.window_animator = Arc::new(WindowAnimator::new());
    window_manager.insert_feedback = BTreeMap::new();
}

#[cfg(test)]
pub(crate) fn create_window_manager_tracking_nothing_with_its_initial_settings() -> WindowManager {
    WindowManager {
        system_element: core::ptr::null(),
        application: BTreeMap::new(),
        window: BTreeMap::new(),
        managed_window: BTreeMap::new(),
        window_lost_focused_event: BTreeSet::new(),
        application_lost_front_switched_event: BTreeSet::new(),
        window_animator: Arc::new(WindowAnimator::new()),
        insert_feedback: BTreeMap::new(),
        rules: Vec::new(),
        applications_to_refresh: Vec::new(),
        focused_window_id: WindowId(0),
        focused_window_process_serial_number: ProcessSerialNumber {
            high_long_of_psn: 0,
            low_long_of_psn: 0,
        },
        last_window_id: WindowId(0),
        enable_mff: false,
        focus_follows_mouse_mode: FocusFollowsMouseMode::Disabled,
        shadow_removal_mode: ShadowRemovalMode::Never,
        window_origin_display_mode: WindowOriginDisplayMode::DisplayTheWindowOpenedOn,
        enable_window_opacity: false,
        menubar_opacity: 1.0f32,
        active_window_opacity: 1.0f32,
        normal_window_opacity: 1.0f32,
        window_opacity_duration: 0.0f32,
        window_animation_duration: 0.0f32,
        window_animation_easing: AnimationEasingType::EaseOutCirc,
        insert_feedback_color: rgba_color_from_packed_argb(0xffd75f5f),
        insert_feedback_color_follows_the_system_accent_color: true,
        scratchpad_window: Vec::new(),
        group_header_style: group_header_style_with_its_initial_settings(),
        group_headers_are_hidden_during_mission_control: false,
    }
}
