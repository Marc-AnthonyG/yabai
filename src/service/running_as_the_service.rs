use std::ffi::OsStr;

use crate::service::launchctl::LAUNCHD_SERVICE_LABEL;

const ENVIRONMENT_VARIABLE_LAUNCHD_SETS_TO_THE_SERVICE_LABEL: &str = "XPC_SERVICE_NAME";

pub fn is_this_process_running_as_the_launchd_service() -> bool {
    is_the_yabai_launchd_service_named_by(
        std::env::var_os(ENVIRONMENT_VARIABLE_LAUNCHD_SETS_TO_THE_SERVICE_LABEL).as_deref(),
    )
}

fn is_the_yabai_launchd_service_named_by(
    service_label_launchd_started_this_process_for: Option<&OsStr>,
) -> bool {
    service_label_launchd_started_this_process_for == Some(OsStr::new(LAUNCHD_SERVICE_LABEL))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::is_the_yabai_launchd_service_named_by;

    #[test]
    fn only_the_exact_yabai_service_label_means_running_as_the_launchd_service() {
        let expected_answers = [
            (Some("com.asmvik.yabai"), true),
            (None, false),
            (Some("0"), false),
            (Some(""), false),
            (Some("com.asmvik.yabai-sa"), false),
            (Some("com.asmvik.yabai.plist"), false),
            (Some("application.com.apple.Terminal.12345.67890"), false),
        ];

        for (service_label, expected_answer) in expected_answers {
            assert_eq!(
                is_the_yabai_launchd_service_named_by(service_label.map(OsStr::new)),
                expected_answer,
                "{service_label:?}"
            );
        }
    }
}
