use crate::ffi::skylight::{SLSGetSpaceManagementMode, SLSMainConnectionID};
use crate::require;
use crate::startup::accessibility_trust_wait::wait_until_accessibility_is_trusted_announcing_the_wait_once;
use crate::startup::permission_requests::{
    ScreenRecordingPermissionAtStartUp, ask_for_every_missing_permission_at_once,
};
use crate::support::privilege::is_running_as_root;

pub(crate) fn ask_for_missing_permissions_then_exit_or_wait_until_the_system_meets_the_daemon_requirements()
-> ScreenRecordingPermissionAtStartUp {
    if is_running_as_root() {
        require!("yabai: running as root is not allowed! abort..\n");
    }

    let permissions_at_start_up = ask_for_every_missing_permission_at_once();

    if !(unsafe { SLSGetSpaceManagementMode(SLSMainConnectionID()) } == 1) {
        require!("yabai: 'display has separate spaces' is disabled! abort..\n");
    }

    if !permissions_at_start_up.accessibility_was_already_trusted {
        wait_until_accessibility_is_trusted_announcing_the_wait_once();
    }

    permissions_at_start_up.screen_recording
}
