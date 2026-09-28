use crate::ffi::accessibility::query_accessibility_trust_prompting_the_user_if_untrusted;
use crate::ffi::skylight::{SLSGetSpaceManagementMode, SLSMainConnectionID};
use crate::require;
use crate::support::privilege::is_running_as_root;

pub(crate) fn exit_unless_the_system_meets_the_daemon_requirements() {
    if is_running_as_root() {
        require!("yabai: running as root is not allowed! abort..\n");
    }

    if !query_accessibility_trust_prompting_the_user_if_untrusted() {
        require!("yabai: could not access accessibility features! abort..\n");
    }

    if !(unsafe { SLSGetSpaceManagementMode(SLSMainConnectionID()) } == 1) {
        require!("yabai: 'display has separate spaces' is disabled! abort..\n");
    }
}
