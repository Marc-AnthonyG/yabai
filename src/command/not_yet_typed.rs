use crate::command::DaemonCommand;

const DOMAINS_STILL_REACHED_THROUGH_THE_MESSAGE_OPTION: [&str; 3] = ["display", "space", "window"];

pub(crate) fn command_not_yet_typed_from_the_message_arguments(
    arguments: Vec<String>,
) -> Result<DaemonCommand, String> {
    let domain = arguments.first().map_or("", String::as_str);
    if !DOMAINS_STILL_REACHED_THROUGH_THE_MESSAGE_OPTION.contains(&domain) {
        return Err(format!(
            "`-m {domain}` is not a command; run `yabai --help` for the commands"
        ));
    }
    Ok(DaemonCommand::NotYetTyped { arguments })
}

#[cfg(test)]
mod tests {
    use super::command_not_yet_typed_from_the_message_arguments;

    fn arguments(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| String::from(*word)).collect()
    }

    #[test]
    fn a_message_to_a_domain_not_yet_typed_passes_every_argument_on() {
        assert!(
            command_not_yet_typed_from_the_message_arguments(arguments(&[
                "window", "--focus", "west"
            ]))
            .is_ok()
        );
    }

    #[test]
    fn a_message_to_a_typed_domain_is_refused() {
        assert!(
            command_not_yet_typed_from_the_message_arguments(arguments(&["config", "layout"]))
                .is_err()
        );
    }

    #[test]
    fn a_message_to_an_unknown_domain_is_refused() {
        assert!(
            command_not_yet_typed_from_the_message_arguments(arguments(&["--focus", "west"]))
                .is_err()
        );
    }
}
