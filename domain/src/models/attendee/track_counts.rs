//! Head-count per live track, the one tally every capacity check reads.
//!
//! Plan 028 W3: the counting paths used to fetch every attendee row to count
//! them. They now fold `(participation_type, count)` pairs from a D1
//! `GROUP BY`. Classifying the distinct values here, not in SQL, keeps the
//! count on the same `ParticipationType::parse` as the rest of the app, so no
//! SQL predicate can drift from it.

use super::ParticipationType;

/// Attendees counted against each live track's capacity.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TrackCounts {
    pub in_person: u32,
    pub online: u32,
}

impl TrackCounts {
    /// Add `n` attendees whose stored participation type is `participation_type`.
    ///
    /// A walk-in is physically present, so it takes an in-person spot
    /// (.issues/157, classified at the parser since .issues/162). Retrospective
    /// learners count toward neither track.
    pub fn add(&mut self, participation_type: &str, n: u32) {
        match ParticipationType::parse(participation_type) {
            p if p.is_in_person() => self.in_person = self.in_person.saturating_add(n),
            other if other.counts_toward_online_track() => {
                self.online = self.online.saturating_add(n);
            }
            _ => {}
        }
    }

    /// Tally one attendee per participation type.
    pub fn from_participation_types<'a>(types: impl IntoIterator<Item = &'a str>) -> Self {
        types.into_iter().fold(Self::default(), |mut counts, t| {
            counts.add(t, 1);
            counts
        })
    }
}
