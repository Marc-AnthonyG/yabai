use crate::command::{LaunchdServiceAction, ScriptingAdditionAction};
use crate::scripting_addition::installer::{
    install_and_load_scripting_addition, uninstall_scripting_addition,
};
use crate::service::launchctl::{
    restart_launchd_service, start_launchd_service_installing_it_if_missing, stop_launchd_service,
};
use crate::service::plist::{install_launchd_service, uninstall_launchd_service};

pub(crate) fn run_launchd_service_action(action: LaunchdServiceAction) -> i32 {
    match action {
        LaunchdServiceAction::Install => install_launchd_service(),
        LaunchdServiceAction::Uninstall => uninstall_launchd_service(),
        LaunchdServiceAction::Start => start_launchd_service_installing_it_if_missing(),
        LaunchdServiceAction::Restart => restart_launchd_service(),
        LaunchdServiceAction::Stop => stop_launchd_service(),
    }
}

pub(crate) fn run_scripting_addition_action(action: ScriptingAdditionAction) -> i32 {
    match action {
        ScriptingAdditionAction::Load => install_and_load_scripting_addition(),
        ScriptingAdditionAction::Uninstall => uninstall_scripting_addition(),
    }
}
