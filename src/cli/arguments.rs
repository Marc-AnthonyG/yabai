use crate::cli::client::send_message_to_daemon_and_print_its_response;
use crate::cli::screen_recording_permission_report::exit_status_reporting_whether_screen_recording_is_granted;
use crate::error;
use crate::scripting_addition::installer::{
    install_and_load_scripting_addition, uninstall_scripting_addition,
};
use crate::service::launchctl::{
    restart_launchd_service, start_launchd_service_installing_it_if_missing, stop_launchd_service,
};
use crate::service::plist::{install_launchd_service, uninstall_launchd_service};
use crate::state::process_wide::CONFIG_FILE_PATH;
use crate::support::log::set_verbose_debug_output_enabled;
use crate::support::strings::are_both_strings_present_and_equal;

pub(crate) const LOAD_SCRIPTING_ADDITION_OPTION: &str = "--load-sa";
pub(crate) const UNINSTALL_SCRIPTING_ADDITION_OPTION: &str = "--uninstall-sa";
pub(crate) const INSTALL_SERVICE_OPTION: &str = "--install-service";
pub(crate) const UNINSTALL_SERVICE_OPTION: &str = "--uninstall-service";
pub(crate) const START_SERVICE_OPTION: &str = "--start-service";
pub(crate) const RESTART_SERVICE_OPTION: &str = "--restart-service";
pub(crate) const STOP_SERVICE_OPTION: &str = "--stop-service";
pub(crate) const SEND_MESSAGE_LONG_OPTION: &str = "--message";
pub(crate) const SEND_MESSAGE_SHORT_OPTION: &str = "-m";
pub(crate) const CONFIG_FILE_LONG_OPTION: &str = "--config";
pub(crate) const CONFIG_FILE_SHORT_OPTION: &str = "-c";
pub(crate) const VERBOSE_DEBUG_OUTPUT_LONG_OPTION: &str = "--verbose";
pub(crate) const VERBOSE_DEBUG_OUTPUT_SHORT_OPTION: &str = "-V";
pub(crate) const PRINT_VERSION_LONG_OPTION: &str = "--version";
pub(crate) const PRINT_VERSION_SHORT_OPTION: &str = "-v";
pub(crate) const PRINT_HELP_LONG_OPTION: &str = "--help";
pub(crate) const PRINT_HELP_SHORT_OPTION: &str = "-h";
pub(crate) const REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION: &str =
    "--report-screen-recording-permission";

pub(crate) const MAJOR_VERSION: i32 = 7;
pub(crate) const MINOR_VERSION: i32 = 1;
pub(crate) const PATCH_VERSION: i32 = 25;

fn help_text_listing_every_public_option() -> String {
    format!(
        "Usage: yabai [option]\n\
         Options:\n    \
         --load-sa              Install and load the scripting-addition.\n    \
         --uninstall-sa         Uninstall the scripting-addition.\n    \
         --install-service      Write launchd service file to disk.\n    \
         --uninstall-service    Remove launchd service file from disk.\n    \
         --start-service        Enable, load, and start the launchd service.\n    \
         --restart-service      Attempts to restart the service instance.\n    \
         --stop-service         Stops a running instance of the service.\n    \
         --message, -m <msg>    Send message to a running instance of yabai.\n    \
         --config, -c <config>  Use the specified configuration file.\n    \
         --verbose, -V          Output debug information to stdout.\n    \
         --version, -v          Print version to stdout and exit.\n    \
         --help, -h             Print options to stdout and exit.\n\
         Type `man yabai` for more information, or visit: \
         https://github.com/asmvik/yabai/blob/v{}.{}.{}/doc/yabai.asciidoc\n",
        MAJOR_VERSION, MINOR_VERSION, PATCH_VERSION
    )
}

pub(crate) fn parse_command_line_exiting_after_a_one_shot_option(
    arguments: &[String],
    arguments_as_given: &[std::ffi::OsString],
) -> Option<String> {
    let argument_count = arguments.len();
    let mut config_file: Option<String> = None;

    if (are_both_strings_present_and_equal(Some(&arguments[1]), Some(PRINT_HELP_LONG_OPTION)))
        || (are_both_strings_present_and_equal(Some(&arguments[1]), Some(PRINT_HELP_SHORT_OPTION)))
    {
        print!("{}", help_text_listing_every_public_option());
        std::process::exit(libc::EXIT_SUCCESS);
    }

    if (are_both_strings_present_and_equal(Some(&arguments[1]), Some(PRINT_VERSION_LONG_OPTION)))
        || (are_both_strings_present_and_equal(
            Some(&arguments[1]),
            Some(PRINT_VERSION_SHORT_OPTION),
        ))
    {
        print!(
            "yabai-v{}.{}.{}\n",
            MAJOR_VERSION, MINOR_VERSION, PATCH_VERSION
        );
        std::process::exit(libc::EXIT_SUCCESS);
    }

    if (are_both_strings_present_and_equal(Some(&arguments[1]), Some(SEND_MESSAGE_LONG_OPTION)))
        || (are_both_strings_present_and_equal(
            Some(&arguments[1]),
            Some(SEND_MESSAGE_SHORT_OPTION),
        ))
    {
        std::process::exit(send_message_to_daemon_and_print_its_response(
            &arguments_as_given[1..],
        ));
    }

    if are_both_strings_present_and_equal(
        Some(&arguments[1]),
        Some(UNINSTALL_SCRIPTING_ADDITION_OPTION),
    ) {
        std::process::exit(uninstall_scripting_addition());
    }

    if are_both_strings_present_and_equal(Some(&arguments[1]), Some(LOAD_SCRIPTING_ADDITION_OPTION))
    {
        std::process::exit(install_and_load_scripting_addition());
    }

    if are_both_strings_present_and_equal(Some(&arguments[1]), Some(INSTALL_SERVICE_OPTION)) {
        std::process::exit(install_launchd_service());
    }

    if are_both_strings_present_and_equal(Some(&arguments[1]), Some(UNINSTALL_SERVICE_OPTION)) {
        std::process::exit(uninstall_launchd_service());
    }

    if are_both_strings_present_and_equal(Some(&arguments[1]), Some(START_SERVICE_OPTION)) {
        std::process::exit(start_launchd_service_installing_it_if_missing());
    }

    if are_both_strings_present_and_equal(Some(&arguments[1]), Some(RESTART_SERVICE_OPTION)) {
        std::process::exit(restart_launchd_service());
    }

    if are_both_strings_present_and_equal(Some(&arguments[1]), Some(STOP_SERVICE_OPTION)) {
        std::process::exit(stop_launchd_service());
    }

    if are_both_strings_present_and_equal(
        Some(&arguments[1]),
        Some(REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION),
    ) {
        std::process::exit(exit_status_reporting_whether_screen_recording_is_granted());
    }

    let mut index = 1;
    while index < argument_count {
        let option = &arguments[index];

        if (are_both_strings_present_and_equal(
            Some(option),
            Some(VERBOSE_DEBUG_OUTPUT_LONG_OPTION),
        )) || (are_both_strings_present_and_equal(
            Some(option),
            Some(VERBOSE_DEBUG_OUTPUT_SHORT_OPTION),
        )) {
            set_verbose_debug_output_enabled(true);
        } else if (are_both_strings_present_and_equal(Some(option), Some(CONFIG_FILE_LONG_OPTION)))
            || (are_both_strings_present_and_equal(Some(option), Some(CONFIG_FILE_SHORT_OPTION)))
        {
            let value = if index < argument_count - 1 {
                index += 1;
                Some(&arguments[index])
            } else {
                None
            };
            let Some(value) = value else {
                error!(
                    "yabai: option '{}|{}' requires an argument!\n",
                    CONFIG_FILE_LONG_OPTION, CONFIG_FILE_SHORT_OPTION
                );
            };
            config_file = Some(value.clone());
        } else {
            error!("yabai: '{}' is not a valid option!\n", option);
        }

        index += 1;
    }

    config_file
}

pub(crate) fn store_config_file_path_from_command_line() {
    let arguments_as_given: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let arguments: Vec<String> = arguments_as_given
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let argument_count = arguments.len();

    let mut config_file: Option<String> = None;
    if argument_count > 1 {
        config_file =
            parse_command_line_exiting_after_a_one_shot_option(&arguments, &arguments_as_given);
    }
    let _ = CONFIG_FILE_PATH.set(config_file.unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::{
        CONFIG_FILE_LONG_OPTION, CONFIG_FILE_SHORT_OPTION, INSTALL_SERVICE_OPTION,
        LOAD_SCRIPTING_ADDITION_OPTION, PRINT_HELP_LONG_OPTION, PRINT_HELP_SHORT_OPTION,
        PRINT_VERSION_LONG_OPTION, PRINT_VERSION_SHORT_OPTION,
        REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION, RESTART_SERVICE_OPTION,
        SEND_MESSAGE_LONG_OPTION, SEND_MESSAGE_SHORT_OPTION, START_SERVICE_OPTION,
        STOP_SERVICE_OPTION, UNINSTALL_SCRIPTING_ADDITION_OPTION, UNINSTALL_SERVICE_OPTION,
        VERBOSE_DEBUG_OUTPUT_LONG_OPTION, VERBOSE_DEBUG_OUTPUT_SHORT_OPTION,
        help_text_listing_every_public_option,
    };

    #[test]
    fn the_help_text_lists_every_public_option() {
        let help_text = help_text_listing_every_public_option();
        let public_options = [
            LOAD_SCRIPTING_ADDITION_OPTION,
            UNINSTALL_SCRIPTING_ADDITION_OPTION,
            INSTALL_SERVICE_OPTION,
            UNINSTALL_SERVICE_OPTION,
            START_SERVICE_OPTION,
            RESTART_SERVICE_OPTION,
            STOP_SERVICE_OPTION,
            SEND_MESSAGE_LONG_OPTION,
            SEND_MESSAGE_SHORT_OPTION,
            CONFIG_FILE_LONG_OPTION,
            CONFIG_FILE_SHORT_OPTION,
            VERBOSE_DEBUG_OUTPUT_LONG_OPTION,
            VERBOSE_DEBUG_OUTPUT_SHORT_OPTION,
            PRINT_VERSION_LONG_OPTION,
            PRINT_VERSION_SHORT_OPTION,
            PRINT_HELP_LONG_OPTION,
            PRINT_HELP_SHORT_OPTION,
        ];

        for public_option in public_options {
            assert!(help_text.contains(public_option), "{public_option}");
        }
    }

    #[test]
    fn the_help_text_never_mentions_the_screen_recording_permission_report() {
        let help_text = help_text_listing_every_public_option();

        assert!(
            !help_text.contains(REPORT_SCREEN_RECORDING_PERMISSION_THROUGH_THE_EXIT_STATUS_OPTION)
        );
    }
}
