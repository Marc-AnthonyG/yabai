use std::process::Command;

use crate::signal::definition::{
    SIGNAL_TYPE_COUNT, SIGNAL_TYPE_NAMES, Signal, SignalPropertyRequirement, SignalType,
};
use crate::signal::queue::PendingSignal;
use crate::support::regex::is_subject_rejected_by_optional_pattern;

pub(crate) fn is_signal_filtered_out_for_pending_signal(
    event_signal: &PendingSignal,
    signal: &Signal,
) -> bool {
    match event_signal.signal_type {
        SignalType::ApplicationLaunched
        | SignalType::ApplicationActivated
        | SignalType::ApplicationDeactivated
        | SignalType::ApplicationVisible => is_subject_rejected_by_optional_pattern(
            signal.app_regex.as_ref(),
            signal.app_regex_exclude,
            event_signal.app.as_deref().unwrap_or(""),
        ),
        SignalType::ApplicationTerminated
        | SignalType::ApplicationHidden
        | SignalType::WindowDestroyed => {
            let app_no_match = is_subject_rejected_by_optional_pattern(
                signal.app_regex.as_ref(),
                signal.app_regex_exclude,
                event_signal.app.as_deref().unwrap_or(""),
            );

            let mut active = signal.active == SignalPropertyRequirement::Undefined;
            if !active {
                active = event_signal.active
                    == i32::from(signal.active == SignalPropertyRequirement::Yes);
            }

            app_no_match || !active
        }
        SignalType::WindowCreated | SignalType::WindowFocused | SignalType::WindowDeminimized => {
            let app_no_match = is_subject_rejected_by_optional_pattern(
                signal.app_regex.as_ref(),
                signal.app_regex_exclude,
                event_signal.app.as_deref().unwrap_or(""),
            );

            let title_no_match = is_subject_rejected_by_optional_pattern(
                signal.title_regex.as_ref(),
                signal.title_regex_exclude,
                event_signal.title.as_deref().unwrap_or(""),
            );

            app_no_match || title_no_match
        }
        SignalType::WindowMoved
        | SignalType::WindowResized
        | SignalType::WindowMinimized
        | SignalType::WindowTitleChanged => {
            let app_no_match = is_subject_rejected_by_optional_pattern(
                signal.app_regex.as_ref(),
                signal.app_regex_exclude,
                event_signal.app.as_deref().unwrap_or(""),
            );

            let title_no_match = is_subject_rejected_by_optional_pattern(
                signal.title_regex.as_ref(),
                signal.title_regex_exclude,
                event_signal.title.as_deref().unwrap_or(""),
            );

            let mut active = signal.active == SignalPropertyRequirement::Undefined;
            if !active {
                active = event_signal.active
                    == i32::from(signal.active == SignalPropertyRequirement::Yes);
            }

            app_no_match || title_no_match || !active
        }
        _ => false,
    }
}

fn shell_running_the_command_of_signal_with_the_variables_of_pending_signal(
    signal: &Signal,
    event_signal: &PendingSignal,
) -> Command {
    let mut shell = Command::new("/usr/bin/env");
    shell.args(["sh", "-c"]);
    if let Some(command) = signal.command.as_deref() {
        shell.arg(command);
    }
    shell.envs(
        event_signal
            .arguments
            .iter()
            .flatten()
            .map(|(name, value)| (name, value)),
    );
    shell
}

pub(crate) fn run_subscriber_commands_of_pending_signals_without_waiting_for_them(
    signal_event: &[Vec<Signal>; SIGNAL_TYPE_COUNT],
    signal_storage: &mut Vec<PendingSignal>,
) {
    for event_signal in signal_storage.drain(..) {
        let subscribers = &signal_event[event_signal.signal_type as usize];
        crate::debug!(
            "{}: transmitting {} to {} subscriber(s)\n",
            "run_subscriber_commands_of_pending_signals_without_waiting_for_them",
            SIGNAL_TYPE_NAMES[event_signal.signal_type as usize],
            subscribers.len()
        );

        for signal in subscribers {
            if is_signal_filtered_out_for_pending_signal(&event_signal, signal) {
                continue;
            }
            let shell = shell_running_the_command_of_signal_with_the_variables_of_pending_signal(
                signal,
                &event_signal,
            )
            .spawn();
            if let Err(error) = shell {
                crate::debug!(
                    "{}: could not run the command of a {} subscriber: {}\n",
                    "run_subscriber_commands_of_pending_signals_without_waiting_for_them",
                    SIGNAL_TYPE_NAMES[event_signal.signal_type as usize],
                    error
                );
            }
        }
    }
}
