use serde::Serialize;

use crate::command::values::OnOrOff;
use crate::signal::definition::{
    SIGNAL_TYPE_BY_DISCRIMINANT, SIGNAL_TYPE_COUNT, Signal, SignalPropertyRequirement, SignalType,
};

#[derive(Serialize, Debug)]
pub(crate) struct SignalSnapshot {
    pub(crate) index: usize,
    pub(crate) event: SignalType,
    pub(crate) action: Option<String>,
    pub(crate) label: Option<String>,
    pub(crate) app: Option<String>,
    pub(crate) app_not: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) title_not: Option<String>,
    pub(crate) active: Option<OnOrOff>,
}

pub(crate) fn every_signal_as_pretty_json(
    signal_event: &[Vec<Signal>; SIGNAL_TYPE_COUNT],
) -> String {
    let snapshots: Vec<SignalSnapshot> = SIGNAL_TYPE_BY_DISCRIMINANT
        .iter()
        .zip(signal_event.iter())
        .skip(SignalType::ApplicationLaunched as usize)
        .flat_map(|(signal_type, signals)| signals.iter().map(move |signal| (*signal_type, signal)))
        .enumerate()
        .map(|(index, (signal_type, signal))| snapshot_of_signal(index, signal_type, signal))
        .collect();
    serde_json::to_string_pretty(&snapshots).unwrap_or_default()
}

fn snapshot_of_signal(index: usize, signal_type: SignalType, signal: &Signal) -> SignalSnapshot {
    let (app, app_not) = pattern_or_its_negation(&signal.app, signal.app_regex_exclude);
    let (title, title_not) = pattern_or_its_negation(&signal.title, signal.title_regex_exclude);
    SignalSnapshot {
        index,
        event: signal_type,
        action: signal.command.clone(),
        label: signal.label.clone(),
        app,
        app_not,
        title,
        title_not,
        active: match signal.active {
            SignalPropertyRequirement::Undefined => None,
            SignalPropertyRequirement::Yes => Some(OnOrOff::On),
            SignalPropertyRequirement::No => Some(OnOrOff::Off),
        },
    }
}

fn pattern_or_its_negation(
    pattern: &Option<String>,
    is_negated: bool,
) -> (Option<String>, Option<String>) {
    if is_negated {
        (None, pattern.clone())
    } else {
        (pattern.clone(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::every_signal_as_pretty_json;
    use crate::signal::definition::{
        SIGNAL_TYPE_COUNT, Signal, SignalPropertyRequirement, SignalType,
    };

    #[test]
    fn signals_list_in_event_order_with_one_index_across_every_event() {
        let mut signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT] = Default::default();
        signal_event[SignalType::SpaceChanged as usize].push(Signal {
            command: Some(String::from("echo space")),
            label: Some(String::from("screen_padding_display_changed")),
            ..Signal::default()
        });
        signal_event[SignalType::WindowFocused as usize].push(Signal {
            command: Some(String::from("echo window")),
            app: Some(String::from("^Finder$")),
            app_regex_exclude: true,
            active: SignalPropertyRequirement::Yes,
            ..Signal::default()
        });

        let listing: serde_json::Value =
            serde_json::from_str(&every_signal_as_pretty_json(&signal_event)).unwrap();

        assert_eq!(listing[0]["index"], 0);
        assert_eq!(listing[0]["event"], "window-focused");
        assert_eq!(listing[0]["app"], serde_json::Value::Null);
        assert_eq!(listing[0]["app_not"], "^Finder$");
        assert_eq!(listing[0]["active"], "on");
        assert_eq!(listing[1]["index"], 1);
        assert_eq!(listing[1]["event"], "space-changed");
        assert_eq!(listing[1]["label"], "screen_padding_display_changed");
        assert_eq!(listing[1]["active"], serde_json::Value::Null);
    }

    #[test]
    fn no_signal_lists_as_an_empty_array() {
        let signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT] = Default::default();

        assert_eq!(every_signal_as_pretty_json(&signal_event), "[]");
    }
}
