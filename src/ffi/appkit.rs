#![allow(deprecated)]
#![allow(non_upper_case_globals)]

pub use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationLoad, NSImage,
    NSRunningApplication, NSScreen, NSWorkspace, NSWorkspaceActiveSpaceDidChangeNotification,
    NSWorkspaceApplicationKey, NSWorkspaceDidHideApplicationNotification,
    NSWorkspaceDidUnhideApplicationNotification, NSWorkspaceDidWakeNotification,
};

pub const NS_WORKSPACE_ACTIVE_DISPLAY_DID_CHANGE_NOTIFICATION: &str =
    "NSWorkspaceActiveDisplayDidChangeNotification";
pub const APPLE_INTERFACE_MENU_BAR_HIDING_CHANGED_NOTIFICATION: &str =
    "AppleInterfaceMenuBarHidingChangedNotification";
pub const NS_APPLICATION_DOCK_DID_RESTART_NOTIFICATION: &str =
    "NSApplicationDockDidRestartNotification";
pub const COM_APPLE_DOCK_PREFCHANGED: &str = "com.apple.dock.prefchanged";
