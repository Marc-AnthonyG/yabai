const SHORTEST_INTERVAL_BETWEEN_PROXY_ALPHA_REFRESHES_IN_SECONDS: f64 = 1.0 / 30.0;

pub(crate) struct ProxyAlphaRefreshSchedule {
    last_refresh_host_time: Option<u64>,
}

impl ProxyAlphaRefreshSchedule {
    pub(crate) fn fresh_from_the_proxy_build() -> ProxyAlphaRefreshSchedule {
        ProxyAlphaRefreshSchedule {
            last_refresh_host_time: None,
        }
    }

    pub(crate) fn claim_refresh_if_due(
        &mut self,
        host_time: u64,
        host_clock_frequency: f64,
    ) -> bool {
        let Some(last_refresh_host_time) = self.last_refresh_host_time else {
            self.last_refresh_host_time = Some(host_time);
            return false;
        };

        let elapsed_host_ticks = host_time.saturating_sub(last_refresh_host_time) as f64;
        let shortest_interval_in_host_ticks =
            SHORTEST_INTERVAL_BETWEEN_PROXY_ALPHA_REFRESHES_IN_SECONDS * host_clock_frequency;
        if elapsed_host_ticks < shortest_interval_in_host_ticks {
            return false;
        }

        self.last_refresh_host_time = Some(host_time);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ProxyAlphaRefreshSchedule;

    const HOST_CLOCK_FREQUENCY: f64 = 24_000_000.0;
    const HOST_TICKS_IN_A_THIRTIETH_OF_A_SECOND: u64 = 800_000;
    const HOST_TIME_OF_THE_FIRST_TICK: u64 = 1_000_000_000;

    fn schedule_whose_first_tick_came_at(host_time: u64) -> ProxyAlphaRefreshSchedule {
        let mut proxy_alpha_refresh_schedule =
            ProxyAlphaRefreshSchedule::fresh_from_the_proxy_build();
        proxy_alpha_refresh_schedule.claim_refresh_if_due(host_time, HOST_CLOCK_FREQUENCY);
        proxy_alpha_refresh_schedule
    }

    #[test]
    fn the_first_tick_after_the_build_does_not_refresh_the_alpha_the_build_just_took() {
        let mut proxy_alpha_refresh_schedule =
            ProxyAlphaRefreshSchedule::fresh_from_the_proxy_build();

        assert!(
            !proxy_alpha_refresh_schedule
                .claim_refresh_if_due(HOST_TIME_OF_THE_FIRST_TICK, HOST_CLOCK_FREQUENCY)
        );
    }

    #[test]
    fn a_tick_less_than_a_thirtieth_of_a_second_after_the_last_refresh_is_not_due() {
        let mut proxy_alpha_refresh_schedule =
            schedule_whose_first_tick_came_at(HOST_TIME_OF_THE_FIRST_TICK);

        for host_ticks_since_the_first_tick in
            [0, 1, 400_000, HOST_TICKS_IN_A_THIRTIETH_OF_A_SECOND - 1]
        {
            assert!(
                !proxy_alpha_refresh_schedule.claim_refresh_if_due(
                    HOST_TIME_OF_THE_FIRST_TICK + host_ticks_since_the_first_tick,
                    HOST_CLOCK_FREQUENCY
                ),
                "{host_ticks_since_the_first_tick} host ticks after the first tick"
            );
        }
    }

    #[test]
    fn a_tick_a_thirtieth_of_a_second_after_the_last_refresh_is_due_and_restarts_the_interval() {
        let mut proxy_alpha_refresh_schedule =
            schedule_whose_first_tick_came_at(HOST_TIME_OF_THE_FIRST_TICK);
        let host_time_of_the_refresh =
            HOST_TIME_OF_THE_FIRST_TICK + HOST_TICKS_IN_A_THIRTIETH_OF_A_SECOND;

        assert!(
            proxy_alpha_refresh_schedule
                .claim_refresh_if_due(host_time_of_the_refresh, HOST_CLOCK_FREQUENCY)
        );
        assert!(!proxy_alpha_refresh_schedule.claim_refresh_if_due(
            host_time_of_the_refresh + HOST_TICKS_IN_A_THIRTIETH_OF_A_SECOND - 1,
            HOST_CLOCK_FREQUENCY
        ));
        assert!(proxy_alpha_refresh_schedule.claim_refresh_if_due(
            host_time_of_the_refresh + 3 * HOST_TICKS_IN_A_THIRTIETH_OF_A_SECOND,
            HOST_CLOCK_FREQUENCY
        ));
    }

    #[test]
    fn at_sixty_ticks_a_second_the_alpha_is_refreshed_on_every_other_tick() {
        let mut proxy_alpha_refresh_schedule =
            ProxyAlphaRefreshSchedule::fresh_from_the_proxy_build();

        let refreshed_ticks: Vec<u64> = (0..9)
            .filter(|tick_index| {
                proxy_alpha_refresh_schedule.claim_refresh_if_due(
                    HOST_TIME_OF_THE_FIRST_TICK + tick_index * 400_000,
                    HOST_CLOCK_FREQUENCY,
                )
            })
            .collect();

        assert_eq!(refreshed_ticks, vec![2, 4, 6, 8]);
    }

    #[test]
    fn a_tick_whose_host_time_is_before_the_last_refresh_is_not_due() {
        let mut proxy_alpha_refresh_schedule =
            schedule_whose_first_tick_came_at(HOST_TIME_OF_THE_FIRST_TICK);

        assert!(!proxy_alpha_refresh_schedule.claim_refresh_if_due(
            HOST_TIME_OF_THE_FIRST_TICK - 5_000_000,
            HOST_CLOCK_FREQUENCY
        ));
    }
}
