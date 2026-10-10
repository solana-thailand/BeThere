//! Per-event payers (`fold_event_payers`, .plans/045 R4.7) and the facts
//! table that moved to `domain`.

use event_checkin_domain::models::facts::{LADDER, LadderSource, ladder_total, percent};
use event_checkin_domain::models::public_stats::{EventPayerRow, fold_event_payers};

fn row(event: &str, pt: &str, n: u64, came: u64) -> EventPayerRow {
    EventPayerRow {
        event_id: event.to_string(),
        name: format!("Event {event}"),
        slug: format!("event-{event}"),
        start_ms: match event {
            "a" => 1,
            _ => 2,
        },
        participation_type: pt.to_string(),
        n,
        checked_in: came,
    }
}

#[test]
fn on_site_types_are_payers_and_others_are_not() {
    let payers = fold_event_payers(&[
        row("a", "In-Person", 10, 9),
        row("a", "walkin", 2, 2),
        row("a", "Online", 3, 0),
        row("a", "(no attendee)", 1, 0),
        row("b", "in_person", 4, 3),
    ]);
    assert_eq!(payers.len(), 2);
    assert_eq!(
        (payers[0].slug.as_str(), payers[0].paid, payers[0].came),
        ("event-a", 12, 11)
    );
    assert_eq!(
        (payers[1].slug.as_str(), payers[1].paid, payers[1].came),
        ("event-b", 4, 3)
    );
}

#[test]
fn an_event_with_only_online_payers_is_left_out() {
    assert!(fold_event_payers(&[row("a", "Online", 5, 0)]).is_empty());
}

#[test]
fn order_follows_the_statement() {
    let payers = fold_event_payers(&[row("b", "In-Person", 1, 1), row("a", "In-Person", 1, 1)]);
    let slugs: Vec<_> = payers.iter().map(|p| p.slug.as_str()).collect();
    assert_eq!(
        slugs,
        ["event-b", "event-a"],
        "the SQL orders by start; the fold keeps it"
    );
}

#[test]
fn facts_keep_their_totals_after_the_move() {
    assert_eq!(ladder_total(), (94, 90));
    assert_eq!(percent(90, 94), 96);
    let system: Vec<_> = LADDER
        .iter()
        .filter(|r| r.source == LadderSource::System)
        .map(|r| (r.event, r.paid, r.came))
        .collect();
    assert_eq!(
        system,
        [("RTM #4", 16, 14), ("RTM #5", 14, 13), ("RTM #6", 21, 20)]
    );
}
