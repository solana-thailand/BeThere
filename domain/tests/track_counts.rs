//! `TrackCounts` (plan 028 W3, .issues/157): the grouped tally must equal the
//! per-attendee loop it replaced, except that a walk-in now takes an in-person
//! spot instead of an online one.

use event_checkin_domain::models::attendee::{
    PARTICIPATION_WALK_IN, ParticipationType, TrackCounts,
};

/// Stored values seen in prod plus the edge cases `parse` handles.
const CORPUS: &[&str] = &[
    "",
    "   ",
    "In-Person",
    "in_person",
    "In-Person (Physical Attendance)",
    "physical",
    "Online",
    "online",
    "Virtual",
    "Online / In person",
    "retrospective",
    "Retrospective",
    "test",
    "Walkin",
    "something else",
];

/// The loop every counting site ran before W3.
fn per_attendee(types: &[&str]) -> TrackCounts {
    let mut counts = TrackCounts::default();
    for t in types {
        let parsed = ParticipationType::parse(t);
        if parsed == ParticipationType::InPerson {
            counts.in_person += 1;
        } else if parsed.counts_toward_online_track() {
            counts.online += 1;
        }
    }
    counts
}

#[test]
fn matches_the_per_attendee_loop_for_every_non_walk_in_value() {
    for t in CORPUS {
        assert_eq!(
            TrackCounts::from_participation_types([*t]),
            per_attendee(&[t]),
            "{t:?}"
        );
    }
}

#[test]
fn a_walk_in_takes_an_in_person_spot_not_an_online_one() {
    let counts = TrackCounts::from_participation_types([PARTICIPATION_WALK_IN]);
    assert_eq!(
        counts,
        TrackCounts {
            in_person: 1,
            online: 0
        }
    );
    // The old loop put it in the online bucket; that is the bug being fixed.
    assert_eq!(
        per_attendee(&[PARTICIPATION_WALK_IN]),
        TrackCounts {
            in_person: 0,
            online: 1
        }
    );
}

#[test]
fn only_the_exact_stored_sentinel_is_a_walk_in() {
    // `count_walkin_attendees` matches `participation_type = 'walkin'` exactly.
    for t in ["Walkin", "WALKIN", " walkin"] {
        assert_eq!(
            TrackCounts::from_participation_types([t]),
            per_attendee(&[t]),
            "{t:?}"
        );
    }
}

#[test]
fn grouped_rows_equal_one_add_per_attendee() {
    let mut grouped = TrackCounts::default();
    grouped.add("In-Person", 3);
    grouped.add("Online", 2);
    grouped.add("retrospective", 4);
    grouped.add(PARTICIPATION_WALK_IN, 5);
    grouped.add("other", 1);
    let rows = ["In-Person"; 3]
        .into_iter()
        .chain(["Online"; 2])
        .chain(["retrospective"; 4])
        .chain([PARTICIPATION_WALK_IN; 5])
        .chain(["other"]);
    assert_eq!(grouped, TrackCounts::from_participation_types(rows));
    assert_eq!(
        grouped,
        TrackCounts {
            in_person: 8,
            online: 3
        }
    );
}

#[test]
fn counts_saturate_instead_of_wrapping() {
    let mut counts = TrackCounts {
        in_person: u32::MAX,
        online: u32::MAX,
    };
    counts.add("", 1);
    counts.add("online", 1);
    assert_eq!(
        counts,
        TrackCounts {
            in_person: u32::MAX,
            online: u32::MAX
        }
    );
}
