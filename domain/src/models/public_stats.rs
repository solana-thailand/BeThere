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
//!   attendee is still on the on-site track and is not on the event's own
//!   staff or organizer list (their deposits still count as handled), and how
//!   many of them were checked in. Comp, staff and online payers are out: the
//!   landing ladder's definition (owner, 6 Oct). Only events whose deposits BeThere recorded: deposits taken by hand
//!   before the system are not in it.
//! - `slip_check`: minutes from a slip's upload to its verification, over the
//!   same deposits.
//! - `refund_after_end`: minutes from the event's end to a refund's recorded
//!   time (0 for a refund made before the end), over refunded deposits that
//!   carry a time. Many older refunds carry none, so the sample is small; it
//!   is published with its size, and not at all under
//!   [`MIN_TIMING_SAMPLES`].

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
    #[serde(default)]
    pub slip_check: Option<Timing>,
    #[serde(default)]
    pub refund_after_end: Option<Timing>,
    /// `deposit_payers` / `deposit_payers_came` per public event, oldest
    /// first (.plans/045 R4.7): the payers' hall. Private events count in the
    /// totals but are not listed.
    #[serde(default)]
    pub payers_by_event: Vec<EventPayers>,
}

/// One public event's deposit payers and how many came, under the
/// `deposit_payers` definition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventPayers {
    pub name: String,
    pub slug: String,
    pub start_ms: i64,
    pub paid: u32,
    pub came: u32,
}

/// One `(event, stored participation_type, n, checked_in)` row of the
/// per-event statement; staff payers are already out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPayerRow {
    pub event_id: String,
    pub name: String,
    pub slug: String,
    pub start_ms: i64,
    pub participation_type: String,
    pub n: u64,
    pub checked_in: u64,
}

/// Fold the per-event rows: an on-site participation type is a payer (the
/// same rule as `fold_stats`); events with no payer are left out. Keeps the
/// statement's order (oldest first).
pub fn fold_event_payers(rows: &[EventPayerRow]) -> Vec<EventPayers> {
    let small = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    let mut out: Vec<(String, EventPayers)> = Vec::new();
    for r in rows {
        if stats_track(&r.participation_type) != StatsTrack::OnSite {
            continue;
        }
        let at = match out.iter().position(|(id, _)| id == &r.event_id) {
            Some(at) => at,
            None => {
                out.push((
                    r.event_id.clone(),
                    EventPayers {
                        name: r.name.clone(),
                        slug: r.slug.clone(),
                        start_ms: r.start_ms,
                        paid: 0,
                        came: 0,
                    },
                ));
                out.len() - 1
            }
        };
        let event = &mut out[at].1;
        event.paid = event.paid.saturating_add(small(r.n));
        event.came = event.came.saturating_add(small(r.checked_in));
    }
    out.into_iter().map(|(_, event)| event).collect()
}

/// A median duration and how many cases it is over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timing {
    pub median_minutes: u32,
    pub samples: u32,
}

/// Fewer cases than this and a median says more about the cases than the
/// process; the landing shows no timing instead.
pub const MIN_TIMING_SAMPLES: usize = 5;

impl Timing {
    /// Median of durations in seconds, rounded to whole minutes (the mean of
    /// the two middle values for an even count). `None` under the minimum.
    pub fn from_seconds(mut seconds: Vec<u64>) -> Option<Timing> {
        if seconds.len() < MIN_TIMING_SAMPLES {
            return None;
        }
        seconds.sort_unstable();
        let mid = seconds.len() / 2;
        let median_s = match seconds.len() % 2 {
            1 => seconds[mid] as f64,
            _ => (seconds[mid - 1] as f64 + seconds[mid] as f64) / 2.0,
        };
        Some(Timing {
            median_minutes: u32::try_from((median_s / 60.0).round() as u64).unwrap_or(u32::MAX),
            samples: u32::try_from(seconds.len()).unwrap_or(u32::MAX),
        })
    }
}

/// Which landing count a stored `participation_type` belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsTrack {
    OnSite,
    Online,
    /// Retrospective leads and unknown values ("test"): in no track count.
    Neither,
}

/// The track of one stored value, by the app's own parser: a staff walk-in
/// parses as `WalkIn` and is on site, as in `TrackCounts::add`.
pub fn stats_track(stored: &str) -> StatsTrack {
    use super::attendee::ParticipationType;
    match ParticipationType::parse(stored) {
        ParticipationType::InPerson | ParticipationType::WalkIn => StatsTrack::OnSite,
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
    let (mut slip_s, mut refund_s) = (Vec::new(), Vec::new());
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
            // Money handled, but not a payer (see `deposit_payers`).
            ("paid_staff", _) => {
                s.deposits_handled_count = s.deposits_handled_count.saturating_add(small(r.n));
            }
            ("thb", _) => s.deposits_handled_thb = r.n,
            ("slip_s", _) => slip_s.push(r.n),
            ("refund_s", _) => refund_s.push(r.n),
            _ => {}
        }
    }
    s.slip_check = Timing::from_seconds(slip_s);
    s.refund_after_end = Timing::from_seconds(refund_s);
    s
}
