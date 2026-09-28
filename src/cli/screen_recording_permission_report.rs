use crate::ffi::core_graphics::CGPreflightScreenCaptureAccess;

pub(crate) const REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION: &str =
    "--report-screen-recording-permission";

pub(crate) fn exit_status_reporting_whether_screen_recording_is_granted() -> i32 {
    if CGPreflightScreenCaptureAccess() {
        libc::EXIT_SUCCESS
    } else {
        libc::EXIT_FAILURE
    }
}
