use std::time::Instant;

use crate::startup::daemon_relaunch_after_screen_recording_grant::relaunch_the_daemon_so_the_screen_recording_grant_takes_effect;
use crate::startup::permission_requests::ScreenRecordingPermissionAtStartUp;
use crate::startup::screen_recording_grant_check_schedule::times_since_watching_began_to_check_for_a_screen_recording_grant;
use crate::startup::screen_recording_grant_in_a_fresh_process::has_screen_recording_been_granted_according_to_a_fresh_yabai_process;

pub(crate) fn start_watching_for_a_requested_screen_recording_grant_to_relaunch_the_daemon(
    screen_recording_permission_at_start_up: ScreenRecordingPermissionAtStartUp,
) {
    if let ScreenRecordingPermissionAtStartUp::RequestedFromTheUser =
        screen_recording_permission_at_start_up
    {
        let _ = std::thread::Builder::new()
            .name(String::from("yabai-screen-recording-grant-watcher"))
            .spawn(relaunch_the_daemon_if_screen_recording_is_granted_before_giving_up);
    }
}

fn relaunch_the_daemon_if_screen_recording_is_granted_before_giving_up() {
    let watching_began = Instant::now();
    for time_since_watching_began in
        times_since_watching_began_to_check_for_a_screen_recording_grant()
    {
        sleep_until(watching_began + time_since_watching_began);
        if has_screen_recording_been_granted_according_to_a_fresh_yabai_process() {
            relaunch_the_daemon_so_the_screen_recording_grant_takes_effect();
            return;
        }
    }
}

fn sleep_until(moment: Instant) {
    std::thread::sleep(moment.saturating_duration_since(Instant::now()));
}
