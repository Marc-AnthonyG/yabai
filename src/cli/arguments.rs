use crate::cli::client::client_send_message;
use crate::error;
use crate::scripting_addition::installer::{scripting_addition_load, scripting_addition_uninstall};
use crate::service::launchctl::{service_restart, service_start, service_stop};
use crate::service::plist::{service_install, service_uninstall};
use crate::state::process_wide::CONFIG_FILE;
use crate::support::log::set_g_verbose;
use crate::support::strings::string_equals;

pub(crate) const SCRPT_ADD_LOAD_OPT: &str = "--load-sa";
pub(crate) const SCRPT_ADD_UNINSTALL_OPT: &str = "--uninstall-sa";
pub(crate) const SERVICE_INSTALL_OPT: &str = "--install-service";
pub(crate) const SERVICE_UNINSTALL_OPT: &str = "--uninstall-service";
pub(crate) const SERVICE_START_OPT: &str = "--start-service";
pub(crate) const SERVICE_RESTART_OPT: &str = "--restart-service";
pub(crate) const SERVICE_STOP_OPT: &str = "--stop-service";
pub(crate) const CLIENT_OPT_LONG: &str = "--message";
pub(crate) const CLIENT_OPT_SHRT: &str = "-m";
pub(crate) const CONFIG_OPT_LONG: &str = "--config";
pub(crate) const CONFIG_OPT_SHRT: &str = "-c";
pub(crate) const DEBUG_VERBOSE_OPT_LONG: &str = "--verbose";
pub(crate) const DEBUG_VERBOSE_OPT_SHRT: &str = "-V";
pub(crate) const VERSION_OPT_LONG: &str = "--version";
pub(crate) const VERSION_OPT_SHRT: &str = "-v";
pub(crate) const HELP_OPT_LONG: &str = "--help";
pub(crate) const HELP_OPT_SHRT: &str = "-h";

pub(crate) const MAJOR: i32 = 7;
pub(crate) const MINOR: i32 = 1;
pub(crate) const PATCH: i32 = 25;

pub(crate) fn parse_arguments(
    arguments: &[String],
    arguments_as_given: &[std::ffi::OsString],
) -> Option<String> {
    let argument_count = arguments.len();
    let mut config_file: Option<String> = None;

    if (string_equals(Some(&arguments[1]), Some(HELP_OPT_LONG)))
        || (string_equals(Some(&arguments[1]), Some(HELP_OPT_SHRT)))
    {
        print!(
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
            MAJOR, MINOR, PATCH
        );
        std::process::exit(libc::EXIT_SUCCESS);
    }

    if (string_equals(Some(&arguments[1]), Some(VERSION_OPT_LONG)))
        || (string_equals(Some(&arguments[1]), Some(VERSION_OPT_SHRT)))
    {
        print!("yabai-v{}.{}.{}\n", MAJOR, MINOR, PATCH);
        std::process::exit(libc::EXIT_SUCCESS);
    }

    if (string_equals(Some(&arguments[1]), Some(CLIENT_OPT_LONG)))
        || (string_equals(Some(&arguments[1]), Some(CLIENT_OPT_SHRT)))
    {
        std::process::exit(client_send_message(&arguments_as_given[1..]));
    }

    if string_equals(Some(&arguments[1]), Some(SCRPT_ADD_UNINSTALL_OPT)) {
        std::process::exit(scripting_addition_uninstall());
    }

    if string_equals(Some(&arguments[1]), Some(SCRPT_ADD_LOAD_OPT)) {
        std::process::exit(scripting_addition_load());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_INSTALL_OPT)) {
        std::process::exit(service_install());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_UNINSTALL_OPT)) {
        std::process::exit(service_uninstall());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_START_OPT)) {
        std::process::exit(service_start());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_RESTART_OPT)) {
        std::process::exit(service_restart());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_STOP_OPT)) {
        std::process::exit(service_stop());
    }

    let mut index = 1;
    while index < argument_count {
        let option = &arguments[index];

        if (string_equals(Some(option), Some(DEBUG_VERBOSE_OPT_LONG)))
            || (string_equals(Some(option), Some(DEBUG_VERBOSE_OPT_SHRT)))
        {
            set_g_verbose(true);
        } else if (string_equals(Some(option), Some(CONFIG_OPT_LONG)))
            || (string_equals(Some(option), Some(CONFIG_OPT_SHRT)))
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
                    CONFIG_OPT_LONG, CONFIG_OPT_SHRT
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
        config_file = parse_arguments(&arguments, &arguments_as_given);
    }
    let _ = CONFIG_FILE.set(config_file.unwrap_or_default());
}
