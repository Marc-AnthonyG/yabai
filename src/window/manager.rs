#![allow(deprecated)]

use std::sync::{Arc, Mutex};

use crate::application::model::Application;
use crate::ffi::accessibility::{
    AXUIElementCreateSystemWide, AXUIElementRef, AXUIElementSetMessagingTimeout,
};
use crate::ffi::carbon_process::ProcessSerialNumber;
use crate::ffi::core_foundation::CFRetained;
use crate::layout::settings::ViewType;
use crate::space::manager::SpaceManager;
use crate::support::color::{RgbaColor, rgba_color_from_hex};
use crate::support::easing::AnimationEasingType;
use crate::support::handles::{NodeId, ProcessId, SpaceId, WindowId};
use crate::support::table::Table;
use crate::window::animation::AnimationContext;
use crate::window::model::{
    Window, WindowFlag, WindowRuleFlag, window_can_move, window_check_flag, window_check_rule_flag,
    window_is_real, window_is_standard, window_is_sticky, window_level_is_standard,
};
use crate::window::rule::Rule;
use crate::window::scratchpad::Scratchpad;
use crate::window::shadow::window_manager_purify_window;

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
    pub(crate) insert_feedback_color_follows_the_system_accent_color: bool,
    pub(crate) scratchpad_window: Vec<Scratchpad>,
}

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
    window_manager.insert_feedback_color_follows_the_system_accent_color = true;

    window_manager.application = Table::new(150, hash_wm_process_id);
    window_manager.window = Table::new(150, hash_wm_window_id);
    window_manager.managed_window = Table::new(150, hash_wm_window_id);
    window_manager.window_lost_focused_event = Table::new(150, hash_wm_window_id);
    window_manager.application_lost_front_switched_event = Table::new(150, hash_wm_process_id);
    window_manager.window_animations_table =
        Arc::new(Mutex::new(Table::new(150, hash_wm_window_id)));
    window_manager.insert_feedback = Table::new(150, hash_wm_window_id);
}
