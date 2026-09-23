//! Security-event spike detection for the Slack alert layer (ISO 27001 A.8.16).
//!
//! The 5xx alert says when *we* break. This module says when someone is
//! pushing on the auth surface: a run of rejected credentials (forged or
//! replayed tokens) or of rate-limit rejections.
//!
//! Counting is per isolate, in memory, over fixed windows. The free plan gives
//! no cheap shared counter: KV allows about 1k writes a day, and Durable Objects
//! cannot be deployed through the PUT fallback. A burst from one client lands on
//! one or a few isolates, so per-isolate counts still catch it; the real total
//! across the edge is at least the number reported. Alerts are rate-limited by a
//! cooldown so a sustained attack produces one message, not one a minute.
//!
//! What counts (see [`classify`]):
//!
//! * **429**: every one. Each means a limiter already tripped.
//! * **401 with credentials**: a request that carried a bearer token or the
//!   session cookie and was still refused. A signed-out visitor's 401 is
//!   routine, since the landing and discover pages probe `/api/auth/me` for
//!   everyone. That probe is excluded even with a cookie: an expired session
//!   refused there means "sign in again", not an attack.

/// A security-relevant response worth counting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecuritySignal {
    /// 401 on a request that presented a token.
    RejectedCredentials,
    /// 429 from the rate limiter.
    RateLimited,
}

/// The session probe whose 401 means "signed out", not "attack".
pub const SESSION_PROBE_PATH: &str = "/api/auth/me";

impl SecuritySignal {
    pub const ALL: [Self; 2] = [Self::RejectedCredentials, Self::RateLimited];

    /// Index into a per-signal counter array.
    pub fn index(self) -> usize {
        match self {
            Self::RejectedCredentials => 0,
            Self::RateLimited => 1,
        }
    }

    pub fn rule(self) -> SpikeRule {
        match self {
            Self::RejectedCredentials => SpikeRule {
                threshold: 20,
                window_ms: 60_000,
                cooldown_ms: 15 * 60_000,
            },
            Self::RateLimited => SpikeRule {
                threshold: 10,
                window_ms: 60_000,
                cooldown_ms: 15 * 60_000,
            },
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::RejectedCredentials => "401 with credentials presented",
            Self::RateLimited => "429 rate-limited",
        }
    }
}

/// Decide whether a response is a security signal.
pub fn classify(status: u16, had_credentials: bool, path: &str) -> Option<SecuritySignal> {
    match status {
        429 => Some(SecuritySignal::RateLimited),
        401 if had_credentials && path != SESSION_PROBE_PATH => {
            Some(SecuritySignal::RejectedCredentials)
        }
        _ => None,
    }
}

/// When a burst is a spike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpikeRule {
    /// Events within one window that trigger an alert.
    pub threshold: u32,
    pub window_ms: u64,
    /// Minimum gap between two alerts for the same signal.
    pub cooldown_ms: u64,
}

/// Fixed-window counter for one signal.
#[derive(Debug, Clone)]
pub struct SpikeCounter {
    rule: SpikeRule,
    window_start_ms: u64,
    count: u32,
    last_alert_ms: Option<u64>,
}

impl SpikeCounter {
    pub const fn new(rule: SpikeRule) -> Self {
        Self {
            rule,
            window_start_ms: 0,
            count: 0,
            last_alert_ms: None,
        }
    }

    /// Record one event. Returns the window count exactly once per spike: on
    /// the event that reaches the threshold, unless an alert went out within
    /// the cooldown. A clock that moves backwards starts a new window.
    pub fn record(&mut self, now_ms: u64) -> Option<u32> {
        let elapsed = now_ms.checked_sub(self.window_start_ms);
        if elapsed.is_none_or(|e| e >= self.rule.window_ms) {
            self.window_start_ms = now_ms;
            self.count = 0;
        }
        self.count = self.count.saturating_add(1);
        if self.count != self.rule.threshold {
            return None;
        }
        let cooled = self.last_alert_ms.is_none_or(|last| {
            now_ms
                .checked_sub(last)
                .is_none_or(|gap| gap >= self.rule.cooldown_ms)
        });
        if !cooled {
            return None;
        }
        self.last_alert_ms = Some(now_ms);
        Some(self.count)
    }
}
