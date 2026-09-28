use std::time::Duration;

use crate::ffi::accessibility::query_accessibility_trust_without_prompting_the_user;
use crate::warn;

const INTERVAL_BETWEEN_ACCESSIBILITY_TRUST_CHECKS: Duration = Duration::from_millis(500);

pub(crate) fn wait_until_accessibility_is_trusted_announcing_the_wait_once() {
    warn!("yabai: waiting for accessibility permission to be granted..\n");
    while !query_accessibility_trust_without_prompting_the_user() {
        std::thread::sleep(INTERVAL_BETWEEN_ACCESSIBILITY_TRUST_CHECKS);
    }
}
