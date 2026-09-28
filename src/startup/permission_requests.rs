use crate::ffi::accessibility::query_accessibility_trust_prompting_the_user_if_untrusted;
use crate::ffi::core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use crate::scripting_addition::installer::is_system_integrity_protection_relaxed_enough_for_scripting_addition;

pub(crate) enum ScreenRecordingPermissionAtStartUp {
    AlreadyGrantedOrUselessWithoutTheScriptingAddition,
    RequestedFromTheUser,
}

pub(crate) struct PermissionsAtStartUp {
    pub(crate) accessibility_was_already_trusted: bool,
    pub(crate) screen_recording: ScreenRecordingPermissionAtStartUp,
}

pub(crate) fn ask_for_every_missing_permission_at_once() -> PermissionsAtStartUp {
    let accessibility_was_already_trusted =
        query_accessibility_trust_prompting_the_user_if_untrusted();

    let screen_recording = if is_screen_recording_missing_while_window_animations_could_use_it() {
        CGRequestScreenCaptureAccess();
        ScreenRecordingPermissionAtStartUp::RequestedFromTheUser
    } else {
        ScreenRecordingPermissionAtStartUp::AlreadyGrantedOrUselessWithoutTheScriptingAddition
    };

    PermissionsAtStartUp {
        accessibility_was_already_trusted,
        screen_recording,
    }
}

fn is_screen_recording_missing_while_window_animations_could_use_it() -> bool {
    !CGPreflightScreenCaptureAccess()
        && is_system_integrity_protection_relaxed_enough_for_scripting_addition()
}
