#![allow(deprecated)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use core::ptr::NonNull;
use std::sync::OnceLock;

use objc2_core_foundation::{CFBoolean, CFData, CFDictionary, CFString, CFType};

pub use objc2_application_services::{
    AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXObserver,
    AXObserverAddNotification, AXObserverCreate, AXObserverGetRunLoopSource,
    AXObserverRemoveNotification, AXUIElement, AXUIElementCopyAttributeValue,
    AXUIElementCopyElementAtPosition, AXUIElementCreateApplication, AXUIElementCreateSystemWide,
    AXUIElementIsAttributeSettable, AXUIElementPerformAction, AXUIElementSetAttributeValue,
    AXUIElementSetMessagingTimeout, AXValue, AXValueCreate, AXValueGetValue, AXValueType,
    kAXTrustedCheckOptionPrompt,
};

use crate::ffi::core_foundation::{CFRetainedAssumedSendAndSync, take_create_rule_result};

pub type AXUIElementRef = *const AXUIElement;
pub type AXObserverRef = *mut AXObserver;

pub const kAXErrorSuccess: AXError = AXError::Success;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn _AXUIElementGetWindow(element_ref: *const AXUIElement, window_id: *mut u32) -> AXError;
    pub fn _AXUIElementCreateWithRemoteToken(data: *const CFData) -> *mut AXUIElement;
}

macro_rules! define_cached_accessibility_cfstring_constants {
    ($($constant_name:ident => $string_value:literal,)*) => {
        $(
            pub fn $constant_name() -> &'static CFString {
                static CACHED_ACCESSIBILITY_CFSTRING: OnceLock<CFRetainedAssumedSendAndSync<CFString>> =
                    OnceLock::new();
                CACHED_ACCESSIBILITY_CFSTRING
                    .get_or_init(|| {
                        CFRetainedAssumedSendAndSync(CFString::from_static_str($string_value))
                    })
                    .as_ref()
            }
        )*
    };
}

define_cached_accessibility_cfstring_constants! {
    kAXWindowsAttribute                 => "AXWindows",
    kAXFocusedWindowAttribute           => "AXFocusedWindow",
    kAXWindowAttribute                  => "AXWindow",
    kAXPositionAttribute                => "AXPosition",
    kAXSizeAttribute                    => "AXSize",
    kAXRoleAttribute                    => "AXRole",
    kAXSubroleAttribute                 => "AXSubrole",
    kAXTitleAttribute                   => "AXTitle",
    kAXMinimizedAttribute               => "AXMinimized",
    kAXCloseButtonAttribute             => "AXCloseButton",
    kAXParentAttribute                  => "AXParent",
    kAXWindowRole                       => "AXWindow",
    kAXDrawerRole                       => "AXDrawer",
    kAXSheetRole                        => "AXSheet",
    kAXStandardWindowSubrole            => "AXStandardWindow",
    kAXDialogSubrole                    => "AXDialog",
    kAXFloatingWindowSubrole            => "AXFloatingWindow",
    kAXUnknownSubrole                   => "AXUnknown",
    kAXPressAction                      => "AXPress",
    kAXRaiseAction                      => "AXRaise",
    kAXCreatedNotification              => "AXCreated",
    kAXFocusedWindowChangedNotification => "AXFocusedWindowChanged",
    kAXWindowMovedNotification          => "AXWindowMoved",
    kAXWindowResizedNotification        => "AXWindowResized",
    kAXTitleChangedNotification         => "AXTitleChanged",
    kAXMenuOpenedNotification           => "AXMenuOpened",
    kAXMenuClosedNotification           => "AXMenuClosed",
    kAXWindowMiniaturizedNotification   => "AXWindowMiniaturized",
    kAXWindowDeminiaturizedNotification => "AXWindowDeminiaturized",
    kAXUIElementDestroyedNotification   => "AXUIElementDestroyed",
    kAXFullscreenAttribute              => "AXFullScreen",
    kAXEnhancedUserInterface            => "AXEnhancedUserInterface",
    kAXExposeShowAllWindows             => "AXExposeShowAllWindows",
    kAXExposeShowFrontWindows           => "AXExposeShowFrontWindows",
    kAXExposeShowDesktop                => "AXExposeShowDesktop",
    kAXExposeExit                       => "AXExposeExit",
}

pub fn accessibility_error_constant_name(error: AXError) -> &'static str {
    match error {
        AXError::Success => "kAXErrorSuccess",
        AXError::Failure => "kAXErrorFailure",
        AXError::IllegalArgument => "kAXErrorIllegalArgument",
        AXError::InvalidUIElement => "kAXErrorInvalidUIElement",
        AXError::InvalidUIElementObserver => "kAXErrorInvalidUIElementObserver",
        AXError::CannotComplete => "kAXErrorCannotComplete",
        AXError::AttributeUnsupported => "kAXErrorAttributeUnsupported",
        AXError::ActionUnsupported => "kAXErrorActionUnsupported",
        AXError::NotificationUnsupported => "kAXErrorNotificationUnsupported",
        AXError::NotImplemented => "kAXErrorNotImplemented",
        AXError::NotificationAlreadyRegistered => "kAXErrorNotificationAlreadyRegistered",
        AXError::NotificationNotRegistered => "kAXErrorNotificationNotRegistered",
        AXError::APIDisabled => "kAXErrorAPIDisabled",
        AXError::NoValue => "kAXErrorNoValue",
        AXError::ParameterizedAttributeUnsupported => "kAXErrorParameterizedAttributeUnsupported",
        AXError::NotEnoughPrecision => "kAXErrorNotEnoughPrecision",
        _ => "kAXErrorSuccess",
    }
}

pub fn query_accessibility_trust_prompting_the_user_if_untrusted() -> bool {
    let options = CFDictionary::from_slices(
        &[unsafe { kAXTrustedCheckOptionPrompt }],
        &[CFBoolean::new(true)],
    );
    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) }
}

pub fn query_accessibility_trust_without_prompting_the_user() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub fn read_window_id_of_accessibility_element(reference: &AXUIElement) -> u32 {
    let mut window_id: u32 = 0;
    unsafe { _AXUIElementGetWindow(reference, &mut window_id) };
    window_id
}

pub unsafe fn read_process_id_from_accessibility_element_memory(
    reference: AXUIElementRef,
) -> libc::pid_t {
    unsafe { *(reference.cast::<u8>().add(0x10).cast::<libc::pid_t>()) }
}

pub fn read_whether_enhanced_user_interface_is_enabled(reference: &AXUIElement) -> bool {
    let mut result = false;
    let mut value: *const CFType = core::ptr::null();

    if unsafe {
        AXUIElementCopyAttributeValue(
            reference,
            kAXEnhancedUserInterface(),
            NonNull::from(&mut value),
        )
    } == kAXErrorSuccess
    {
        if let Some(owned_value) = unsafe { take_create_rule_result(value) } {
            result = owned_value
                .downcast_ref::<CFBoolean>()
                .is_some_and(CFBoolean::as_bool);
        }
    }

    result
}

struct RestoreEnhancedUserInterfaceOnDrop<'a> {
    application_reference: &'a AXUIElement,
    was_enabled: bool,
}

impl Drop for RestoreEnhancedUserInterfaceOnDrop<'_> {
    fn drop(&mut self) {
        if self.was_enabled {
            unsafe {
                AXUIElementSetAttributeValue(
                    self.application_reference,
                    kAXEnhancedUserInterface(),
                    CFBoolean::new(true),
                )
            };
        }
    }
}

pub fn with_enhanced_user_interface_disabled<Result>(
    application_reference: &AXUIElement,
    body: impl FnOnce() -> Result,
) -> Result {
    let was_enabled = read_whether_enhanced_user_interface_is_enabled(application_reference);

    if was_enabled {
        unsafe {
            AXUIElementSetAttributeValue(
                application_reference,
                kAXEnhancedUserInterface(),
                CFBoolean::new(false),
            )
        };
    }

    let _restore_enhanced_user_interface_on_drop = RestoreEnhancedUserInterfaceOnDrop {
        application_reference,
        was_enabled,
    };

    body()
}
