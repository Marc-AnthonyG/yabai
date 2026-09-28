use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::config_file::location::locate_the_config_file_warning_when_there_is_none;
use crate::config_file::shell_run::run_config_file_in_a_shell_without_waiting_for_it;
use crate::state::process_wide::RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED;
use crate::support::file_change_watch::FileChangeWatch;

const QUIET_PERIOD_THAT_ENDS_ONE_SAVE: Duration = Duration::from_millis(150);
const DELAY_BEFORE_WATCHING_AGAIN_AFTER_A_FAILED_WATCH: Duration = Duration::from_secs(1);

static CONFIG_FILE_WATCHER_HAS_STARTED: AtomicBool = AtomicBool::new(false);

pub(crate) fn start_watching_the_config_file_to_reload_it_on_change_unless_already_watching() {
    if CONFIG_FILE_WATCHER_HAS_STARTED.load(Ordering::Relaxed) {
        return;
    }
    let Some(config_file) = locate_the_config_file_warning_when_there_is_none() else {
        return;
    };

    let watcher_has_started = std::thread::Builder::new()
        .name(String::from("yabai-config-file-watcher"))
        .spawn(move || rerun_the_config_file_whenever_its_contents_change(config_file))
        .is_ok();
    CONFIG_FILE_WATCHER_HAS_STARTED.store(watcher_has_started, Ordering::Relaxed);
}

fn rerun_the_config_file_whenever_its_contents_change(config_file: String) {
    let mut contents_that_last_ran = std::fs::read(&config_file).ok();
    loop {
        let Ok(config_file_change_watch) =
            FileChangeWatch::start_watching_the_file(config_file.as_ref())
        else {
            std::thread::sleep(DELAY_BEFORE_WATCHING_AGAIN_AFTER_A_FAILED_WATCH);
            continue;
        };

        rerun_the_config_file_if_reloading_is_enabled_and_it_changed_since_it_last_ran(
            &config_file,
            &mut contents_that_last_ran,
        );

        if config_file_change_watch
            .wait_for_a_change_then_until_changes_stop_for(QUIET_PERIOD_THAT_ENDS_ONE_SAVE)
            .is_err()
        {
            std::thread::sleep(DELAY_BEFORE_WATCHING_AGAIN_AFTER_A_FAILED_WATCH);
        }
    }
}

fn rerun_the_config_file_if_reloading_is_enabled_and_it_changed_since_it_last_ran(
    config_file: &str,
    contents_that_last_ran: &mut Option<Vec<u8>>,
) {
    if !RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let Ok(current_contents) = std::fs::read(config_file) else {
        return;
    };
    if contents_that_last_ran.as_ref() == Some(&current_contents) {
        return;
    }

    *contents_that_last_ran = Some(current_contents);
    run_config_file_in_a_shell_without_waiting_for_it(config_file);
}
