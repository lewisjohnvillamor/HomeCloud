//! In-process attempt limiting.
//!
//! Deliberately not a distributed rate limiter: a home deployment is one
//! process, and adding Redis for this would fail the "measured need"
//! test in `AGENTS.md`. Losing counters on restart is acceptable — the
//! goal is to make online password guessing slow, not to build an audit
//! trail.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Failures tolerated inside one window before the account is paused.
const MAX_FAILURES: u32 = 10;

/// How long failures are remembered, and how long a lockout lasts.
const WINDOW: Duration = Duration::from_secs(15 * 60);

/// Most distinct keys tracked at once. Bounded so a flood of made-up
/// account names cannot grow this map without limit.
const MAX_TRACKED_KEYS: usize = 4096;

#[derive(Debug)]
struct Attempts {
    failures: u32,
    first_failure: Instant,
}

/// Tracks failed attempts per key (here, per account name).
#[derive(Debug, Default)]
pub struct AttemptLimiter {
    entries: Mutex<HashMap<String, Attempts>>,
}

impl AttemptLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `Err(retry_after)` when the key is currently locked out.
    pub fn check(&self, key: &str) -> Result<(), Duration> {
        let mut entries = self.lock();

        let Some(attempts) = entries.get(key) else {
            return Ok(());
        };

        let elapsed = attempts.first_failure.elapsed();
        if elapsed >= WINDOW {
            entries.remove(key);
            return Ok(());
        }

        if attempts.failures >= MAX_FAILURES {
            return Err(WINDOW - elapsed);
        }

        Ok(())
    }

    pub fn record_failure(&self, key: &str) {
        let mut entries = self.lock();

        if entries.len() >= MAX_TRACKED_KEYS {
            entries.retain(|_, attempts| attempts.first_failure.elapsed() < WINDOW);
        }

        match entries.get_mut(key) {
            Some(attempts) if attempts.first_failure.elapsed() < WINDOW => {
                attempts.failures += 1;
            }
            _ => {
                entries.insert(
                    key.to_owned(),
                    Attempts {
                        failures: 1,
                        first_failure: Instant::now(),
                    },
                );
            }
        }
    }

    /// A successful sign-in clears the count for that key.
    pub fn record_success(&self, key: &str) {
        self.lock().remove(key);
    }

    /// Recovers from a poisoned lock rather than propagating a panic: a
    /// panic elsewhere must not lock everyone out of signing in.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Attempts>> {
        self.entries.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("attempt limiter lock was poisoned; continuing");
            poisoned.into_inner()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_key_is_allowed() {
        assert!(AttemptLimiter::new().check("ada").is_ok());
    }

    #[test]
    fn repeated_failures_lock_the_key_out() {
        let limiter = AttemptLimiter::new();

        for _ in 0..MAX_FAILURES {
            assert!(limiter.check("ada").is_ok());
            limiter.record_failure("ada");
        }

        assert!(limiter.check("ada").is_err());
    }

    #[test]
    fn a_successful_sign_in_clears_the_count() {
        let limiter = AttemptLimiter::new();
        for _ in 0..MAX_FAILURES {
            limiter.record_failure("ada");
        }

        limiter.record_success("ada");

        assert!(limiter.check("ada").is_ok());
    }

    #[test]
    fn one_account_cannot_lock_out_another() {
        let limiter = AttemptLimiter::new();
        for _ in 0..MAX_FAILURES {
            limiter.record_failure("ada");
        }

        assert!(limiter.check("grace").is_ok());
    }
}
