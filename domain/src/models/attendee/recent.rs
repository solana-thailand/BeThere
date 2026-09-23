//! The bounded "recent check-ins" list on the admin roster (plan 028 W9).

use std::cmp::Reverse;
use std::collections::HashMap;

use chrono::DateTime;

use super::core::Attendee;
use crate::models::api::RecentCheckIn;

/// How many check-ins the dashboard shows per tab. It is the `.take(10)` in
/// the frontend's `render_recent_check_ins`.
pub const RECENT_CHECK_INS_PER_TYPE: usize = 10;

/// The newest `per_type` check-ins for each distinct `participation_type`,
/// newest first.
///
/// The roster used to ship every checked-in attendee, on every poll. The
/// dashboard filters this list by tab and then takes 10. A tab is a predicate
/// over `participation_type`, so its 10 newest always sit inside the union of
/// each type's 10 newest. A single overall cap would empty the Online tab
/// whenever the 10 newest check-ins were all in person.
///
/// A `checked_in_at` that is not RFC 3339 cannot be ranked here, while the
/// browser's `Date.parse` might still rank it. Such entries are always kept,
/// after the ranked ones, so the bound can never hide a check-in the dashboard
/// would have shown.
pub fn recent_check_ins(attendees: &[Attendee], per_type: usize) -> Vec<RecentCheckIn> {
    let mut ranked: HashMap<&str, Vec<(i64, &Attendee, &str)>> = HashMap::new();
    let mut unranked: Vec<(&Attendee, &str)> = Vec::new();
    for attendee in attendees {
        let Some(ts) = attendee.checked_in_at.as_deref() else {
            continue;
        };
        match DateTime::parse_from_rfc3339(ts) {
            Ok(at) => ranked
                .entry(attendee.participation_type.as_str())
                .or_default()
                .push((at.timestamp_millis(), attendee, ts)),
            Err(_) => unranked.push((attendee, ts)),
        }
    }

    let mut kept: Vec<(i64, &Attendee, &str)> = ranked
        .into_values()
        .flat_map(|mut group| {
            group.sort_by_key(|entry| Reverse(entry.0));
            group.truncate(per_type);
            group
        })
        .collect();
    kept.sort_by_key(|entry| Reverse(entry.0));

    kept.into_iter()
        .map(|(_, attendee, ts)| (attendee, ts))
        .chain(unranked)
        .map(|(attendee, ts)| RecentCheckIn {
            api_id: attendee.api_id.clone(),
            name: attendee.display_name().to_string(),
            checked_in_at: ts.to_string(),
            checked_in_by: attendee.checked_in_by.clone(),
        })
        .collect()
}
