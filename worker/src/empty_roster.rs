//! What an event's empty D1 roster means (`.issues/167` part B).
//!
//! A duplicated event copies its source's `sheet_id`, and a sheet has no event
//! column. So when D1 has no rows for such an event, the sheet can only return
//! the *source* event's attendees, and registration dedup then hands a new
//! registrant the source row's `api_id` / `claim_token`.
//!
//! Owner decision (2026-09-29): events created after D1 became the
//! authoritative attendee store read an empty roster as empty. Older,
//! sheet-only events keep the sheet fallback.

use chrono::{DateTime, Utc};
use event_checkin_domain::models::event::EventConfig;

/// When D1 became the authoritative attendee store: commit `fa0dca12`
/// ("make D1 attendee write authoritative + fail-closed"), 2026-08-12 01:59
/// +07:00. Every registration of an event created after it writes D1 first
/// and fails closed, so D1's roster is the whole roster.
pub const D1_AUTHORITATIVE_SINCE: &str = "2026-08-11T18:59:34Z";

/// What to do when D1 answers with no attendees for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyRoster {
    /// Read the event's sheet. For legacy events whose attendees live only
    /// there, and for the organizer's explicit sheet sync.
    ReadSheet,
    /// Empty is the answer: the sheet can only hold another event's rows.
    Trust,
}

impl EmptyRoster {
    /// The policy for `config`'s attendee reads.
    pub fn for_event(config: &EventConfig) -> Self {
        Self::for_created_at(&config.created_at)
    }

    /// The policy for an event, from its `created_at` (RFC 3339). An empty or
    /// unparseable timestamp is a legacy event, so it keeps the sheet.
    pub fn for_created_at(created_at: &str) -> Self {
        let Ok(created) = DateTime::parse_from_rfc3339(created_at) else {
            return Self::ReadSheet;
        };
        let Ok(since) = D1_AUTHORITATIVE_SINCE.parse::<DateTime<Utc>>() else {
            return Self::ReadSheet;
        };
        match created.with_timezone(&Utc) >= since {
            true => Self::Trust,
            false => Self::ReadSheet,
        }
    }
}
