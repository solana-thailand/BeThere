//! Replay window for claim capability tokens (Issue 071).
//!
//! Claim, quiz and adventure tokens travel in the URL **path**, so Cloudflare's
//! own per-request log records them regardless of what the Worker logs. Issue
//! 070 cleaned the Worker's structured stream; it cannot reach the platform
//! log. Anyone with Workers Logs read access can therefore replay a claim for
//! as long as the token is accepted — which, before this module, was forever.
//!
//! Option 2 of Issue 071: keep the URL shape and the UX, but bound the window.
//!
//! **The anchor is check-in, not issuance.** The token is a UUID v7 minted at
//! *registration* (`handlers/register/signup.rs`) and reused verbatim at
//! check-in, so a TTL measured from issuance would start ticking weeks before
//! the event and expire attendees mid-event. Check-in is when the capability
//! becomes meaningful: it is when the claim QR is generated, and both the claim
//! and quiz paths already require a checked-in attendee.
//!
//! A token whose attendee has not checked in is *not* expired here — it has no
//! anchor yet, and the existing "you must be checked in first" guards already
//! make it useless for claiming.

/// Default replay window: 30 days after check-in.
///
/// Chosen to be far longer than any realistic claim (attendees claim at or
/// shortly after the event) while replacing an unbounded window with a bounded
/// one. Override per environment with `CLAIM_TOKEN_TTL_SECS`; `0` disables the
/// check entirely and is the operational kill switch if it ever denies a real
/// attendee.
pub(crate) const DEFAULT_CLAIM_TOKEN_TTL_SECS: i64 = 30 * 86_400;

/// Policy applied when a claim token is exchanged for an attendee.
///
/// Passed explicitly at every resolution site so the compiler — not a lint and
/// not review — guarantees no path resolves a token without stating its policy.
/// Callers that deliberately need the raw row regardless of age (admin repair,
/// deletion by token) say so with [`ClaimTokenPolicy::unrestricted`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClaimTokenPolicy {
    /// Seconds after `checked_in_at` that the token stays usable. `0` disables.
    pub ttl_secs: i64,
    /// Evaluation time, unix seconds. Injected so the logic stays pure.
    pub now_unix: i64,
}

impl ClaimTokenPolicy {
    /// The configured window, evaluated now.
    pub(crate) fn enforced(ttl_secs: i64) -> Self {
        Self {
            ttl_secs,
            now_unix: chrono::Utc::now().timestamp(),
        }
    }

    /// No expiry. For admin/maintenance paths that must see the row whatever
    /// its age — deleting an attendee by token, repairing a missing token.
    pub(crate) fn unrestricted() -> Self {
        Self {
            ttl_secs: 0,
            now_unix: 0,
        }
    }

    /// Whether a token anchored at `checked_in_at` is outside the window.
    pub(crate) fn is_expired(&self, checked_in_at: Option<&str>) -> bool {
        match self.ttl_secs {
            // Disabled, or `unrestricted()`.
            ttl if ttl <= 0 => false,
            ttl => match checked_in_at_unix(checked_in_at) {
                // Not checked in yet — no anchor, and the claim/quiz paths
                // already refuse. Nothing to expire.
                None => false,
                Some(anchor) => self.now_unix > anchor.saturating_add(ttl),
            },
        }
    }
}

/// Parse a stored `checked_in_at` into unix seconds.
///
/// Timestamps are written as RFC 3339 (`chrono::Utc::now().to_rfc3339()` in
/// `handlers/checkin.rs` and `virtual_checkin.rs`). Returns `None` for absent,
/// empty or unparseable values — an unparseable timestamp is a data bug, and
/// treating it as "expired" would lock a real attendee out of an NFT they are
/// entitled to, so this fails **open** and leaves the existing guards in charge.
fn checked_in_at_unix(checked_in_at: Option<&str>) -> Option<i64> {
    let raw = checked_in_at?.trim();
    match raw.is_empty() {
        true => None,
        false => match chrono::DateTime::parse_from_rfc3339(raw) {
            Ok(dt) => Some(dt.timestamp()),
            Err(_) => {
                tracing::warn!(
                    "claim token policy: unparseable checked_in_at, not applying the replay window"
                );
                None
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-09-01T00:00:00Z
    const CHECKED_IN: &str = "2026-09-01T00:00:00+00:00";
    const CHECKED_IN_UNIX: i64 = 1_788_220_800;
    const DAY: i64 = 86_400;

    fn at(now_unix: i64) -> ClaimTokenPolicy {
        ClaimTokenPolicy {
            ttl_secs: 30 * DAY,
            now_unix,
        }
    }

    #[test]
    fn anchor_parses_to_the_expected_instant() {
        assert_eq!(checked_in_at_unix(Some(CHECKED_IN)), Some(CHECKED_IN_UNIX));
    }

    #[test]
    fn inside_the_window_is_live() {
        assert!(!at(CHECKED_IN_UNIX).is_expired(Some(CHECKED_IN)));
        assert!(!at(CHECKED_IN_UNIX + 29 * DAY).is_expired(Some(CHECKED_IN)));
        // Exactly on the boundary is still live — expiry is strictly after.
        assert!(!at(CHECKED_IN_UNIX + 30 * DAY).is_expired(Some(CHECKED_IN)));
    }

    #[test]
    fn outside_the_window_is_expired() {
        assert!(at(CHECKED_IN_UNIX + 30 * DAY + 1).is_expired(Some(CHECKED_IN)));
        assert!(at(CHECKED_IN_UNIX + 365 * DAY).is_expired(Some(CHECKED_IN)));
    }

    #[test]
    fn not_checked_in_has_no_anchor_so_never_expires() {
        let far_future = at(CHECKED_IN_UNIX + 10_000 * DAY);
        assert!(!far_future.is_expired(None));
        assert!(!far_future.is_expired(Some("")));
        assert!(!far_future.is_expired(Some("   ")));
    }

    #[test]
    fn unparseable_timestamp_fails_open() {
        let far_future = at(CHECKED_IN_UNIX + 10_000 * DAY);
        assert!(!far_future.is_expired(Some("not a timestamp")));
        // Date-only is not RFC 3339 and must not be guessed at.
        assert!(!far_future.is_expired(Some("2026-09-01")));
    }

    #[test]
    fn zero_ttl_disables_the_window() {
        let disabled = ClaimTokenPolicy {
            ttl_secs: 0,
            now_unix: CHECKED_IN_UNIX + 10_000 * DAY,
        };
        assert!(!disabled.is_expired(Some(CHECKED_IN)));
    }

    #[test]
    fn negative_ttl_is_treated_as_disabled_not_as_expire_everything() {
        // A misconfigured CLAIM_TOKEN_TTL_SECS must not lock every attendee out.
        let misconfigured = ClaimTokenPolicy {
            ttl_secs: -1,
            now_unix: CHECKED_IN_UNIX + DAY,
        };
        assert!(!misconfigured.is_expired(Some(CHECKED_IN)));
    }

    #[test]
    fn unrestricted_never_expires() {
        assert!(!ClaimTokenPolicy::unrestricted().is_expired(Some(CHECKED_IN)));
    }

    #[test]
    fn overflow_in_the_anchor_cannot_panic() {
        let policy = ClaimTokenPolicy {
            ttl_secs: i64::MAX,
            now_unix: i64::MAX,
        };
        // saturating_add keeps this at i64::MAX, so `now > anchor + ttl` is false.
        assert!(!policy.is_expired(Some(CHECKED_IN)));
    }
}
