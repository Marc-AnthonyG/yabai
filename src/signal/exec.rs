use std::process::Command;

use crate::signal::definition::{
    SIGNAL_TYPE_COUNT, SIGNAL_TYPE_NAMES, Signal, SignalPropertyRequirement, SignalType,
};
use crate::signal::queue::PendingSignal;
use crate::support::regex::is_subject_rejected_by_optional_pattern;

fn is_application_of_pending_signal_rejected_by_signal(
    event_signal: &PendingSignal,
    signal: &Signal,
) -> bool {
    is_subject_rejected_by_optional_pattern(
        signal.app_regex.as_ref(),
        signal.app_regex_exclude,
        event_signal.app.as_deref().unwrap_or(""),
    )
}

fn is_title_of_pending_signal_rejected_by_signal(
    event_signal: &PendingSignal,
    signal: &Signal,
) -> bool {
    is_subject_rejected_by_optional_pattern(
        signal.title_regex.as_ref(),
        signal.title_regex_exclude,
        event_signal.title.as_deref().unwrap_or(""),
    )
}

fn is_activity_of_pending_signal_rejected_by_signal(
    event_signal: &PendingSignal,
    signal: &Signal,
) -> bool {
    match signal.active {
        SignalPropertyRequirement::Undefined => false,
        required => event_signal.active != i32::from(required == SignalPropertyRequirement::Yes),
    }
}

pub(crate) fn is_signal_filtered_out_for_pending_signal(
    event_signal: &PendingSignal,
    signal: &Signal,
) -> bool {
    let application_is_rejected =
        || is_application_of_pending_signal_rejected_by_signal(event_signal, signal);
    let title_is_rejected = || is_title_of_pending_signal_rejected_by_signal(event_signal, signal);
    let activity_is_rejected =
        || is_activity_of_pending_signal_rejected_by_signal(event_signal, signal);

    match event_signal.signal_type {
        SignalType::ApplicationLaunched
        | SignalType::ApplicationActivated
        | SignalType::ApplicationDeactivated
        | SignalType::ApplicationVisible => application_is_rejected(),
        SignalType::ApplicationTerminated
        | SignalType::ApplicationHidden
        | SignalType::WindowDestroyed => application_is_rejected() || activity_is_rejected(),
        SignalType::WindowCreated | SignalType::WindowFocused | SignalType::WindowDeminimized => {
            application_is_rejected() || title_is_rejected()
        }
        SignalType::WindowMoved
        | SignalType::WindowResized
        | SignalType::WindowMinimized
        | SignalType::WindowTitleChanged => {
            application_is_rejected() || title_is_rejected() || activity_is_rejected()
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

#[cfg(test)]
mod tests {
    use regex::Regex;

    use super::is_signal_filtered_out_for_pending_signal;
    use crate::signal::definition::{Signal, SignalPropertyRequirement, SignalType};
    use crate::signal::queue::PendingSignal;

    fn pending_signal(
        signal_type: SignalType,
        app: &str,
        title: &str,
        active: bool,
    ) -> PendingSignal {
        PendingSignal {
            signal_type,
            arguments: Default::default(),
            app: Some(app.to_string()),
            title: Some(title.to_string()),
            active: i32::from(active),
        }
    }

    fn signal_for_app(pattern: &str, exclude: bool) -> Signal {
        Signal {
            app_regex: Some(Regex::new(pattern).unwrap()),
            app_regex_exclude: exclude,
            ..Signal::default()
        }
    }

    #[test]
    fn a_signal_without_filters_lets_every_event_through() {
        let event = pending_signal(SignalType::WindowMoved, "Safari", "Start Page", false);

        assert!(!is_signal_filtered_out_for_pending_signal(
            &event,
            &Signal::default()
        ));
    }

    #[test]
    fn an_application_event_is_filtered_by_the_app_pattern_and_its_exclusion() {
        let event = pending_signal(SignalType::ApplicationLaunched, "Safari", "", false);

        assert!(!is_signal_filtered_out_for_pending_signal(
            &event,
            &signal_for_app("^Safari$", false)
        ));
        assert!(is_signal_filtered_out_for_pending_signal(
            &event,
            &signal_for_app("^Finder$", false)
        ));
        assert!(is_signal_filtered_out_for_pending_signal(
            &event,
            &signal_for_app("^Safari$", true)
        ));
    }

    #[test]
    fn a_window_move_is_filtered_by_its_title() {
        let event = pending_signal(SignalType::WindowMoved, "Safari", "Downloads", false);
        let signal = Signal {
            title_regex: Some(Regex::new("^Start Page$").unwrap()),
            ..Signal::default()
        };

        assert!(is_signal_filtered_out_for_pending_signal(&event, &signal));
    }

    #[test]
    fn the_active_requirement_filters_the_events_that_carry_it_and_no_other() {
        let signal_requiring_an_active_window = Signal {
            active: SignalPropertyRequirement::Yes,
            ..Signal::default()
        };
        let inactive_window_move = pending_signal(SignalType::WindowMoved, "Safari", "", false);
        let active_window_move = pending_signal(SignalType::WindowMoved, "Safari", "", true);
        let inactive_window_creation =
            pending_signal(SignalType::WindowCreated, "Safari", "", false);

        assert!(is_signal_filtered_out_for_pending_signal(
            &inactive_window_move,
            &signal_requiring_an_active_window
        ));
        assert!(!is_signal_filtered_out_for_pending_signal(
            &active_window_move,
            &signal_requiring_an_active_window
        ));
        assert!(!is_signal_filtered_out_for_pending_signal(
            &inactive_window_creation,
            &signal_requiring_an_active_window
        ));
    }
}
