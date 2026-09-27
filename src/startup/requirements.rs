use crate::ffi::accessibility::ax_privilege;
use crate::ffi::skylight::{SLSGetSpaceManagementMode, SLSMainConnectionID};
use crate::require;
use crate::support::privilege::is_root;

pub(crate) fn exit_unless_the_system_meets_the_daemon_requirements() {
    if is_root() {
        require!("yabai: running as root is not allowed! abort..\n");
    }

    if !ax_privilege() {
        require!("yabai: could not access accessibility features! abort..\n");
    }

    if !(unsafe { SLSGetSpaceManagementMode(SLSMainConnectionID()) } == 1) {
        require!("yabai: 'display has separate spaces' is disabled! abort..\n");
    }
}
