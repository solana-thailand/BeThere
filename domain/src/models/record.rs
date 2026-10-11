//! Your record (Super GOAT SG1): every door a person was registered for, lit
//! where they came. `door_history` is the single definition, used by
//! `GET /api/me/record`, the `/me/record` page and any deposit rule, so they
//! cannot count differently (SUPER-GOAT-SPEC §SG1, `site_returns.py`):
//!
//! - **Door events:** the events in a campaign (the series the community
//!   runs: Road to Mainnet today). Smoke, sandbox and one-off test events are
//!   in none, so they are out without a list.
//! - **Not counted:** staff invitations (`invited`, filtered in SQL), the
//!   event's own staff and organizers, the online track (the door is the
//!   measure; walk-ins are on site, as in the public stats).
//! - **Came:** a check-in time that is not virtual. A virtual check-in (the
//!   claim flow, `checked_in_by = 'virtual'`) is not a door.
//! - **One person:** the rows of every linked email (`person_emails`) fold
//!   into one entry per event; came if any of them came.
//!
//! Registered and missed stays in (`came: false`): the record is honest both
//! ways. What a miss costs is the organizer's rule, not this function's. An
//! event that has not ended and was not (yet) checked into is `upcoming`, not
//! a miss.

use serde::{Deserialize, Serialize};

use super::public_stats::{StatsTrack, stats_track};

/// `checked_in_by` of the claim flow's check-in: not a door.
pub const VIRTUAL_CHECKIN: &str = "virtual";

/// One attendee row of one of the person's emails at a door event.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DoorRow {
    pub event_slug: String,
    pub event_name: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub participation_type: String,
    pub checked_in: bool,
    pub checked_in_by: String,
    /// The email is on the event's own staff or organizer list.
    pub crew: bool,
}

/// One door event in the person's record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attended {
    pub event_slug: String,
    pub event_name: String,
    pub start_ms: i64,
    pub came: bool,
    /// Not ended and not checked into: neither came nor missed yet.
    #[serde(default)]
    pub upcoming: bool,
}

/// The person's door history at `now_ms`, oldest first, one entry per event.
pub fn door_history(rows: &[DoorRow], now_ms: i64) -> Vec<Attended> {
    let mut out: Vec<Attended> = Vec::new();
    for r in rows {
        if r.crew || stats_track(&r.participation_type) != StatsTrack::OnSite {
            continue;
        }
        let came = r.checked_in && r.checked_in_by != VIRTUAL_CHECKIN;
        match out.iter_mut().find(|a| a.event_slug == r.event_slug) {
            Some(a) => a.came |= came,
            None => out.push(Attended {
                event_slug: r.event_slug.clone(),
                event_name: r.event_name.clone(),
                start_ms: r.start_ms,
                came,
                upcoming: r.end_ms > now_ms,
            }),
        }
    }
    for a in &mut out {
        a.upcoming &= !a.came;
    }
    out.sort_by(|a, b| (a.start_ms, &a.event_slug).cmp(&(b.start_ms, &b.event_slug)));
    out
}

/// Events that are settled: came, or ended without a check-in.
pub fn settled(history: &[Attended]) -> u32 {
    u32::try_from(history.iter().filter(|a| !a.upcoming).count()).unwrap_or(u32::MAX)
}

/// Door check-ins: the number a regular rule compares against.
pub fn door_checkins(history: &[Attended]) -> u32 {
    u32::try_from(history.iter().filter(|a| a.came).count()).unwrap_or(u32::MAX)
}

/// A badge the person claimed (any event, door or not).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Badge {
    pub event_slug: String,
    pub event_name: String,
    pub asset_id: String,
}

/// `GET /api/me/record`: the signed-in person's own record, never another's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyRecord {
    pub events: Vec<Attended>,
    pub door_checkins: u32,
    #[serde(default)]
    pub badges: Vec<Badge>,
    /// The cluster the badges were minted on (`mainnet-beta` or `devnet`),
    /// for the explorer links.
    #[serde(default)]
    pub badge_cluster: String,
    /// The person's live "I showed up" links (SG4), one per event at most.
    #[serde(default)]
    pub shared: Vec<SharedLink>,
}

/// A live "I showed up" link (Super GOAT SG4): `/showed/{token}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedLink {
    pub event_slug: String,
    pub token: String,
}

/// What `/showed/{token}` shows: the event, never the person.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowedPage {
    pub event_name: String,
    pub event_slug: String,
    pub start_ms: i64,
    /// The card image path (the "I showed up" card, else the event card), or
    /// empty for none.
    #[serde(default)]
    pub card: String,
}

/// Length of a share token: 16 random bytes as hex.
pub const SHOWED_TOKEN_LEN: usize = 32;

/// Whether `token` has a share token's shape (lowercase hex, exact length).
/// A shape check only: whether it is live is D1's answer.
pub fn is_showed_token(token: &str) -> bool {
    token.len() == SHOWED_TOKEN_LEN
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The share decision: a link only for an event the history says the
/// person came to.
pub fn may_share(history: &[Attended], event_slug: &str) -> bool {
    history.iter().any(|a| a.came && a.event_slug == event_slug)
}
