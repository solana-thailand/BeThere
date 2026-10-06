//! The landing reads `GET /api/public/stats` by these field names.

use event_checkin_domain::models::public_stats::PublicStats;

#[test]
fn field_names_are_the_landing_contract() {
    let json = serde_json::to_value(PublicStats::default()).expect("serializes");
    let mut keys: Vec<&str> = json
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "deposit_payers",
            "deposit_payers_came",
            "deposits_handled_count",
            "deposits_handled_thb",
            "door_scans",
            "events_held",
            "measured_at",
            "online_registrations",
            "onsite_registrations",
        ]
    );
}

use event_checkin_domain::models::public_stats::{StatsRow, StatsTrack, fold_stats, stats_track};

fn row(kind: &str, pt: &str, n: u64, checked_in: u64) -> StatsRow {
    StatsRow {
        kind: kind.into(),
        participation_type: pt.into(),
        n,
        checked_in,
    }
}

#[test]
fn every_spelling_of_a_track_lands_in_it() {
    // An empty stored value is in person, the app's own rule (`parse`).
    for pt in ["in_person", "in-person", "In-Person", "walkin", ""] {
        assert_eq!(stats_track(pt), StatsTrack::OnSite, "{pt}");
    }
    for pt in ["online", "Online"] {
        assert_eq!(stats_track(pt), StatsTrack::Online, "{pt}");
    }
    // A deposit with no attendee row comes back as "(no attendee)".
    for pt in ["test", "retrospective", "(no attendee)"] {
        assert_eq!(stats_track(pt), StatsTrack::Neither, "{pt}");
    }
}

#[test]
fn rows_fold_into_the_landing_numbers() {
    let s = fold_stats(
        &[
            row("held", "", 14, 0),
            row("reg", "in_person", 140, 110),
            row("reg", "in-person", 1, 1),
            row("reg", "online", 385, 3),
            row("reg", "test", 1, 0),
            row("paid", "in_person", 54, 50),
            row("paid", "online", 14, 0),
            row("thb", "", 34_000, 0),
        ],
        "2026-10-06T00:00:00Z".into(),
    );
    assert_eq!(s.events_held, 14);
    assert_eq!((s.onsite_registrations, s.door_scans), (141, 111));
    assert_eq!(s.online_registrations, 385, "test rows count nowhere");
    assert_eq!(
        (s.deposits_handled_count, s.deposits_handled_thb),
        (68, 34_000)
    );
    assert_eq!((s.deposit_payers, s.deposit_payers_came), (54, 50));
}
