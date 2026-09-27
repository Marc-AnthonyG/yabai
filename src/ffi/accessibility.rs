#![allow(deprecated)]
#![allow(unused_imports)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::OnceLock;

use objc2_core_foundation::{CFBoolean, CFData, CFDictionary, CFString, CFType};

pub use objc2_application_services::{
    AXError, AXIsProcessTrustedWithOptions, AXObserver, AXObserverAddNotification,
    AXObserverCallback, AXObserverCreate, AXObserverGetRunLoopSource, AXObserverRemoveNotification,
    AXUIElement, AXUIElementCopyAttributeValue, AXUIElementCopyElementAtPosition,
    AXUIElementCreateApplication, AXUIElementCreateSystemWide, AXUIElementIsAttributeSettable,
    AXUIElementPerformAction, AXUIElementSetAttributeValue, AXUIElementSetMessagingTimeout,
    AXValue, AXValueCreate, AXValueGetValue, AXValueType, kAXTrustedCheckOptionPrompt,
};

use crate::ffi::core_foundation::{
    CFDictionaryCreate, CFIndex, SendCFRetained, as_cftype, cfboolean_get_value, kCFBooleanFalse,
    kCFBooleanTrue, kCFCopyStringDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
    take_create_rule_result,
};

pub type AXUIElementRef = *const AXUIElement;
pub type AXObserverRef = *mut AXObserver;
pub type AXValueRef = *const AXValue;

pub type ObserverCallback = AXObserverCallback;

pub const kAXErrorSuccess: AXError = AXError::Success;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn _AXUIElementGetWindow(r#ref: *const AXUIElement, window_id: *mut u32) -> AXError;
    pub fn _AXUIElementCreateWithRemoteToken(data: *const CFData) -> *mut AXUIElement;
}

macro_rules! ax_string_constants {
    ($($constant_name:ident => $string_value:literal,)*) => {
        $(
            pub fn $constant_name() -> &'static CFString {
                static CACHED: OnceLock<SendCFRetained<CFString>> = OnceLock::new();
                CACHED
                    .get_or_init(|| SendCFRetained(CFString::from_static_str($string_value)))
                    .as_ref()
            }
        )*
    };
}

ax_string_constants! {
    kAXWindowsAttribute                 => "AXWindows",
    kAXFocusedWindowAttribute           => "AXFocusedWindow",
    kAXMainWindowAttribute              => "AXMainWindow",
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

pub fn ax_error_str(error: AXError) -> &'static str {
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

pub fn ax_privilege() -> bool {
    let mut keys: [*const c_void; 1] =
        [(unsafe { kAXTrustedCheckOptionPrompt } as *const CFString).cast::<c_void>()];
    let mut values: [*const c_void; 1] =
        [(kCFBooleanTrue() as *const CFBoolean).cast::<c_void>()];
    let options: objc2_core_foundation::CFRetained<CFDictionary> = unsafe {
        CFDictionaryCreate(
            None,
            keys.as_mut_ptr(),
            values.as_mut_ptr(),
            keys.len() as CFIndex,
            &raw const kCFCopyStringDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        )
    }
    .unwrap();
    unsafe { AXIsProcessTrustedWithOptions(Some(&options)) }
}

pub fn ax_window_id(reference: &AXUIElement) -> u32 {
    let mut window_id: u32 = 0;
    unsafe { _AXUIElementGetWindow(reference, &mut window_id) };
    window_id
}

pub unsafe fn ax_window_pid(reference: AXUIElementRef) -> libc::pid_t {
    unsafe { *(reference.cast::<u8>().add(0x10).cast::<libc::pid_t>()) }
}

pub fn ax_enhanced_userinterface(reference: &AXUIElement) -> bool {
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
            result = cfboolean_get_value(unsafe {
                &*((&*owned_value as *const CFType).cast::<CFBoolean>())
            });
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
                    as_cftype(kCFBooleanTrue()),
                )
            };
        }
    }
}

pub fn with_enhanced_user_interface_disabled<Result>(
    application_reference: &AXUIElement,
    body: impl FnOnce() -> Result,
) -> Result {
    let was_enabled = ax_enhanced_userinterface(application_reference);

    if was_enabled {
        unsafe {
            AXUIElementSetAttributeValue(
                application_reference,
                kAXEnhancedUserInterface(),
                as_cftype(kCFBooleanFalse()),
            )
        };
    }

    let _restore_enhanced_user_interface_on_drop = RestoreEnhancedUserInterfaceOnDrop {
        application_reference,
        was_enabled,
    };

    body()
}
