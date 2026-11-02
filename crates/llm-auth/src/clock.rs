use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};

/// Trait for querying current time, allowing deterministic unit testing.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Standard system wall-clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Controllable clock for unit tests.
#[derive(Debug, Clone)]
pub struct MockClock {
    current: Arc<Mutex<DateTime<Utc>>>,
}

impl MockClock {
    pub fn new(initial: DateTime<Utc>) -> Self {
        Self {
            current: Arc::new(Mutex::new(initial)),
        }
    }

    pub fn advance(&self, duration: Duration) {
        let mut lock = self.current.lock().unwrap();
        *lock += duration;
    }

    pub fn set(&self, new_time: DateTime<Utc>) {
        let mut lock = self.current.lock().unwrap();
        *lock = new_time;
    }
}

impl Clock for MockClock {
    fn now(&self) -> DateTime<Utc> {
        *self.current.lock().unwrap()
    }
}

/// Refresh tokens when less than 5 minutes remain before expiry.
pub const REFRESH_EARLY_THRESHOLD: Duration = Duration::minutes(5);

/// Returns true if an access token with expiration `expires_at` should be refreshed at `now`.
pub fn should_refresh(expires_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };

    expires_at - now < REFRESH_EARLY_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_early_logic_with_fake_clock() {
        let base_time = "2026-11-02T12:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let clock = MockClock::new(base_time);

        // Case 1: Token expires in 10 minutes -> should NOT refresh
        let expires_in_10m = base_time + Duration::minutes(10);
        assert!(!should_refresh(Some(expires_in_10m), clock.now()));

        // Case 2: Advance clock by 5 minutes 1 second (4m 59s remaining < 5m) -> SHOULD refresh
        clock.advance(Duration::seconds(301));
        assert!(should_refresh(Some(expires_in_10m), clock.now()));

        // Case 3: Token already expired -> SHOULD refresh
        let expired = base_time - Duration::minutes(1);
        assert!(should_refresh(Some(expired), clock.now()));

        // Case 4: Token expiring exactly at 5m -> expires_at - now == 5m, not < 5m
        let expires_in_exact_5m = clock.now() + Duration::minutes(5);
        assert!(!should_refresh(Some(expires_in_exact_5m), clock.now()));

        // Case 5: Token expiring at 4m 59s -> SHOULD refresh
        let expires_in_4m59s = clock.now() + Duration::seconds(299);
        assert!(should_refresh(Some(expires_in_4m59s), clock.now()));

        // Case 6: No expiration -> should NOT refresh
        assert!(!should_refresh(None, clock.now()));
    }
}
