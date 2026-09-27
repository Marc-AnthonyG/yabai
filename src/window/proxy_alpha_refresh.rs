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
