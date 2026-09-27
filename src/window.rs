#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::display_manager::{DisplayManager, display_manager_display_id_arrangement};
use crate::ffi::CFStringOwned;
use crate::ffi::accessibility::{
    AXError, AXObserver, AXObserverAddNotification, AXObserverRemoveNotification, AXUIElement,
    AXUIElementCopyAttributeValue, AXUIElementIsAttributeSettable, AXUIElementRef, AXValue,
    AXValueGetValue, AXValueType, ax_error_str, kAXDialogSubrole, kAXErrorSuccess,
    kAXFloatingWindowSubrole, kAXFullscreenAttribute, kAXMinimizedAttribute, kAXParentAttribute,
    kAXPositionAttribute, kAXRoleAttribute, kAXSizeAttribute, kAXStandardWindowSubrole,
    kAXSubroleAttribute, kAXTitleAttribute, kAXUIElementDestroyedNotification,
    kAXUnknownSubrole, kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
    kAXWindowRole,
};
use crate::ffi::color_sync::CGDisplayGetDisplayIDFromUUID;
use crate::ffi::core_foundation::{
    CFBoolean, CFEqual, CFIndex, CFNumber, CFRetained, CFString, CFType, CFUUIDCreateFromString,
    CGPoint, CGRect, SendCFRetained, as_cftype, cfarray_borrow_value_at_index, cfarray_count,
    cfarray_of_cfnumbers, cfboolean_get_value, cfnumber_read_u64_widening, k_cgs_window_title,
    kCFNumberSInt32Type, take_create_rule_result, ts_cfstring_copy,
};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::ffi::libsystem::{PROC_PIDPATHINFO_MAXSIZE, proc_name};
use crate::ffi::mach_port::{
    MACH_RCV_MSG, MACH_SEND_MSG, NDR_record, NDR_record_t, mach_msg, mach_msg_header_t,
    mig_get_special_reply_port,
};
use crate::ffi::skylight::{
    SLSConnectionGetPID, SLSCopyBestManagedDisplayForRect, SLSCopyManagedDisplayForWindow,
    SLSCopySpacesForWindows, SLSCopyWindowProperty, SLSGetWindowAlpha, SLSGetWindowBounds,
    SLSGetWindowLevel, SLSGetWindowOwner, SLSGetWindowSubLevel, SLSManagedDisplayGetCurrentSpace,
    SLSWindowIsOrderedIn, SLSWindowIteratorAdvance, SLSWindowIteratorGetCount,
    SLSWindowIteratorGetLevel, SLSWindowIteratorGetParentID, SLSWindowIteratorGetTags,
    SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows,
};
use crate::ffi::skylight_dynamic::cgs_get_connection_port_by_id;
use crate::globals::{
    CONNECTION, LAYER_ABOVE_WINDOW_LEVEL, LAYER_BELOW_WINDOW_LEVEL, LAYER_NORMAL_WINDOW_LEVEL,
};
use crate::handles::{DisplayId, NodeId, ProcessId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::misc::helpers::{LAYER_STR, json_bool, ts_string_escape};
use crate::misc::macros::{LAYER_ABOVE, LAYER_BELOW, LAYER_NORMAL};
use crate::misc::response::Response;
use crate::space::{space_display_id, space_is_fullscreen, space_is_visible};
use crate::space_manager::{SpaceManager, space_manager_mission_control_index};
use crate::state::MouseDragState;
use crate::view::{
    WINDOW_NODE_CHILD_STR, WINDOW_NODE_SPLIT_STR, WindowNodeChild, view_find_window_node,
    window_node_index_of_window, window_node_is_left_child,
};
use crate::window_manager::{WindowManager, window_manager_find_managed_window};
use crate::workspace::{
    workspace_is_macos_sequoia, workspace_is_macos_sonoma, workspace_is_macos_tahoe,
    workspace_is_macos_ventura,
};

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
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowFlag(pub u8);

impl WindowFlag {
    pub(crate) const SHADOW: WindowFlag = WindowFlag(0x01);
    pub(crate) const FULLSCREEN: WindowFlag = WindowFlag(0x02);
    pub(crate) const MINIMIZE: WindowFlag = WindowFlag(0x04);
    pub(crate) const FLOAT: WindowFlag = WindowFlag(0x08);
    pub(crate) const STICKY: WindowFlag = WindowFlag(0x10);
    pub(crate) const WINDOWED: WindowFlag = WindowFlag(0x20);
    pub(crate) const MOVABLE: WindowFlag = WindowFlag(0x40);
    pub(crate) const RESIZABLE: WindowFlag = WindowFlag(0x80);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowRuleFlag(pub u8);

impl WindowRuleFlag {
    pub(crate) const MANAGED: WindowRuleFlag = WindowRuleFlag(0x01);
    pub(crate) const FULLSCREEN: WindowRuleFlag = WindowRuleFlag(0x02);
    pub(crate) const MFF: WindowRuleFlag = WindowRuleFlag(0x04);
    pub(crate) const MFF_VALUE: WindowRuleFlag = WindowRuleFlag(0x08);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct AxWindowNotification(pub u8);

impl AxWindowNotification {
    pub(crate) const MINIMIZED: AxWindowNotification =
        AxWindowNotification(1 << AX_WINDOW_MINIMIZED_INDEX);
    pub(crate) const DEMINIMIZED: AxWindowNotification =
        AxWindowNotification(1 << AX_WINDOW_DEMINIMIZED_INDEX);
    pub(crate) const DESTROYED: AxWindowNotification =
        AxWindowNotification(1 << AX_WINDOW_DESTROYED_INDEX);
    pub(crate) const ALL: AxWindowNotification = AxWindowNotification(
        AxWindowNotification::DESTROYED.0
            | AxWindowNotification::MINIMIZED.0
            | AxWindowNotification::DEMINIMIZED.0,
    );
}

pub(crate) struct WindowLivenessCell {
    pub(crate) window_id: WindowId,
    pub(crate) application_process_id: ProcessId,
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
pub(crate) struct SLSGetWindowSubLevelMessage {
    pub(crate) header: mach_msg_header_t,
    pub(crate) NDR_record: NDR_record_t,
    pub(crate) window_id: u32,
    pub(crate) sub_level: i32,
    pub(crate) padding1: i32,
    pub(crate) padding2: i32,
}

const _: () = assert!(core::mem::size_of::<SLSGetWindowSubLevelMessage>() == 0x30);
const _: () = assert!(core::mem::offset_of!(SLSGetWindowSubLevelMessage, window_id) == 0x20);
const _: () = assert!(core::mem::offset_of!(SLSGetWindowSubLevelMessage, sub_level) == 0x24);

macro_rules! window_property_list {
    ($window_property_entry:ident) => {
        $window_property_entry! {
            ("id", WINDOW_PROPERTY_ID, 0x000000001),
            ("pid", WINDOW_PROPERTY_PID, 0x000000002),
            ("app", WINDOW_PROPERTY_APP, 0x000000004),
            ("title", WINDOW_PROPERTY_TITLE, 0x000000008),
            ("scratchpad", WINDOW_PROPERTY_SCRATCHPAD, 0x000000010),
            ("frame", WINDOW_PROPERTY_FRAME, 0x000000020),
            ("role", WINDOW_PROPERTY_ROLE, 0x000000040),
            ("subrole", WINDOW_PROPERTY_SUBROLE, 0x000000080),
            ("root-window", WINDOW_PROPERTY_ROOT_WINDOW, 0x000000100),
            ("display", WINDOW_PROPERTY_DISPLAY, 0x000000200),
            ("space", WINDOW_PROPERTY_SPACE, 0x000000400),
            ("level", WINDOW_PROPERTY_LEVEL, 0x000000800),
            ("sub-level", WINDOW_PROPERTY_SUB_LEVEL, 0x000001000),
            ("layer", WINDOW_PROPERTY_LAYER, 0x000002000),
            ("sub-layer", WINDOW_PROPERTY_SUB_LAYER, 0x000004000),
            ("opacity", WINDOW_PROPERTY_OPACITY, 0x000008000),
            ("split-type", WINDOW_PROPERTY_SPLIT_TYPE, 0x000010000),
            ("split-child", WINDOW_PROPERTY_SPLIT_CHILD, 0x000020000),
            ("stack-index", WINDOW_PROPERTY_STACK_INDEX, 0x000040000),
            ("can-move", WINDOW_PROPERTY_CAN_MOVE, 0x000080000),
            ("can-resize", WINDOW_PROPERTY_CAN_RESIZE, 0x000100000),
            ("has-focus", WINDOW_PROPERTY_HAS_FOCUS, 0x000200000),
            ("has-shadow", WINDOW_PROPERTY_HAS_SHADOW, 0x000400000),
            ("has-parent-zoom", WINDOW_PROPERTY_HAS_PARENT_ZOOM, 0x000800000),
            ("has-fullscreen-zoom", WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM, 0x001000000),
            ("has-ax-reference", WINDOW_PROPERTY_HAS_AX_REFERENCE, 0x002000000),
            ("is-native-fullscreen", WINDOW_PROPERTY_IS_FULLSCREEN, 0x004000000),
            ("is-visible", WINDOW_PROPERTY_IS_VISIBLE, 0x008000000),
            ("is-minimized", WINDOW_PROPERTY_IS_MINIMIZED, 0x010000000),
            ("is-hidden", WINDOW_PROPERTY_IS_HIDDEN, 0x020000000),
            ("is-floating", WINDOW_PROPERTY_IS_FLOATING, 0x040000000),
            ("is-sticky", WINDOW_PROPERTY_IS_STICKY, 0x080000000),
            ("is-grabbed", WINDOW_PROPERTY_IS_GRABBED, 0x100000000),
        }
    };
}

macro_rules! define_window_property_list {
    ($(($name:literal, $identifier:ident, $value:literal)),* $(,)?) => {
        $(pub(crate) const $identifier: u64 = $value;)*

        pub(crate) static WINDOW_PROPERTY_VAL: [u64; 33] = [$($value),*];

        pub(crate) static WINDOW_PROPERTY_STR: [&str; 33] = [$($name),*];
    };
}

window_property_list!(define_window_property_list);

pub(crate) const AX_WINDOW_MINIMIZED_INDEX: usize = 0;
pub(crate) const AX_WINDOW_DEMINIMIZED_INDEX: usize = 1;
pub(crate) const AX_WINDOW_DESTROYED_INDEX: usize = 2;

pub(crate) static AX_WINDOW_NOTIFICATION_STR: [&str; 3] = [
    "kAXWindowMiniaturizedNotification",
    "kAXWindowDeminiaturizedNotification",
    "kAXUIElementDestroyedNotification",
];

pub(crate) static AX_WINDOW_NOTIFICATION: OnceLock<[CFStringOwned; 3]> = OnceLock::new();

pub(crate) fn ax_window_notification() -> &'static [CFStringOwned; 3] {
    AX_WINDOW_NOTIFICATION.get_or_init(|| {
        [
            SendCFRetained(unsafe {
                CFRetained::retain(NonNull::from(kAXWindowMiniaturizedNotification()))
            }),
            SendCFRetained(unsafe {
                CFRetained::retain(NonNull::from(kAXWindowDeminiaturizedNotification()))
            }),
            SendCFRetained(unsafe {
                CFRetained::retain(NonNull::from(kAXUIElementDestroyedNotification()))
            }),
        ]
    })
}

pub(crate) fn window_check_flag(window: &Window, flag: WindowFlag) -> bool {
    (window.flags & flag.0) != 0
}

pub(crate) fn window_clear_flag(window: &mut Window, flag: WindowFlag) {
    window.flags &= !flag.0;
}

pub(crate) fn window_set_flag(window: &mut Window, flag: WindowFlag) {
    window.flags |= flag.0;
}

pub(crate) fn window_check_rule_flag(window: &Window, flag: WindowRuleFlag) -> bool {
    (window.rule_flags & flag.0) != 0
}

pub(crate) fn window_clear_rule_flag(window: &mut Window, flag: WindowRuleFlag) {
    window.rule_flags &= !flag.0;
}

pub(crate) fn window_set_rule_flag(window: &mut Window, flag: WindowRuleFlag) {
    window.rule_flags |= flag.0;
}

pub(crate) fn window_observe(window: &mut Window, window_manager: &mut WindowManager) -> bool {
    let observer_ref = window
        .application
        .and_then(|process_id| window_manager.application.find(&process_id))
        .map(|application| application.observer_ref);
    let Some(observer) = observer_ref.and_then(|observer_ref| unsafe { observer_ref.as_ref() })
    else {
        return false;
    };

    let element_ref = window.element_ref;
    let Some(element) = (unsafe { element_ref.as_ref() }) else {
        return false;
    };

    let liveness_reference = Arc::into_raw(Arc::clone(&window.liveness));
    window.liveness_reference_held_by_the_observation = Some(liveness_reference);

    for index in 0..ax_window_notification().len() {
        let result = unsafe {
            AXObserverAddNotification(
                observer,
                element,
                ax_window_notification()[index].as_ref(),
                liveness_reference as *mut c_void,
            )
        };
        if result == kAXErrorSuccess || result == AXError::NotificationAlreadyRegistered {
            window.notification |= 1 << index;
        } else {
            crate::debug!(
                "{}: {} failed with error {}\n",
                "window_observe",
                AX_WINDOW_NOTIFICATION_STR[index],
                ax_error_str(result)
            );
        }
    }

    (window.notification & AxWindowNotification::ALL.0) == AxWindowNotification::ALL.0
}

struct WindowUnobserveRequest {
    observer_ref: Option<SendCFRetained<AXObserver>>,
    window_ref: Option<SendCFRetained<AXUIElement>>,
    notification: u8,
    liveness_reference: *const WindowLivenessCell,
}

pub(crate) fn window_unobserve(window: &mut Window, window_manager: &mut WindowManager) {
    let Some(liveness_reference) = window.liveness_reference_held_by_the_observation.take() else {
        return;
    };

    let observer_ref = window
        .application
        .and_then(|process_id| window_manager.application.find(&process_id))
        .and_then(|application| NonNull::new(application.observer_ref))
        .map(|observer| SendCFRetained(unsafe { CFRetained::retain(observer) }));

    let window_ref = NonNull::new(window.element_ref.cast_mut())
        .map(|element| SendCFRetained(unsafe { CFRetained::retain(element) }));

    let request = Box::into_raw(Box::new(WindowUnobserveRequest {
        observer_ref,
        window_ref,
        notification: std::mem::replace(&mut window.notification, 0),
        liveness_reference,
    }));

    dispatch_after_on_main_queue(0, move || window_unobserve_on_main_queue(request));
}

fn window_unobserve_on_main_queue(request: *mut WindowUnobserveRequest) {
    let request = unsafe { Box::from_raw(request) };

    if let (Some(observer_ref), Some(window_ref)) = (&request.observer_ref, &request.window_ref) {
        for index in 0..ax_window_notification().len() {
            if (request.notification & (1 << index)) == 0 {
                continue;
            }

            unsafe {
                AXObserverRemoveNotification(
                    observer_ref.as_ref(),
                    window_ref.as_ref(),
                    ax_window_notification()[index].as_ref(),
                )
            };
        }
    }

    drop(unsafe { Arc::from_raw(request.liveness_reference) });
}

pub(crate) fn window_display_uuid(window_id: WindowId) -> Option<CFStringOwned> {
    let connection_id = *CONNECTION.get().unwrap();

    let uuid =
        unsafe { take_create_rule_result(SLSCopyManagedDisplayForWindow(connection_id, window_id.0)) };
    if uuid.is_none() {
        let mut frame = CGRect::ZERO;
        unsafe { SLSGetWindowBounds(connection_id, window_id.0, &mut frame) };
        return unsafe {
            take_create_rule_result(SLSCopyBestManagedDisplayForRect(connection_id, frame))
        }
        .map(SendCFRetained);
    }

    uuid.map(SendCFRetained)
}

pub(crate) fn window_display_id(window_id: WindowId) -> DisplayId {
    let Some(uuid_string) = window_display_uuid(window_id) else {
        return DisplayId(0);
    };

    let Some(uuid) = CFUUIDCreateFromString(None, Some(uuid_string.as_ref())) else {
        return DisplayId(0);
    };
    let id = unsafe { CGDisplayGetDisplayIDFromUUID(&*uuid) } as i32;

    DisplayId(id as u32)
}

pub(crate) fn window_display_space(window_id: WindowId) -> SpaceId {
    let Some(uuid) = window_display_uuid(window_id) else {
        return SpaceId(0);
    };

    let space_id =
        unsafe { SLSManagedDisplayGetCurrentSpace(*CONNECTION.get().unwrap(), uuid.as_ref()) };

    SpaceId(space_id)
}

pub(crate) fn window_space(window_id: WindowId) -> SpaceId {
    let mut space_id: u64 = 0;

    let connection_id = *CONNECTION.get().unwrap();
    let window_list_ref = cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);
    let space_list_ref = unsafe {
        take_create_rule_result(SLSCopySpacesForWindows(connection_id, 0x7, &*window_list_ref))
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
        window_display_space(window_id)
    }
}

pub(crate) fn window_space_list(window_id: WindowId) -> Vec<SpaceId> {
    let mut space_list: Vec<SpaceId> = Vec::new();

    let connection_id = *CONNECTION.get().unwrap();
    let window_list_ref = cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);
    let Some(space_list_ref) = (unsafe {
        take_create_rule_result(SLSCopySpacesForWindows(connection_id, 0x7, &*window_list_ref))
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

pub(crate) fn window_layer(level: i32) -> &'static str {
    if level == *LAYER_BELOW_WINDOW_LEVEL.get().unwrap() {
        return LAYER_STR[LAYER_BELOW as usize].unwrap();
    }
    if level == *LAYER_NORMAL_WINDOW_LEVEL.get().unwrap() {
        return LAYER_STR[LAYER_NORMAL as usize].unwrap();
    }
    if level == *LAYER_ABOVE_WINDOW_LEVEL.get().unwrap() {
        return LAYER_STR[LAYER_ABOVE as usize].unwrap();
    }
    "unknown"
}

pub(crate) fn window_nonax_serialize(
    response: &mut Response,
    window_id: WindowId,
    flags: u64,
    display_manager: &mut DisplayManager,
) {
    let mut flags = flags;
    if flags == 0x0 {
        flags |= !flags;
    }

    let connection_id = *CONNECTION.get().unwrap();

    let mut process_id: Option<libc::pid_t> = None;
    let mut space_id: Option<SpaceId> = None;
    let mut level: Option<i32> = None;
    let mut sub_level: Option<i32> = None;

    if (flags & WINDOW_PROPERTY_PID) != 0 || (flags & WINDOW_PROPERTY_APP) != 0 {
        let mut connection: i32 = 0;
        unsafe { SLSGetWindowOwner(connection_id, window_id.0, &mut connection) };
        let mut value: libc::pid_t = 0;
        unsafe { SLSConnectionGetPID(connection, &mut value) };
        process_id = Some(value);
    }

    if (flags & WINDOW_PROPERTY_DISPLAY) != 0
        || (flags & WINDOW_PROPERTY_SPACE) != 0
        || (flags & WINDOW_PROPERTY_IS_FULLSCREEN) != 0
    {
        space_id = Some(window_space(window_id));
    }

    if (flags & WINDOW_PROPERTY_LEVEL) != 0 || (flags & WINDOW_PROPERTY_LAYER) != 0 {
        level = Some(window_level(window_id));
    }

    if (flags & WINDOW_PROPERTY_SUB_LEVEL) != 0 || (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        sub_level = Some(window_sub_level(window_id));
    }

    let mut did_output = false;
    response.write(format_args!("{{\n"));

    if (flags & WINDOW_PROPERTY_ID) != 0 {
        response.write(format_args!("\t\"id\":{}", window_id.0 as i32));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_PID) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"pid\":{}", process_id.unwrap()));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_APP) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        static PROCESS_NAME: Mutex<[u8; PROC_PIDPATHINFO_MAXSIZE]> =
            Mutex::new([0u8; PROC_PIDPATHINFO_MAXSIZE]);
        let mut process_name = PROCESS_NAME.lock().unwrap();
        unsafe {
            proc_name(
                process_id.unwrap(),
                process_name.as_mut_ptr().cast::<c_void>(),
                process_name.len() as u32,
            )
        };

        let end = process_name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(process_name.len());
        let application_name = String::from_utf8_lossy(&process_name[..end]).into_owned();
        drop(process_name);
        let escaped_application_name = ts_string_escape(&application_name);

        response.write(format_args!(
            "\t\"app\":\"{}\"",
            escaped_application_name.as_deref().unwrap_or(&application_name)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_TITLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let title = window_property_title_ts(window_id);
        let escaped_title = ts_string_escape(&title);

        response.write(format_args!(
            "\t\"title\":\"{}\"",
            escaped_title.as_deref().unwrap_or(&title)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SCRATCHPAD) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"scratchpad\":\"{}\"", ""));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_FRAME) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut frame = CGRect::ZERO;
        unsafe { SLSGetWindowBounds(connection_id, window_id.0, &mut frame) };

        response.write(format_args!(
            "\t\"frame\":{{\n\t\t\"x\":{:.4},\n\t\t\"y\":{:.4},\n\t\t\"w\":{:.4},\n\t\t\"h\":{:.4}\n\t}}",
            frame.origin.x, frame.origin.y, frame.size.width, frame.size.height
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"role\":\"{}\"", ""));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUBROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"subrole\":\"{}\"", ""));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROOT_WINDOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let parent_window_id = window_parent(window_id);
        response.write(format_args!(
            "\t\"root-window\":{}",
            json_bool(parent_window_id == WindowId(0))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_DISPLAY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let display =
            display_manager_display_id_arrangement(space_display_id(space_id.unwrap()), display_manager);
        response.write(format_args!("\t\"display\":{}", display));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPACE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let space = space_manager_mission_control_index(space_id.unwrap());
        response.write(format_args!("\t\"space\":{}", space));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_LEVEL) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"level\":{}", level.unwrap()));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUB_LEVEL) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"sub-level\":{}", sub_level.unwrap()));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_LAYER) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let layer = window_layer(level.unwrap());
        response.write(format_args!("\t\"layer\":\"{}\"", layer));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let sub_layer = window_layer(sub_level.unwrap());
        response.write(format_args!("\t\"sub-layer\":\"{}\"", sub_layer));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_OPACITY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let opacity = window_opacity(window_id);
        response.write(format_args!("\t\"opacity\":{:.4}", opacity as f64));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_TYPE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"split-type\":\"{}\"",
            WINDOW_NODE_SPLIT_STR[0]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_CHILD) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"split-child\":\"{}\"",
            WINDOW_NODE_CHILD_STR[WindowNodeChild::None as usize]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_STACK_INDEX) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"stack-index\":{}", 0));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_MOVE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"can-move\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_RESIZE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"can-resize\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FOCUS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-focus\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_SHADOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-shadow\":{}",
            json_bool(window_shadow(window_id))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_PARENT_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-parent-zoom\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-fullscreen-zoom\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_AX_REFERENCE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-ax-reference\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FULLSCREEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let is_fullscreen = space_is_fullscreen(space_id.unwrap());
        response.write(format_args!(
            "\t\"is-native-fullscreen\":{}",
            json_bool(is_fullscreen)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-visible\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_MINIMIZED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-minimized\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_HIDDEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-hidden\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FLOATING) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-floating\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_STICKY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let is_sticky = window_is_sticky(window_id);
        response.write(format_args!("\t\"is-sticky\":{}", json_bool(is_sticky)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_GRABBED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-grabbed\":{}", json_bool(false)));
    }

    response.write(format_args!("\n}}"));
}

pub(crate) fn window_serialize(
    response: &mut Response,
    window_id: WindowId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut flags = flags;
    if flags == 0x0 {
        flags |= !flags;
    }

    let connection_id = *CONNECTION.get().unwrap();

    let mut space_id: Option<SpaceId> = None;
    let mut level: Option<i32> = None;
    let mut sub_level: Option<i32> = None;
    let mut view: Option<SpaceId> = None;
    let mut node: Option<NodeId> = None;
    let mut is_minimized: Option<bool> = None;
    let mut is_sticky: Option<bool> = None;

    if (flags & WINDOW_PROPERTY_DISPLAY) != 0
        || (flags & WINDOW_PROPERTY_SPACE) != 0
        || (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0
    {
        space_id = Some(window_space(window_id));
    }

    if (flags & WINDOW_PROPERTY_LEVEL) != 0 || (flags & WINDOW_PROPERTY_LAYER) != 0 {
        level = Some(window_level(window_id));
    }

    if (flags & WINDOW_PROPERTY_SUB_LEVEL) != 0 || (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        sub_level = Some(window_sub_level(window_id));
    }

    if (flags & WINDOW_PROPERTY_SPLIT_TYPE) != 0
        || (flags & WINDOW_PROPERTY_SPLIT_CHILD) != 0
        || (flags & WINDOW_PROPERTY_STACK_INDEX) != 0
        || (flags & WINDOW_PROPERTY_HAS_PARENT_ZOOM) != 0
        || (flags & WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM) != 0
    {
        view = window_manager_find_managed_window(window_manager, window_id);
        node = match view {
            Some(view) => view_find_window_node(space_manager, view, window_id),
            None => None,
        };
    }

    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 || (flags & WINDOW_PROPERTY_IS_MINIMIZED) != 0 {
        is_minimized = Some(window_check_flag(window, WindowFlag::MINIMIZE));
    }

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 || (flags & WINDOW_PROPERTY_IS_STICKY) != 0 {
        is_sticky =
            Some(window_check_flag(window, WindowFlag::STICKY) || window_is_sticky(window_id));
    }

    let application = window
        .application
        .and_then(|process_id| window_manager.application.find(&process_id));
    let application_is_hidden = application.is_some_and(|application| application.is_hidden);

    let mut did_output = false;
    response.write(format_args!("{{\n"));

    if (flags & WINDOW_PROPERTY_ID) != 0 {
        response.write(format_args!("\t\"id\":{}", window.id.0 as i32));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_PID) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"pid\":{}",
            application.map_or(0, |application| application.process_id.0)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_APP) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let application_name = application.map_or("(null)", |application| &application.name);
        let escaped_application_name = ts_string_escape(application_name);

        response.write(format_args!(
            "\t\"app\":\"{}\"",
            escaped_application_name.as_deref().unwrap_or(application_name)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_TITLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let title = window_title_ts(window);
        let escaped_title = ts_string_escape(&title);

        response.write(format_args!(
            "\t\"title\":\"{}\"",
            escaped_title.as_deref().unwrap_or(&title)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SCRATCHPAD) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"scratchpad\":\"{}\"",
            window.scratchpad.as_deref().unwrap_or("")
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_FRAME) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"frame\":{{\n\t\t\"x\":{:.4},\n\t\t\"y\":{:.4},\n\t\t\"w\":{:.4},\n\t\t\"h\":{:.4}\n\t}}",
            window.frame.origin.x,
            window.frame.origin.y,
            window.frame.size.width,
            window.frame.size.height
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let role = window_role_ts(window);
        response.write(format_args!("\t\"role\":\"{}\"", role));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUBROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let subrole = window_subrole_ts(window);
        response.write(format_args!("\t\"subrole\":\"{}\"", subrole));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROOT_WINDOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"root-window\":{}",
            json_bool(window.is_root)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_DISPLAY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let display =
            display_manager_display_id_arrangement(space_display_id(space_id.unwrap()), display_manager);
        response.write(format_args!("\t\"display\":{}", display));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPACE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let space = space_manager_mission_control_index(space_id.unwrap());
        response.write(format_args!("\t\"space\":{}", space));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_LEVEL) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"level\":{}", level.unwrap()));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUB_LEVEL) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"sub-level\":{}", sub_level.unwrap()));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_LAYER) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let layer = window_layer(level.unwrap());
        response.write(format_args!("\t\"layer\":\"{}\"", layer));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let sub_layer = window_layer(sub_level.unwrap());
        response.write(format_args!("\t\"sub-layer\":\"{}\"", sub_layer));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_OPACITY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let opacity = window_opacity(window.id);
        response.write(format_args!("\t\"opacity\":{:.4}", opacity as f64));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_TYPE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut split_type: usize = 0;
        if let (Some(view), Some(node)) = (view, node) {
            if let Some(view) = space_manager.view.find(&view) {
                if let Some(parent) = view.find_node(node).and_then(|node| node.parent) {
                    if let Some(parent) = view.find_node(parent) {
                        split_type = parent.split as usize;
                    }
                }
            }
        }

        response.write(format_args!(
            "\t\"split-type\":\"{}\"",
            WINDOW_NODE_SPLIT_STR[split_type]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_CHILD) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let split_child = match (view, node) {
            (Some(view), Some(node)) => {
                if window_node_is_left_child(view, node, space_manager) {
                    WindowNodeChild::First as usize
                } else {
                    WindowNodeChild::Second as usize
                }
            }
            _ => WindowNodeChild::None as usize,
        };

        response.write(format_args!(
            "\t\"split-child\":\"{}\"",
            WINDOW_NODE_CHILD_STR[split_child]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_STACK_INDEX) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut stack_index: i32 = 0;
        if let (Some(view), Some(node)) = (view, node) {
            let window_count = space_manager
                .view
                .find(&view)
                .and_then(|view| view.find_node(node))
                .map_or(0, |node| node.window_count);
            if window_count > 1 {
                stack_index =
                    window_node_index_of_window(view, node, window.id, space_manager) + 1;
            }
        }

        response.write(format_args!("\t\"stack-index\":{}", stack_index));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_MOVE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"can-move\":{}",
            json_bool(window_can_move(window))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_RESIZE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"can-resize\":{}",
            json_bool(window_can_resize(window))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FOCUS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-focus\":{}",
            json_bool(window.id == window_manager.focused_window_id)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_SHADOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-shadow\":{}",
            json_bool(window_shadow(window.id))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_PARENT_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut zoom_parent = false;
        if let (Some(view), Some(node)) = (view, node) {
            if let Some(node) = space_manager
                .view
                .find(&view)
                .and_then(|view| view.find_node(node))
            {
                zoom_parent = node.zoom.is_some() && node.zoom == node.parent;
            }
        }

        response.write(format_args!(
            "\t\"has-parent-zoom\":{}",
            json_bool(zoom_parent)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut zoom_fullscreen = false;
        if let (Some(view), Some(node)) = (view, node) {
            if let Some(node) = space_manager
                .view
                .find(&view)
                .and_then(|view| view.find_node(node))
            {
                zoom_fullscreen = node.zoom.is_some() && node.zoom == Some(ROOT_NODE_ID);
            }
        }

        response.write(format_args!(
            "\t\"has-fullscreen-zoom\":{}",
            json_bool(zoom_fullscreen)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_AX_REFERENCE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-ax-reference\":{}", json_bool(true)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FULLSCREEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-native-fullscreen\":{}",
            json_bool(window_check_flag(window, WindowFlag::FULLSCREEN))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut ordered_in: u8 = 0;
        unsafe { SLSWindowIsOrderedIn(connection_id, window.id.0, &mut ordered_in) };

        let visible = ordered_in != 0
            && !is_minimized.unwrap()
            && !application_is_hidden
            && (is_sticky.unwrap() || space_is_visible(space_id.unwrap()));
        response.write(format_args!("\t\"is-visible\":{}", json_bool(visible)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_MINIMIZED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-minimized\":{}",
            json_bool(is_minimized.unwrap())
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_HIDDEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-hidden\":{}",
            json_bool(application_is_hidden)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FLOATING) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-floating\":{}",
            json_bool(window_check_flag(window, WindowFlag::FLOAT))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_STICKY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-sticky\":{}",
            json_bool(is_sticky.unwrap())
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_GRABBED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let grabbed = mouse_drag_state.window_id == Some(window.id);
        response.write(format_args!("\t\"is-grabbed\":{}", json_bool(grabbed)));
    }

    response.write(format_args!("\n}}"));
}

pub(crate) fn window_property_title_ts(window_id: WindowId) -> String {
    let mut value: *mut CFType = core::ptr::null_mut();
    unsafe {
        SLSCopyWindowProperty(
            *CONNECTION.get().unwrap(),
            window_id.0,
            k_cgs_window_title(),
            &mut value,
        )
    };
    let Some(value) = (unsafe { take_create_rule_result(value.cast_const()) }) else {
        return String::new();
    };

    let result = ts_cfstring_copy(unsafe { &*((&*value as *const CFType).cast::<CFString>()) });
    drop(value);
    result.unwrap_or_default()
}

pub(crate) fn window_title_ts(window: &Window) -> String {
    match &window.title {
        Some(title) => ts_cfstring_copy(title.as_ref()).unwrap_or_default(),
        None => String::new(),
    }
}

pub(crate) fn window_title(window: &Window) -> Option<CFStringOwned> {
    let mut value: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXTitleAttribute(),
            NonNull::from(&mut value),
        )
    };
    unsafe { take_create_rule_result(value.cast::<CFString>()) }.map(SendCFRetained)
}

pub(crate) fn window_ax_origin(window: &Window) -> CGPoint {
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

pub(crate) fn window_ax_frame(window: &Window) -> CGRect {
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

pub(crate) fn window_ax_can_move(window: &Window) -> bool {
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

pub(crate) fn window_can_move(window: &Window) -> bool {
    window_check_flag(window, WindowFlag::MOVABLE)
}

pub(crate) fn window_ax_can_resize(window: &Window) -> bool {
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

pub(crate) fn window_can_resize(window: &Window) -> bool {
    window_check_flag(window, WindowFlag::RESIZABLE)
}

pub(crate) fn window_can_minimize(window: &Window) -> bool {
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

pub(crate) fn window_is_undersized(window: &Window) -> bool {
    if window.frame.size.width <= 500.0f32 as f64 {
        return true;
    }
    if window.frame.size.height <= 500.0f32 as f64 {
        return true;
    }
    false
}

pub(crate) fn window_is_minimized(window: &Window) -> bool {
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
            result = cfboolean_get_value(unsafe {
                &*((&*value as *const CFType).cast::<CFBoolean>())
            });
            drop(value);
        }
    }

    result
}

pub(crate) fn window_is_fullscreen(window: &Window) -> bool {
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
            result = cfboolean_get_value(unsafe {
                &*((&*value as *const CFType).cast::<CFBoolean>())
            });
            drop(value);
        }
    }

    result
}

pub(crate) fn window_is_sticky(window_id: WindowId) -> bool {
    let mut result = false;

    let window_list_ref = cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);
    let space_list_ref = unsafe {
        take_create_rule_result(SLSCopySpacesForWindows(
            *CONNECTION.get().unwrap(),
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

pub(crate) fn window_shadow(window_id: WindowId) -> bool {
    let tags = window_tags(window_id);
    (tags & 0x8) == 0
}

pub(crate) fn window_opacity(window_id: WindowId) -> f32 {
    let mut alpha: f32 = 0.0f32;
    unsafe { SLSGetWindowAlpha(*CONNECTION.get().unwrap(), window_id.0, &mut alpha) };
    alpha
}

pub(crate) fn window_parent(window_id: WindowId) -> WindowId {
    let mut parent_window_id: u32 = 0;
    let connection_id = *CONNECTION.get().unwrap();

    let window_ref = cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);

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

pub(crate) fn window_level(window_id: WindowId) -> i32 {
    let mut level: i32 = 0;
    let connection_id = *CONNECTION.get().unwrap();

    if workspace_is_macos_ventura()
        || workspace_is_macos_sonoma()
        || workspace_is_macos_sequoia()
        || workspace_is_macos_tahoe()
    {
        let window_ref = cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);

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
pub(crate) fn SLSGetWindowSubLevel__Internal(connection_id: i32, window_id: WindowId) -> i32 {
    let Some(cgs_get_connection_port_by_id) = cgs_get_connection_port_by_id() else {
        return 0;
    };

    let mut message = SLSGetWindowSubLevelMessage {
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
    message.header.msgh_remote_port = unsafe { cgs_get_connection_port_by_id(connection_id) };
    message.header.msgh_local_port = unsafe { mig_get_special_reply_port() };
    message.header.msgh_id = if workspace_is_macos_tahoe() {
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

pub(crate) fn window_sub_level(window_id: WindowId) -> i32 {
    if cgs_get_connection_port_by_id().is_some() {
        SLSGetWindowSubLevel__Internal(*CONNECTION.get().unwrap(), window_id)
    } else {
        unsafe { SLSGetWindowSubLevel(*CONNECTION.get().unwrap(), window_id.0) }
    }
}

pub(crate) fn window_tags(window_id: WindowId) -> u64 {
    let mut tags: u64 = 0;
    let connection_id = *CONNECTION.get().unwrap();
    let window_ref = cfarray_of_cfnumbers(&[window_id.0], kCFNumberSInt32Type);

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

pub(crate) fn window_ax_role(window: &Window) -> Option<CFStringOwned> {
    let mut role: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXRoleAttribute(),
            NonNull::from(&mut role),
        )
    };
    unsafe { take_create_rule_result(role.cast::<CFString>()) }.map(SendCFRetained)
}

pub(crate) fn window_role(window: &Window) -> Option<&CFString> {
    window.role.as_ref().map(|role| role.as_ref())
}

pub(crate) fn window_role_ts(window: &Window) -> String {
    let Some(role) = window_role(window) else {
        return String::new();
    };

    let result = ts_cfstring_copy(role);
    result.unwrap_or_default()
}

pub(crate) fn window_ax_subrole(window: &Window) -> Option<CFStringOwned> {
    let mut subrole: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &*window.element_ref,
            kAXSubroleAttribute(),
            NonNull::from(&mut subrole),
        )
    };
    unsafe { take_create_rule_result(subrole.cast::<CFString>()) }.map(SendCFRetained)
}

pub(crate) fn window_subrole(window: &Window) -> Option<&CFString> {
    window.subrole.as_ref().map(|subrole| subrole.as_ref())
}

pub(crate) fn window_subrole_ts(window: &Window) -> String {
    let Some(subrole) = window_subrole(window) else {
        return String::new();
    };

    let result = ts_cfstring_copy(subrole);
    result.unwrap_or_default()
}

pub(crate) fn window_is_root(window: &Window, window_manager: &mut WindowManager) -> bool {
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
            .and_then(|process_id| window_manager.application.find(&process_id))
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

pub(crate) fn window_is_real(window: &Window) -> bool {
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
            ) || CFEqual(Some(as_cftype(subrole)), Some(as_cftype(kAXDialogSubrole()))));
    }

    is_window
}

pub(crate) fn window_is_standard(window: &Window) -> bool {
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

pub(crate) fn window_level_is_standard(window: &Window) -> bool {
    let level = window_level(window.id);
    level == *LAYER_NORMAL_WINDOW_LEVEL.get().unwrap()
}

pub(crate) fn window_is_unknown(window: &Window) -> bool {
    let Some(subrole) = window_subrole(window) else {
        return false;
    };

    let result = CFEqual(
        Some(as_cftype(subrole)),
        Some(as_cftype(kAXUnknownSubrole())),
    );
    result
}

pub(crate) fn window_create(
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
            application_process_id: application,
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
    };

    window.application = Some(application);
    window.element_ref = window_ref;
    window.id = window_id;
    window.frame = window_ax_frame(&window);
    window.role = window_ax_role(&window);
    window.subrole = window_ax_subrole(&window);
    window.title = window_title(&window);
    window.is_root = window_parent(window.id) == WindowId(0) || window_is_root(&window, window_manager);

    if window_shadow(window.id) {
        window_set_flag(&mut window, WindowFlag::SHADOW);
    }

    if window_is_minimized(&window) {
        window_set_flag(&mut window, WindowFlag::MINIMIZE);
    }

    if window_ax_can_move(&window) {
        window_set_flag(&mut window, WindowFlag::MOVABLE);
    }

    if window_ax_can_resize(&window) {
        window_set_flag(&mut window, WindowFlag::RESIZABLE);
    }

    if (window_is_fullscreen(&window)) || (space_is_fullscreen(window_space(window.id))) {
        window_set_flag(&mut window, WindowFlag::FULLSCREEN);
    }

    if window_is_sticky(window.id) {
        window_set_flag(&mut window, WindowFlag::STICKY);
    }

    window
}

pub(crate) fn window_destroy(mut window: Window) {
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
