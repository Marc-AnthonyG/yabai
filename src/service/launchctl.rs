use std::path::Path;
use std::process::{Command, Stdio};

use crate::service::plist::{build_launchd_service_plist_path, write_launchd_service_plist};
use crate::support::filesystem::is_existing_file_that_is_not_a_directory;
use crate::support::spawned_program_exit_status::{
    SpawnedProgramOutput, run_program_and_read_its_exit_status_even_while_child_exits_are_ignored,
};
use crate::{error, warn};

const LAUNCHCTL_EXECUTABLE_PATH: &str = "/bin/launchctl";
pub(crate) const LAUNCHD_SERVICE_LABEL: &str = "com.asmvik.yabai";

//
// NOTE(asmvik): A launchd service has the following states:
//
//          1. Installed / Uninstalled
//          2. Active (Enable / Disable)
//          3. Bootstrapped (Load / Unload)
//          4. Running (Start / Stop)
//

fn run_launchctl_and_wait_for_its_exit_status(arguments: &[&str], suppress_output: bool) -> i32 {
    let mut launchctl = Command::new(LAUNCHCTL_EXECUTABLE_PATH);
    launchctl.args(arguments);
    if suppress_output {
        launchctl.stdout(Stdio::null()).stderr(Stdio::null());
    }
    launchctl
        .status()
        .ok()
        .and_then(|exit_status| exit_status.code())
        .unwrap_or(1)
}

fn gui_domain_target_of_the_current_user() -> String {
    format!("gui/{}", unsafe { libc::getuid() })
}

fn yabai_service_target_of_the_current_user() -> String {
    format!(
        "{}/{}",
        gui_domain_target_of_the_current_user(),
        LAUNCHD_SERVICE_LABEL
    )
}

pub fn start_launchd_service_installing_it_if_missing() -> i32 {
    let yabai_plist_path = build_launchd_service_plist_path();
    if !is_existing_file_that_is_not_a_directory(&yabai_plist_path) {
        warn!(
            "yabai: service file '{}' is not installed! attempting installation..\n",
            yabai_plist_path
        );

        let result = write_launchd_service_plist(&yabai_plist_path);
        if result != 0 {
            error!(
                "yabai: service file '{}' could not be installed! abort..\n",
                yabai_plist_path
            );
        }
    }

    let service_target = yabai_service_target_of_the_current_user();

    let domain_target = gui_domain_target_of_the_current_user();

    //
    // NOTE(asmvik): Check if service is bootstrapped
    //

    let print_arguments = ["print", &service_target];
    let is_bootstrapped = run_launchctl_and_wait_for_its_exit_status(&print_arguments, true);

    if is_bootstrapped != 0 {
        //
        // NOTE(asmvik): Service is not bootstrapped and could be disabled.
        // There is no way to query if the service is disabled, and we cannot
        // bootstrap a disabled service. Try to enable the service. This will be
        // a no-op if the service is already enabled.
        //

        let enable_arguments = ["enable", &service_target];
        run_launchctl_and_wait_for_its_exit_status(&enable_arguments, false);

        //
        // NOTE(asmvik): Bootstrap service into the target domain.
        // This will also start the program **iff* RunAtLoad is set to true.
        //

        let bootstrap_arguments = ["bootstrap", &domain_target, &yabai_plist_path];
        run_launchctl_and_wait_for_its_exit_status(&bootstrap_arguments, false)
    } else {
        //
        // NOTE(asmvik): The service has already been bootstrapped.
        // Tell the bootstrapped service to launch immediately; it is an
        // error to bootstrap a service that has already been bootstrapped.
        //

        let kickstart_arguments = ["kickstart", &service_target];
        run_launchctl_and_wait_for_its_exit_status(&kickstart_arguments, false)
    }
}

pub fn restart_launchd_service() -> i32 {
    let yabai_plist_path = build_launchd_service_plist_path();
    if !is_existing_file_that_is_not_a_directory(&yabai_plist_path) {
        error!(
            "yabai: service file '{}' is not installed! abort..\n",
            yabai_plist_path
        );
    }

    let service_target = yabai_service_target_of_the_current_user();

    let kickstart_arguments = ["kickstart", "-k", &service_target];
    run_launchctl_and_wait_for_its_exit_status(&kickstart_arguments, false)
}

pub fn stop_launchd_service() -> i32 {
    let yabai_plist_path = build_launchd_service_plist_path();
    if !is_existing_file_that_is_not_a_directory(&yabai_plist_path) {
        error!(
            "yabai: service file '{}' is not installed! abort..\n",
            yabai_plist_path
        );
    }

    let service_target = yabai_service_target_of_the_current_user();

    let domain_target = gui_domain_target_of_the_current_user();

    //
    // NOTE(asmvik): Check if service is bootstrapped
    //

    let print_arguments = ["print", &service_target];
    let is_bootstrapped = run_launchctl_and_wait_for_its_exit_status(&print_arguments, true);

    if is_bootstrapped != 0 {
        //
        // NOTE(asmvik): Service is not bootstrapped, but the program
        // could still be running an instance that was started **while the service
        // was bootstrapped**, so we tell it to stop said service.
        //

        let kill_arguments = ["kill", "SIGTERM", &service_target];
        run_launchctl_and_wait_for_its_exit_status(&kill_arguments, false)
    } else {
        //
        // NOTE(asmvik): Service is bootstrapped; we stop a potentially
        // running instance of the program and unload the service, making it
        // not trigger automatically in the future.
        //
        // This is NOT the same as disabling the service, which will prevent
        // it from being boostrapped in the future (without explicitly re-enabling
        // it first).
        //

        let bootout_arguments = ["bootout", &domain_target, &yabai_plist_path];
        run_launchctl_and_wait_for_its_exit_status(&bootout_arguments, false);

        let disable_arguments = ["disable", &service_target];
        run_launchctl_and_wait_for_its_exit_status(&disable_arguments, false)
    }
}

pub fn kickstart_the_launchd_service_killing_its_running_instance() -> bool {
    let service_target = format!(
        "gui/{}/{}",
        unsafe { libc::getuid() } as i32,
        LAUNCHD_SERVICE_LABEL
    );

    run_program_and_read_its_exit_status_even_while_child_exits_are_ignored(
        Path::new(LAUNCHCTL_EXECUTABLE_PATH),
        &["kickstart", "-k", &service_target],
        SpawnedProgramOutput::SharedWithThisProcess,
    ) == Some(libc::EXIT_SUCCESS)
}
