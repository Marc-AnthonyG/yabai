use crate::cli::arguments::REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION;
use crate::support::spawned_program_exit_status::{
    SpawnedProgramOutput, run_program_and_read_its_exit_status_even_while_child_exits_are_ignored,
};

pub(crate) fn has_screen_recording_been_granted_according_to_a_fresh_yabai_process() -> bool {
    let Ok(yabai_executable_path) = std::env::current_exe() else {
        return false;
    };

    run_program_and_read_its_exit_status_even_while_child_exits_are_ignored(
        &yabai_executable_path,
        &[REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION],
        SpawnedProgramOutput::DiscardedIntoDevNull,
    ) == Some(libc::EXIT_SUCCESS)
}
