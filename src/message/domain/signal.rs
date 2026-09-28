use regex::Regex;

use crate::command::selectors::IndexOrLabelSelector;
use crate::command::signal::{SignalCommand, SignalDefinition};
use crate::command::values::OnOrOff;
use crate::serialise::signal::every_signal_as_pretty_json;
use crate::signal::definition::{
    SIGNAL_TYPE_COUNT, Signal, SignalPropertyRequirement,
    add_signal_replacing_any_with_the_same_label, remove_signal_at_listing_index,
    remove_signal_with_label,
};

pub(crate) fn run_signal_command(
    command: SignalCommand,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) -> Result<String, String> {
    match command {
        SignalCommand::Add(definition) => {
            let signal = signal_of_definition(&definition)?;
            add_signal_replacing_any_with_the_same_label(definition.event, signal, signal_event);
            Ok(String::new())
        }
        SignalCommand::Remove { signal } => {
            let was_removed = match &signal {
                IndexOrLabelSelector::Index(index) => {
                    remove_signal_at_listing_index(*index as usize, signal_event)
                }
                IndexOrLabelSelector::Label(label) => remove_signal_with_label(label, signal_event),
            };
            if !was_removed {
                return Err(match signal {
                    IndexOrLabelSelector::Index(index) => {
                        format!("signal with index '{index}' not found.")
                    }
                    IndexOrLabelSelector::Label(label) => {
                        format!("signal with label '{label}' not found.")
                    }
                });
            }
            Ok(String::new())
        }
        SignalCommand::List => Ok(every_signal_as_pretty_json(signal_event) + "\n"),
    }
}

fn signal_of_definition(definition: &SignalDefinition) -> Result<Signal, String> {
    let (app, app_regex_exclude) =
        pattern_and_whether_it_is_negated(&definition.app, &definition.app_not);
    let (title, title_regex_exclude) =
        pattern_and_whether_it_is_negated(&definition.title, &definition.title_not);
    Ok(Signal {
        app_regex: compile_optional_pattern(app.as_deref(), "--app")?,
        title_regex: compile_optional_pattern(title.as_deref(), "--title")?,
        app,
        title,
        app_regex_exclude,
        title_regex_exclude,
        active: match definition.active {
            None => SignalPropertyRequirement::Undefined,
            Some(OnOrOff::On) => SignalPropertyRequirement::Yes,
            Some(OnOrOff::Off) => SignalPropertyRequirement::No,
        },
        command: Some(definition.action.clone()),
        label: definition.label.clone(),
    })
}

fn pattern_and_whether_it_is_negated(
    pattern: &Option<String>,
    negated_pattern: &Option<String>,
) -> (Option<String>, bool) {
    match (pattern, negated_pattern) {
        (Some(pattern), _) => (Some(pattern.clone()), false),
        (None, Some(negated_pattern)) => (Some(negated_pattern.clone()), true),
        (None, None) => (None, false),
    }
}

fn compile_optional_pattern(pattern: Option<&str>, flag: &str) -> Result<Option<Regex>, String> {
    pattern
        .map(|pattern| {
            Regex::new(pattern).map_err(|_| format!("invalid regex pattern '{pattern}' for {flag}"))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::run_signal_command;
    use crate::command::selectors::IndexOrLabelSelector;
    use crate::command::signal::{SignalCommand, SignalDefinition};
    use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};

    fn definition_of(event: SignalType, label: &str) -> SignalDefinition {
        SignalDefinition {
            event,
            action: String::from("true"),
            label: Some(String::from(label)),
            app: None,
            app_not: Some(String::from("^Finder$")),
            title: None,
            title_not: None,
            active: None,
        }
    }

    #[test]
    fn a_signal_with_the_label_of_another_replaces_it() {
        let mut signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT] = Default::default();

        for event in [SignalType::SpaceChanged, SignalType::SpaceCreated] {
            run_signal_command(
                SignalCommand::Add(definition_of(event, "padding")),
                &mut signal_event,
            )
            .unwrap();
        }

        assert!(signal_event[SignalType::SpaceChanged as usize].is_empty());
        let replacement = &signal_event[SignalType::SpaceCreated as usize][0];
        assert!(replacement.app_regex_exclude);
        assert!(replacement.app_regex.is_some());
    }

    #[test]
    fn removing_a_signal_that_is_not_there_fails_naming_it() {
        let mut signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT] = Default::default();

        let outcome = run_signal_command(
            SignalCommand::Remove {
                signal: IndexOrLabelSelector::Label(String::from("padding")),
            },
            &mut signal_event,
        );

        assert_eq!(
            outcome,
            Err(String::from("signal with label 'padding' not found."))
        );
    }

    #[test]
    fn a_signal_is_removed_by_the_index_the_listing_gives_it_across_events() {
        let mut signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT] = Default::default();
        for (event, label) in [
            (SignalType::WindowFocused, "first"),
            (SignalType::SpaceChanged, "second"),
            (SignalType::SpaceChanged, "third"),
        ] {
            run_signal_command(
                SignalCommand::Add(definition_of(event, label)),
                &mut signal_event,
            )
            .unwrap();
        }

        run_signal_command(
            SignalCommand::Remove {
                signal: IndexOrLabelSelector::Index(1),
            },
            &mut signal_event,
        )
        .unwrap();

        let labels_left: Vec<Option<&str>> = signal_event
            .iter()
            .flatten()
            .map(|signal| signal.label.as_deref())
            .collect();
        assert_eq!(labels_left, [Some("first"), Some("third")]);
    }
}
