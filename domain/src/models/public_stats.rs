//! `GET /api/public/stats`: the landing page's numbers, aggregates only.
//!
//! Every number on the landing comes from here, with `measured_at` beside it
//! (BETHERE-BUILD-PLAN rule 1). Definitions, all over events that are not
//! draft or archived (a draft never ran; archived holds tests):
//!
//! - `events_held`: active or completed events whose end has passed. Not
//!   `COUNT(*)`: a draft is an event that never ran.
//! - `onsite_registrations` / `online_registrations`: attendee rows on each
//!   track (walk-ins count on site); staff invitations (`approval_status =
//!   'invited'`) excluded.
//! - `door_scans`: on-site rows with a check-in time.
//! - `deposits_handled_*`: verified THB deposits that moved money (cash, or
//!   credit carried from an earlier event; legacy rows with no source count as
//!   cash); staff comps and ฿0 rows excluded.
//! - `deposit_payers` / `deposit_payers_came`: of those deposits, the ones whose
//!   attendee is still on the on-site track, and how many of them were checked
//!   in. Only events whose deposits BeThere recorded: deposits taken by hand
//!   before the system are not in it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicStats {
    /// When these were counted (RFC 3339, UTC).
    pub measured_at: String,
    pub events_held: u32,
    pub onsite_registrations: u32,
    pub online_registrations: u32,
    pub door_scans: u32,
    pub deposits_handled_count: u32,
    pub deposits_handled_thb: u64,
    pub deposit_payers: u32,
    pub deposit_payers_came: u32,
}

/// Which landing count a stored `participation_type` belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsTrack {
    OnSite,
    Online,
    /// Retrospective leads and unknown values ("test"): in no track count.
    Neither,
}

/// The track of one stored value, by the app's own parser: a staff walk-in is
/// on site, as in `TrackCounts::add`.
pub fn stats_track(stored: &str) -> StatsTrack {
    use super::attendee::{PARTICIPATION_WALK_IN, ParticipationType};
    if stored == PARTICIPATION_WALK_IN {
        return StatsTrack::OnSite;
    }
    match ParticipationType::parse(stored) {
        ParticipationType::InPerson => StatsTrack::OnSite,
        ParticipationType::Online => StatsTrack::Online,
        ParticipationType::Retrospective | ParticipationType::Other => StatsTrack::Neither,
    }
}

/// One `(kind, stored participation_type, n, checked_in)` row of the stats query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsRow {
    pub kind: String,
    pub participation_type: String,
    pub n: u64,
    pub checked_in: u64,
}

/// Fold the query's rows into the landing numbers.
pub fn fold_stats(rows: &[StatsRow], measured_at: String) -> PublicStats {
    let small = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    let mut s = PublicStats {
        measured_at,
        ..PublicStats::default()
    };
    for r in rows {
        let track = stats_track(&r.participation_type);
        match (r.kind.as_str(), track) {
            ("held", _) => s.events_held = small(r.n),
            ("reg", StatsTrack::OnSite) => {
                s.onsite_registrations = s.onsite_registrations.saturating_add(small(r.n));
                s.door_scans = s.door_scans.saturating_add(small(r.checked_in));
            }
            ("reg", StatsTrack::Online) => {
                s.online_registrations = s.online_registrations.saturating_add(small(r.n));
            }
            ("paid", _) => {
                s.deposits_handled_count = s.deposits_handled_count.saturating_add(small(r.n));
                if track == StatsTrack::OnSite {
                    s.deposit_payers = s.deposit_payers.saturating_add(small(r.n));
                    s.deposit_payers_came =
                        s.deposit_payers_came.saturating_add(small(r.checked_in));
                }
            }
            ("thb", _) => s.deposits_handled_thb = r.n,
            _ => {}
        }
    }
    s
}
