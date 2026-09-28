use std::time::Duration;

const INTERVAL_BETWEEN_SCREEN_RECORDING_GRANT_CHECKS: Duration = Duration::from_secs(2);
const LONGEST_WAIT_FOR_A_SCREEN_RECORDING_GRANT: Duration = Duration::from_secs(10 * 60);

pub(crate) fn times_since_watching_began_to_check_for_a_screen_recording_grant()
-> impl Iterator<Item = Duration> {
    (1u32..)
        .map(|check_number| INTERVAL_BETWEEN_SCREEN_RECORDING_GRANT_CHECKS * check_number)
        .take_while(|time_since_watching_began| {
            *time_since_watching_began <= LONGEST_WAIT_FOR_A_SCREEN_RECORDING_GRANT
        })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::times_since_watching_began_to_check_for_a_screen_recording_grant;

    #[test]
    fn the_first_check_comes_two_seconds_after_watching_begins() {
        assert_eq!(
            times_since_watching_began_to_check_for_a_screen_recording_grant().next(),
            Some(Duration::from_secs(2))
        );
    }

    #[test]
    fn every_check_comes_two_seconds_after_the_one_before() {
        let check_times: Vec<Duration> =
            times_since_watching_began_to_check_for_a_screen_recording_grant().collect();

        for consecutive_check_times in check_times.windows(2) {
            assert_eq!(
                consecutive_check_times[1] - consecutive_check_times[0],
                Duration::from_secs(2),
                "{consecutive_check_times:?}"
            );
        }
    }

    #[test]
    fn the_last_check_comes_exactly_ten_minutes_after_watching_begins() {
        assert_eq!(
            times_since_watching_began_to_check_for_a_screen_recording_grant().last(),
            Some(Duration::from_secs(10 * 60))
        );
    }

    #[test]
    fn watching_gives_up_after_three_hundred_checks() {
        assert_eq!(
            times_since_watching_began_to_check_for_a_screen_recording_grant().count(),
            300
        );
    }
}
