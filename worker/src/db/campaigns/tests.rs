use super::stats::totals_sql;
use super::series::{EventSeriesEntry, compute_series_neighbors};

/// Guards the fix for the stats 500. Asserting on the SQL string is the
/// only native check available — the NULL behaviour lives in SQLite, not in
/// Rust — but it does catch the realistic regression: someone "simplifying"
/// the COALESCE away and reintroducing a 500 on every new campaign.
#[test]
fn totals_sql_coalesces_the_sum() {
    let sql = totals_sql();
    assert!(
        sql.contains("COALESCE(SUM(CASE WHEN is_complete = 1 THEN 1 ELSE 0 END), 0)"),
        "SUM must stay wrapped in COALESCE — an empty set yields NULL and 500s: {sql}"
    );
    assert!(sql.contains("COUNT(*) AS total_enrolled"));
    assert!(sql.contains("FROM developer_campaign_progress"));
    assert!(sql.contains("WHERE campaign_id = ?"));
}

fn entry(id: &str, seq: i64) -> EventSeriesEntry {
    EventSeriesEntry {
        event_id: id.to_string(),
        name: format!("Event {id}"),
        slug: format!("slug-{id}"),
        event_start_ms: 0,
        sequence_order: seq,
    }
}

// Series with 3 events: A → B → C. Reused across the position tests.
fn three_event_series() -> Vec<EventSeriesEntry> {
    vec![entry("A", 1), entry("B", 2), entry("C", 3)]
}

// --- Edge: empty list ---
#[test]
fn empty_list_returns_orphan_index() {
    let (idx, prev, next) = compute_series_neighbors(&[], "A");
    assert_eq!(idx, -1);
    assert!(prev.is_none());
    assert!(next.is_none());
}

// --- Edge: orphan (linked but missing from joined events list) ---
#[test]
fn orphan_event_returns_negative_index_and_no_neighbors() {
    let events = three_event_series();
    let (idx, prev, next) = compute_series_neighbors(&events, "ZZZ");
    assert_eq!(idx, -1);
    assert!(prev.is_none());
    assert!(next.is_none());
}

// --- Edge: single-event campaign ---
#[test]
fn single_event_has_index_zero_and_no_neighbors() {
    let events = vec![entry("solo", 1)];
    let (idx, prev, next) = compute_series_neighbors(&events, "solo");
    assert_eq!(idx, 0);
    assert!(prev.is_none(), "single event should have no previous");
    assert!(next.is_none(), "single event should have no next");
}

// --- Edge: first event in a multi-event series ---
#[test]
fn first_event_has_no_previous() {
    let events = three_event_series();
    let (idx, prev, next) = compute_series_neighbors(&events, "A");
    assert_eq!(idx, 0);
    assert!(prev.is_none(), "first event should have no previous");
    assert_eq!(next.as_ref().unwrap().event_id, "B");
}

// --- Edge: last event in a multi-event series ---
#[test]
fn last_event_has_no_next() {
    let events = three_event_series();
    let (idx, prev, next) = compute_series_neighbors(&events, "C");
    assert_eq!(idx, 2);
    assert_eq!(prev.as_ref().unwrap().event_id, "B");
    assert!(next.is_none(), "last event should have no next");
}

// --- Middle of the series ---
#[test]
fn middle_event_has_both_neighbors() {
    let events = three_event_series();
    let (idx, prev, next) = compute_series_neighbors(&events, "B");
    assert_eq!(idx, 1);
    assert_eq!(prev.as_ref().unwrap().event_id, "A");
    assert_eq!(next.as_ref().unwrap().event_id, "C");
}

// --- Position is by list index, not sequence_order ---
// A campaign_events row could (theoretically) carry the same sequence_order
// twice; the handler relies on list position as the source of truth.
#[test]
fn neighbors_use_list_index_not_sequence_order() {
    // Deliberately non-sequential order values; list order is A,B,C.
    let events = vec![entry("A", 5), entry("B", 5), entry("C", 1)];
    let (idx, prev, next) = compute_series_neighbors(&events, "B");
    assert_eq!(idx, 1);
    assert_eq!(prev.as_ref().unwrap().event_id, "A");
    assert_eq!(next.as_ref().unwrap().event_id, "C");
}

// --- Neighbors are cloned by value, not by reference ---
#[test]
fn neighbors_carry_full_entry_payload() {
    let events = three_event_series();
    let (_, _, next) = compute_series_neighbors(&events, "B");
    let n = next.unwrap();
    assert_eq!(n.event_id, "C");
    assert_eq!(n.name, "Event C");
    assert_eq!(n.slug, "slug-C");
    assert_eq!(n.sequence_order, 3);
}

// --- First match wins when an id appears twice (defensive) ---
#[test]
fn duplicate_event_id_uses_first_match() {
    let events = vec![entry("dup", 1), entry("other", 2), entry("dup", 3)];
    // First "dup" is at index 0 → no previous, next is "other".
    let (idx, prev, next) = compute_series_neighbors(&events, "dup");
    assert_eq!(idx, 0);
    assert!(prev.is_none());
    assert_eq!(next.as_ref().unwrap().event_id, "other");
}
