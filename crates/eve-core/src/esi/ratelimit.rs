//! ESI error-budget circuit breaker.
//!
//! ESI's real limit is not requests-per-second but an **error budget**: the
//! `X-Esi-Error-Limit-Remain` / `X-Esi-Error-Limit-Reset` headers tell you how
//! many erroring requests you have left in the current window. If you hit zero
//! you get blocked. This type tracks the budget and trips a breaker when it
//! runs low so non-essential polling backs off before we get throttled.

use std::time::{Duration, Instant};

/// Below this many remaining errors, the breaker trips and we pause
/// non-essential work until the window resets.
const LOW_WATERMARK: i64 = 20;

/// Tracks the ESI error budget reported by response headers.
#[derive(Debug, Clone)]
pub struct ErrorBudget {
    remain: i64,
    /// When the current error window resets.
    reset_at: Instant,
}

impl Default for ErrorBudget {
    fn default() -> Self {
        // Assume a healthy budget until the server tells us otherwise. ESI's
        // default window is 100 errors / 60s.
        Self {
            remain: 100,
            reset_at: Instant::now(),
        }
    }
}

impl ErrorBudget {
    /// Update the budget from the two ESI headers (values already parsed).
    /// `reset_secs` is the number of seconds until the window resets.
    pub fn observe(&mut self, remain: i64, reset_secs: u64) {
        self.remain = remain;
        self.reset_at = Instant::now() + Duration::from_secs(reset_secs);
    }

    /// Convenience: parse the relevant headers off a response map. Missing or
    /// malformed headers leave the budget unchanged.
    pub fn observe_headers<'a, F>(&mut self, get: F)
    where
        F: Fn(&str) -> Option<&'a str>,
    {
        let remain = get("x-esi-error-limit-remain").and_then(|v| v.parse::<i64>().ok());
        let reset = get("x-esi-error-limit-reset").and_then(|v| v.parse::<u64>().ok());
        if let (Some(remain), Some(reset)) = (remain, reset) {
            self.observe(remain, reset);
        }
    }

    /// How long the caller should wait before issuing non-essential requests.
    /// `Duration::ZERO` means "go ahead".
    pub fn backoff(&self) -> Duration {
        if self.remain > LOW_WATERMARK {
            return Duration::ZERO;
        }
        // Budget is low (or exhausted): wait out the rest of the window.
        self.reset_at.saturating_duration_since(Instant::now())
    }

    /// True when the breaker is tripped (budget low/exhausted and window not yet
    /// elapsed).
    pub fn is_tripped(&self) -> bool {
        self.remain <= LOW_WATERMARK && self.reset_at > Instant::now()
    }

    pub fn remaining(&self) -> i64 {
        self.remain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_budget_does_not_back_off() {
        let mut b = ErrorBudget::default();
        b.observe(80, 60);
        assert!(!b.is_tripped());
        assert_eq!(b.backoff(), Duration::ZERO);
    }

    #[test]
    fn low_budget_trips_breaker() {
        let mut b = ErrorBudget::default();
        b.observe(5, 30);
        assert!(b.is_tripped());
        assert!(b.backoff() > Duration::ZERO);
        assert!(b.backoff() <= Duration::from_secs(30));
    }

    #[test]
    fn parses_headers() {
        let mut b = ErrorBudget::default();
        let headers = std::collections::HashMap::from([
            ("x-esi-error-limit-remain", "9"),
            ("x-esi-error-limit-reset", "15"),
        ]);
        b.observe_headers(|k| headers.get(k).copied());
        assert_eq!(b.remaining(), 9);
        assert!(b.is_tripped());
    }
}
