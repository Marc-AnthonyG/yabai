use crate::service::launchctl::kickstart_the_launchd_service_killing_its_running_instance;
use crate::service::running_as_the_service::is_this_process_running_as_the_launchd_service;
use crate::warn;

pub(crate) fn relaunch_the_daemon_so_the_screen_recording_grant_takes_effect() {
    if is_this_process_running_as_the_launchd_service()
        && kickstart_the_launchd_service_killing_its_running_instance()
    {
        return;
    }

    warn!(
        "yabai: screen recording permission was granted! restart yabai to enable window animations..\n"
    );
}
