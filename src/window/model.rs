#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::ffi::CFStringOwned;
use crate::ffi::accessibility::{
    AXUIElementCopyAttributeValue, AXUIElementIsAttributeSettable, AXUIElementRef, AXValue,
    AXValueGetValue, AXValueType, kAXDialogSubrole, kAXErrorSuccess, kAXFloatingWindowSubrole,
    kAXFullscreenAttribute, kAXMinimizedAttribute, kAXParentAttribute, kAXPositionAttribute,
    kAXRoleAttribute, kAXSizeAttribute, kAXStandardWindowSubrole, kAXSubroleAttribute,
    kAXTitleAttribute, kAXUnknownSubrole, kAXWindowRole,
};
use crate::ffi::color_sync::CGDisplayGetDisplayIDFromUUID;
use crate::ffi::core_foundation::{
    CFBoolean, CFEqual, CFIndex, CFNumber, CFRetainedAssumedSendAndSync, CFString, CFType,
    CFUUIDCreateFromString, CGPoint, CGRect, as_cftype, cfarray_borrow_value_at_index,
    cfarray_count, cfboolean_get_value, cfnumber_read_u64_widening, cfstring_to_string,
    create_cfarray_of_cfnumbers, kCFNumberSInt32Type, take_create_rule_result,
    window_title_property_key,
};
use crate::ffi::mach_port::{
    MACH_RCV_MSG, MACH_SEND_MSG, NDR_record, NDR_record_t, mach_msg, mach_msg_header_t,
    mig_get_special_reply_port,
};
use crate::ffi::skylight::{
    SLSCopyBestManagedDisplayForRect, SLSCopyManagedDisplayForWindow, SLSCopySpacesForWindows,
    SLSCopyWindowProperty, SLSGetWindowAlpha, SLSGetWindowBounds, SLSGetWindowLevel,
    SLSGetWindowSubLevel, SLSManagedDisplayGetCurrentSpace, SLSWindowIteratorAdvance,
    SLSWindowIteratorGetCount, SLSWindowIteratorGetLevel, SLSWindowIteratorGetParentID,
    SLSWindowIteratorGetTags, SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows,
};
use crate::ffi::skylight_dynamic::resolved_cgs_get_connection_port_by_id_function;
use crate::space::managed_space::is_native_fullscreen_space;
use crate::state::process_wide::{LAYER_NORMAL_WINDOW_LEVEL, SKYLIGHT_CONNECTION_ID};
use crate::support::handles::{DisplayId, ProcessId, SpaceId, WindowId};
use crate::support::macos_version::{
    is_running_on_macos_sequoia, is_running_on_macos_sonoma, is_running_on_macos_tahoe,
    is_running_on_macos_ventura,
};
use crate::window::manager::WindowManager;

pub(crate) struct Window {
    pub(crate) application: Option<ProcessId>,
    pub(crate) element_ref: AXUIElementRef,
    pub(crate) id: WindowId,
    pub(crate) liveness: Arc<WindowLivenessCell>,
    pub(crate) liveness_reference_held_by_the_observation: Option<*const WindowLivenessCell>,
    pub(crate) role: Option<CFStringOwned>,
    pub(crate) subrole: Option<CFStringOwned>,
    pub(crate) title: Option<CFStringOwned>,
    pub(crate) frame: CGRect,
    pub(crate) windowed_frame: CGRect,
    pub(crate) is_root: bool,
    pub(crate) is_eligible: bool,
    pub(crate) notification: u8,
    pub(crate) rule_flags: u8,
    pub(crate) flags: u8,
    pub(crate) opacity: f32,
    pub(crate) layer: i32,
    pub(crate) scratchpad: Option<String>,
    pub(crate) stays_a_group_on_its_own: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowFlag(pub u8);

impl WindowFlag {
    pub(crate) const HAS_SHADOW: WindowFlag = WindowFlag(0x01);
    pub(crate) const IN_NATIVE_FULLSCREEN: WindowFlag = WindowFlag(0x02);
    pub(crate) const MINIMIZED: WindowFlag = WindowFlag(0x04);
    pub(crate) const FLOATING: WindowFlag = WindowFlag(0x08);
    pub(crate) const STICKY: WindowFlag = WindowFlag(0x10);
    pub(crate) const IN_WINDOWED_FULLSCREEN: WindowFlag = WindowFlag(0x20);
    pub(crate) const MOVABLE: WindowFlag = WindowFlag(0x40);
    pub(crate) const RESIZABLE: WindowFlag = WindowFlag(0x80);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowRuleFlag(pub u8);

impl WindowRuleFlag {
    pub(crate) const MANAGE_FORCED_ON: WindowRuleFlag = WindowRuleFlag(0x01);
    pub(crate) const NATIVE_FULLSCREEN_REQUESTED: WindowRuleFlag = WindowRuleFlag(0x02);
    pub(crate) const OVERRIDES_MOUSE_FOLLOWS_FOCUS: WindowRuleFlag = WindowRuleFlag(0x04);
    pub(crate) const MOUSE_FOLLOWS_FOCUS_OVERRIDE_IS_ON: WindowRuleFlag = WindowRuleFlag(0x08);
}

pub(crate) struct WindowLivenessCell {
    pub(crate) window_id: WindowId,
    pub(crate) state: AtomicU8,
}

pub(crate) const WINDOW_LIVENESS_ALIVE: u8 = 0;
pub(crate) const WINDOW_LIVENESS_CLAIMED_FOR_DESTRUCTION: u8 = 1;

impl WindowLivenessCell {
    pub(crate) fn claim_for_destruction(&self) -> bool {
        self.state
            .compare_exchange(
                WINDOW_LIVENESS_ALIVE,
                WINDOW_LIVENESS_CLAIMED_FOR_DESTRUCTION,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
    }

    pub(crate) fn is_still_alive(&self) -> bool {
        self.state.load(Ordering::SeqCst) == WINDOW_LIVENESS_ALIVE
    }
}

#[allow(non_snake_case)]
#[repr(C, packed(4))]
pub(crate) struct WindowSubLevelMachMessage {
    pub(crate) header: mach_msg_header_t,
    pub(crate) NDR_record: NDR_record_t,
    pub(crate) window_id: u32,
    pub(crate) sub_level: i32,
    pub(crate) padding1: i32,
    pub(crate) padding2: i32,
}

const _: () = assert!(core::mem::size_of::<WindowSubLevelMachMessage>() == 0x30);
const _: () = assert!(core::mem::offset_of!(WindowSubLevelMachMessage, window_id) == 0x20);
const _: () = assert!(core::mem::offset_of!(WindowSubLevelMachMessage, sub_level) == 0x24);

pub(crate) fn is_window_flag_set(window: &Window, flag: WindowFlag) -> bool {
    (window.flags & flag.0) != 0
}

pub(crate) fn clear_window_flag(window: &mut Window, flag: WindowFlag) {
    window.flags &= !flag.0;
}

pub(crate) fn set_window_flag(window: &mut Window, flag: WindowFlag) {
    window.flags |= flag.0;
}

pub(crate) fn is_window_rule_flag_set(window: &Window, flag: WindowRuleFlag) -> bool {
    (window.rule_flags & flag.0) != 0
}

pub(crate) fn clear_window_rule_flag(window: &mut Window, flag: WindowRuleFlag) {
    window.rule_flags &= !flag.0;
}

pub(crate) fn set_window_rule_flag(window: &mut Window, flag: WindowRuleFlag) {
    window.rule_flags |= flag.0;
}

pub(crate) fn copy_uuid_of_display_holding_window(window_id: WindowId) -> Option<CFStringOwned> {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();

    let uuid = unsafe {
        take_create_rule_result(SLSCopyManagedDisplayForWindow(connection_id, window_id.0))
    };
    if uuid.is_none() {
        let mut frame = CGRect::ZERO;
        unsafe { SLSGetWindowBounds(connection_id, window_id.0, &mut frame) };
        return unsafe {
            take_create_rule_result(SLSCopyBestManagedDisplayForRect(connection_id, frame))
        }
        .map(CFRetainedAssumedSendAndSync);
    }

    uuid.map(CFRetainedAssumedSendAndSync)
}

pub(crate) fn query_display_holding_window(window_id: WindowId) -> DisplayId {
    let Some(uuid_string) = copy_uuid_of_display_holding_window(window_id) else {
        return DisplayId(0);
    };

    let Some(uuid) = CFUUIDCreateFromString(None, Some(uuid_string.as_ref())) else {
        return DisplayId(0);
    };
    let id = unsafe { CGDisplayGetDisplayIDFromUUID(&*uuid) } as i32;

    DisplayId(id as u32)
}

pub(crate) fn query_current_space_of_display_holding_window(window_id: WindowId) -> SpaceId {
    let Some(uuid) = copy_uuid_of_display_holding_window(window_id) else {
        return SpaceId(0);
    };

    let space_id = unsafe {
        SLSManagedDisplayGetCurrentSpace(*SKYLIGHT_CONNECTION_ID.get().unwrap(), uuid.as_ref())
    };

    SpaceId(space_id)
}

pub(crate) fn query_space_holding_window(window_id: WindowId) -> SpaceId {
    let mut space_id: u64 = 0;

    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let window_list_ref = create_cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);
    let space_list_ref = unsafe {
        take_create_rule_result(SLSCopySpacesForWindows(
            connection_id,
            0x7,
            &*window_list_ref,
        ))
    };

    if let Some(space_list_ref) = space_list_ref {
        let count = cfarray_count(&space_list_ref) as i32;
        if count != 0 {
            let id_ref: Option<&CFNumber> =
                unsafe { cfarray_borrow_value_at_index(&space_list_ref, 0) };
            if let Some(id_ref) = id_ref {
                space_id = cfnumber_read_u64_widening(id_ref);
            }
        }
    }
    drop(window_list_ref);

    if space_id != 0 {
        SpaceId(space_id)
    } else {
        query_current_space_of_display_holding_window(window_id)
    }
}

pub(crate) fn query_every_space_holding_window(window_id: WindowId) -> Vec<SpaceId> {
    let mut space_list: Vec<SpaceId> = Vec::new();

    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let window_list_ref = create_cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);
    let Some(space_list_ref) = (unsafe {
        take_create_rule_result(SLSCopySpacesForWindows(
            connection_id,
            0x7,
            &*window_list_ref,
        ))
    }) else {
        return space_list;
    };

    let count = cfarray_count(&space_list_ref) as i32;
    if count == 0 {
        return space_list;
    }

    space_list.reserve(count as usize);

    for index in 0..count {
        let id_ref: Option<&CFNumber> =
            unsafe { cfarray_borrow_value_at_index(&space_list_ref, index as CFIndex) };
        space_list.push(SpaceId(match id_ref {
            Some(id_ref) => cfnumber_read_u64_widening(id_ref),
            None => 0,
        }));
    }

    space_list
}

pub(crate) fn query_window_title_from_window_server(window_id: WindowId) -> String {
    let mut value: *mut CFType = core::ptr::null_mut();
    unsafe {
        SLSCopyWindowProperty(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            window_id.0,
            window_title_property_key(),
            &mut value,
        )
    };
    let Some(value) = (unsafe { take_create_rule_result(value.cast_const()) }) else {
        return String::new();
    };

    let result = cfstring_to_string(unsafe { &*((&*value as *const CFType).cast::<CFString>()) });
    drop(value);
    result.unwrap_or_default()
}

pub(crate) fn window_title_as_string(window: &Window) -> String {
    match &window.title {
        Some(title) => cfstring_to_string(title.as_ref()).unwrap_or_default(),
        None => String::new(),
    }
}

pub(crate) fn copy_window_title_through_accessibility(window: &Window) -> Option<CFStringOwned> {
    let mut value: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXTitleAttribute(),
            NonNull::from(&mut value),
        )
    };
    unsafe { take_create_rule_result(value.cast::<CFString>()) }.map(CFRetainedAssumedSendAndSync)
}

pub(crate) fn read_window_origin_through_accessibility(window: &Window) -> CGPoint {
    let mut origin = CGPoint::ZERO;
    let mut position_ref: *const CFType = core::ptr::null();

    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXPositionAttribute(),
            NonNull::from(&mut position_ref),
        )
    };

    if let Some(position_ref) = unsafe { take_create_rule_result(position_ref) } {
        unsafe {
            AXValueGetValue(
                &*((&*position_ref as *const CFType).cast::<AXValue>()),
                AXValueType::CGPoint,
                NonNull::from(&mut origin).cast::<c_void>(),
            )
        };
        drop(position_ref);
    }

    origin
}

pub(crate) fn read_window_frame_through_accessibility(window: &Window) -> CGRect {
    let mut frame = CGRect::ZERO;
    let mut position_ref: *const CFType = core::ptr::null();
    let mut size_ref: *const CFType = core::ptr::null();

    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXPositionAttribute(),
            NonNull::from(&mut position_ref),
        )
    };
    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXSizeAttribute(),
            NonNull::from(&mut size_ref),
        )
    };

    if let Some(position_ref) = unsafe { take_create_rule_result(position_ref) } {
        unsafe {
            AXValueGetValue(
                &*((&*position_ref as *const CFType).cast::<AXValue>()),
                AXValueType::CGPoint,
                NonNull::from(&mut frame.origin).cast::<c_void>(),
            )
        };
        drop(position_ref);
    }

    if let Some(size_ref) = unsafe { take_create_rule_result(size_ref) } {
        unsafe {
            AXValueGetValue(
                &*((&*size_ref as *const CFType).cast::<AXValue>()),
                AXValueType::CGSize,
                NonNull::from(&mut frame.size).cast::<c_void>(),
            )
        };
        drop(size_ref);
    }

    frame
}

pub(crate) fn can_window_be_moved_through_accessibility(window: &Window) -> bool {
    let mut result: u8 = 0;
    if unsafe {
        AXUIElementIsAttributeSettable(
            &*window.element_ref,
            kAXPositionAttribute(),
            NonNull::from(&mut result),
        )
    } != kAXErrorSuccess
    {
        result = 0;
    }
    result != 0
}

pub(crate) fn is_window_movable(window: &Window) -> bool {
    is_window_flag_set(window, WindowFlag::MOVABLE)
}

pub(crate) fn can_window_be_resized_through_accessibility(window: &Window) -> bool {
    let mut result: u8 = 0;
    if unsafe {
        AXUIElementIsAttributeSettable(
            &*window.element_ref,
            kAXSizeAttribute(),
            NonNull::from(&mut result),
        )
    } != kAXErrorSuccess
    {
        result = 0;
    }
    result != 0
}

pub(crate) fn is_window_resizable(window: &Window) -> bool {
    is_window_flag_set(window, WindowFlag::RESIZABLE)
}

pub(crate) fn can_window_be_minimized_through_accessibility(window: &Window) -> bool {
    let mut result: u8 = 0;
    if unsafe {
        AXUIElementIsAttributeSettable(
            &*window.element_ref,
            kAXMinimizedAttribute(),
            NonNull::from(&mut result),
        )
    } != kAXErrorSuccess
    {
        result = 0;
    }
    result != 0
}

pub(crate) fn is_window_at_most_500_points_wide_or_tall(window: &Window) -> bool {
    if window.frame.size.width <= 500.0f32 as f64 {
        return true;
    }
    if window.frame.size.height <= 500.0f32 as f64 {
        return true;
    }
    false
}

pub(crate) fn is_window_minimized_according_to_accessibility(window: &Window) -> bool {
    let mut result: bool = false;
    let mut value: *const CFType = core::ptr::null();

    if unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXMinimizedAttribute(),
            NonNull::from(&mut value),
        )
    } == kAXErrorSuccess
    {
        if let Some(value) = unsafe { take_create_rule_result(value) } {
            result =
                cfboolean_get_value(unsafe { &*((&*value as *const CFType).cast::<CFBoolean>()) });
            drop(value);
        }
    }

    result
}

pub(crate) fn is_window_in_native_fullscreen_according_to_accessibility(window: &Window) -> bool {
    let mut result: bool = false;
    let mut value: *const CFType = core::ptr::null();

    if unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXFullscreenAttribute(),
            NonNull::from(&mut value),
        )
    } == kAXErrorSuccess
    {
        if let Some(value) = unsafe { take_create_rule_result(value) } {
            result =
                cfboolean_get_value(unsafe { &*((&*value as *const CFType).cast::<CFBoolean>()) });
            drop(value);
        }
    }

    result
}

pub(crate) fn is_window_on_more_than_one_space(window_id: WindowId) -> bool {
    let mut result = false;

    let window_list_ref = create_cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);
    let space_list_ref = unsafe {
        take_create_rule_result(SLSCopySpacesForWindows(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            0x7,
            &*window_list_ref,
        ))
    };
    if let Some(space_list_ref) = space_list_ref {
        result = cfarray_count(&space_list_ref) > 1;
        drop(space_list_ref);
    }

    drop(window_list_ref);
    result
}

pub(crate) fn is_window_shadow_shown_according_to_window_server(window_id: WindowId) -> bool {
    let tags = query_window_tags_from_window_server(window_id);
    (tags & 0x8) == 0
}

pub(crate) fn query_window_opacity_from_window_server(window_id: WindowId) -> f32 {
    let mut alpha: f32 = 0.0f32;
    unsafe {
        SLSGetWindowAlpha(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            window_id.0,
            &mut alpha,
        )
    };
    alpha
}

pub(crate) fn query_parent_window_from_window_server(window_id: WindowId) -> WindowId {
    let mut parent_window_id: u32 = 0;
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();

    let window_ref = create_cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);

    let query =
        unsafe { take_create_rule_result(SLSWindowQueryWindows(connection_id, &*window_ref, 1)) };
    if let Some(query) = query {
        let iterator = unsafe { take_create_rule_result(SLSWindowQueryResultCopyWindows(&*query)) };
        if let Some(iterator) = iterator {
            if unsafe { SLSWindowIteratorGetCount(&*iterator) } == 1 {
                if unsafe { SLSWindowIteratorAdvance(&*iterator) } {
                    parent_window_id = unsafe { SLSWindowIteratorGetParentID(&*iterator) };
                }
            }

            drop(iterator);
        }
        drop(query);
    }
    drop(window_ref);

    WindowId(parent_window_id)
}

pub(crate) fn query_window_level_from_window_server(window_id: WindowId) -> i32 {
    let mut level: i32 = 0;
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();

    if is_running_on_macos_ventura()
        || is_running_on_macos_sonoma()
        || is_running_on_macos_sequoia()
        || is_running_on_macos_tahoe()
    {
        let window_ref = create_cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);

        let query = unsafe {
            take_create_rule_result(SLSWindowQueryWindows(connection_id, &*window_ref, 1))
        };
        if let Some(query) = query {
            let iterator =
                unsafe { take_create_rule_result(SLSWindowQueryResultCopyWindows(&*query)) };
            if let Some(iterator) = iterator {
                if unsafe { SLSWindowIteratorGetCount(&*iterator) } == 1 {
                    if unsafe { SLSWindowIteratorAdvance(&*iterator) } {
                        level = unsafe { SLSWindowIteratorGetLevel(&*iterator) };
                    }
                }

                drop(iterator);
            }
            drop(query);
        }
        drop(window_ref);
    } else {
        unsafe { SLSGetWindowLevel(connection_id, window_id.0, &mut level) };
    }

    level
}

#[allow(non_snake_case)]
pub(crate) fn query_window_sub_level_with_a_raw_mach_message(
    connection_id: i32,
    window_id: WindowId,
) -> i32 {
    let Some(resolved_cgs_get_connection_port_by_id_function) =
        resolved_cgs_get_connection_port_by_id_function()
    else {
        return 0;
    };

    let mut message = WindowSubLevelMachMessage {
        header: mach_msg_header_t {
            msgh_bits: 0,
            msgh_size: 0,
            msgh_remote_port: 0,
            msgh_local_port: 0,
            msgh_voucher_port: 0,
            msgh_id: 0,
        },
        NDR_record: NDR_record_t {
            mig_vers: 0,
            if_vers: 0,
            reserved1: 0,
            mig_encoding: 0,
            int_rep: 0,
            char_rep: 0,
            float_rep: 0,
            reserved2: 0,
        },
        window_id: 0,
        sub_level: 0,
        padding1: 0,
        padding2: 0,
    };

    message.NDR_record = unsafe { NDR_record };
    message.window_id = window_id.0;
    message.header.msgh_bits = 0x1513;
    message.header.msgh_remote_port =
        unsafe { resolved_cgs_get_connection_port_by_id_function(connection_id) };
    message.header.msgh_local_port = unsafe { mig_get_special_reply_port() };
    message.header.msgh_id = if is_running_on_macos_tahoe() {
        0x76E3
    } else {
        0x73C3
    };
    unsafe {
        mach_msg(
            (&raw mut message).cast::<mach_msg_header_t>(),
            MACH_SEND_MSG | MACH_RCV_MSG,
            0x24,
            0x30,
            message.header.msgh_local_port,
            0,
            0,
        )
    };

    message.sub_level
}

pub(crate) fn query_window_sub_level_from_window_server(window_id: WindowId) -> i32 {
    if resolved_cgs_get_connection_port_by_id_function().is_some() {
        query_window_sub_level_with_a_raw_mach_message(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            window_id,
        )
    } else {
        unsafe { SLSGetWindowSubLevel(*SKYLIGHT_CONNECTION_ID.get().unwrap(), window_id.0) }
    }
}

pub(crate) fn query_window_tags_from_window_server(window_id: WindowId) -> u64 {
    let mut tags: u64 = 0;
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let window_ref = create_cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);

    let query =
        unsafe { take_create_rule_result(SLSWindowQueryWindows(connection_id, &*window_ref, 1)) };
    if let Some(query) = query {
        let iterator = unsafe { take_create_rule_result(SLSWindowQueryResultCopyWindows(&*query)) };
        if let Some(iterator) = iterator {
            if unsafe { SLSWindowIteratorGetCount(&*iterator) } == 1 {
                if unsafe { SLSWindowIteratorAdvance(&*iterator) } {
                    tags = unsafe { SLSWindowIteratorGetTags(&*iterator) };
                }
            }

            drop(iterator);
        }
        drop(query);
    }
    drop(window_ref);

    tags
}

pub(crate) fn copy_window_role_through_accessibility(window: &Window) -> Option<CFStringOwned> {
    let mut role: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXRoleAttribute(),
            NonNull::from(&mut role),
        )
    };
    unsafe { take_create_rule_result(role.cast::<CFString>()) }.map(CFRetainedAssumedSendAndSync)
}

pub(crate) fn window_role(window: &Window) -> Option<&CFString> {
    window.role.as_ref().map(|role| role.as_ref())
}

pub(crate) fn window_role_as_string(window: &Window) -> String {
    let Some(role) = window_role(window) else {
        return String::new();
    };

    let result = cfstring_to_string(role);
    result.unwrap_or_default()
}

pub(crate) fn copy_window_subrole_through_accessibility(window: &Window) -> Option<CFStringOwned> {
    let mut subrole: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXSubroleAttribute(),
            NonNull::from(&mut subrole),
        )
    };
    unsafe { take_create_rule_result(subrole.cast::<CFString>()) }.map(CFRetainedAssumedSendAndSync)
}

pub(crate) fn window_subrole(window: &Window) -> Option<&CFString> {
    window.subrole.as_ref().map(|subrole| subrole.as_ref())
}

pub(crate) fn window_subrole_as_string(window: &Window) -> String {
    let Some(subrole) = window_subrole(window) else {
        return String::new();
    };

    let result = cfstring_to_string(subrole);
    result.unwrap_or_default()
}

pub(crate) fn is_window_root_according_to_accessibility(
    window: &Window,
    window_manager: &mut WindowManager,
) -> bool {
    let mut result = false;
    let mut value: *const CFType = core::ptr::null();

    let error = unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXParentAttribute(),
            NonNull::from(&mut value),
        )
    };
    let value = unsafe { take_create_rule_result(value) };

    if error == kAXErrorSuccess {
        let application_ref = window
            .application
            .and_then(|process_id| window_manager.application.get(&process_id))
            .and_then(|application| unsafe { application.element_ref.as_ref() });

        result = !(match &value {
            Some(value) => !match application_ref {
                Some(application_ref) => CFEqual(Some(value), Some(as_cftype(application_ref))),
                None => false,
            },
            None => false,
        });
    }

    drop(value);
    result
}

pub(crate) fn is_window_a_standard_floating_or_dialog_window(window: &Window) -> bool {
    let mut is_window = false;

    'out: {
        let Some(role) = window_role(window) else {
            break 'out;
        };
        let Some(subrole) = window_subrole(window) else {
            break 'out;
        };

        is_window = CFEqual(Some(as_cftype(role)), Some(as_cftype(kAXWindowRole())))
            && (CFEqual(
                Some(as_cftype(subrole)),
                Some(as_cftype(kAXStandardWindowSubrole())),
            ) || CFEqual(
                Some(as_cftype(subrole)),
                Some(as_cftype(kAXFloatingWindowSubrole())),
            ) || CFEqual(
                Some(as_cftype(subrole)),
                Some(as_cftype(kAXDialogSubrole())),
            ));
    }

    is_window
}

pub(crate) fn is_window_a_standard_window(window: &Window) -> bool {
    let mut standard_window = false;

    'out: {
        let Some(role) = window_role(window) else {
            break 'out;
        };
        let Some(subrole) = window_subrole(window) else {
            break 'out;
        };

        standard_window = CFEqual(Some(as_cftype(role)), Some(as_cftype(kAXWindowRole())))
            && CFEqual(
                Some(as_cftype(subrole)),
                Some(as_cftype(kAXStandardWindowSubrole())),
            );
    }

    standard_window
}

pub(crate) fn is_window_at_normal_window_level(window: &Window) -> bool {
    let level = query_window_level_from_window_server(window.id);
    level == *LAYER_NORMAL_WINDOW_LEVEL.get().unwrap()
}

pub(crate) fn is_window_subrole_unknown(window: &Window) -> bool {
    let Some(subrole) = window_subrole(window) else {
        return false;
    };

    let result = CFEqual(
        Some(as_cftype(subrole)),
        Some(as_cftype(kAXUnknownSubrole())),
    );
    result
}

pub(crate) fn create_window_from_accessibility_element(
    application: ProcessId,
    window_ref: AXUIElementRef,
    window_id: WindowId,
    window_manager: &mut WindowManager,
) -> Window {
    let mut window = Window {
        application: None,
        element_ref: core::ptr::null(),
        id: WindowId(0),
        liveness: Arc::new(WindowLivenessCell {
            window_id,
            state: AtomicU8::new(WINDOW_LIVENESS_ALIVE),
        }),
        liveness_reference_held_by_the_observation: None,
        role: None,
        subrole: None,
        title: None,
        frame: CGRect::ZERO,
        windowed_frame: CGRect::ZERO,
        is_root: false,
        is_eligible: false,
        notification: 0,
        rule_flags: 0,
        flags: 0,
        opacity: 0.0f32,
        layer: 0,
        scratchpad: None,
        stays_a_group_on_its_own: false,
    };

    window.application = Some(application);
    window.element_ref = window_ref;
    window.id = window_id;
    window.frame = read_window_frame_through_accessibility(&window);
    window.role = copy_window_role_through_accessibility(&window);
    window.subrole = copy_window_subrole_through_accessibility(&window);
    window.title = copy_window_title_through_accessibility(&window);
    window.is_root = query_parent_window_from_window_server(window.id) == WindowId(0)
        || is_window_root_according_to_accessibility(&window, window_manager);

    if is_window_shadow_shown_according_to_window_server(window.id) {
        set_window_flag(&mut window, WindowFlag::HAS_SHADOW);
    }

    if is_window_minimized_according_to_accessibility(&window) {
        set_window_flag(&mut window, WindowFlag::MINIMIZED);
    }

    if can_window_be_moved_through_accessibility(&window) {
        set_window_flag(&mut window, WindowFlag::MOVABLE);
    }

    if can_window_be_resized_through_accessibility(&window) {
        set_window_flag(&mut window, WindowFlag::RESIZABLE);
    }

    if (is_window_in_native_fullscreen_according_to_accessibility(&window))
        || (is_native_fullscreen_space(query_space_holding_window(window.id)))
    {
        set_window_flag(&mut window, WindowFlag::IN_NATIVE_FULLSCREEN);
    }

    if is_window_on_more_than_one_space(window.id) {
        set_window_flag(&mut window, WindowFlag::STICKY);
    }

    window
}

pub(crate) fn destroy_window_releasing_its_accessibility_element(mut window: Window) {
    window.id = WindowId(0);
    if window.role.is_some() {
        drop(window.role.take());
    }
    if window.subrole.is_some() {
        drop(window.subrole.take());
    }
    if window.title.is_some() {
        drop(window.title.take());
    }
    drop(unsafe { take_create_rule_result(window.element_ref) });
    drop(window);
}
