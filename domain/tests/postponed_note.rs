//! The postponed notice (migration 0053): bounds, normalisation, and the serde
//! contract that lets it ride in KV JSON written before the field existed.

use event_checkin_domain::models::event::{
    EventConfig, EventMeta, MAX_POSTPONED_NOTE_CHARS, MAX_TICKET_NOTE_CHARS, UpdateEventRequest,
    normalize_postponed_note,
};

const RTM6_NOTE: &str = "เลื่อนจาก 27 ก.ย. เนื่องจากน้ำท่วม วันใหม่ 4 ต.ค.";

fn sample_config() -> EventConfig {
    EventConfig::from_global_config(
        "RTM#6",
        "",
        "",
        1_790_000_000_000,
        1_790_003_600_000,
        "sheet",
        "Attendees",
        "staff",
        "",
        "",
        "",
        "",
        vec![],
        vec![],
        "",
        "",
    )
}

// ── normalisation ───────────────────────────────────────────────────────────

#[test]
fn blank_means_not_postponed() {
    assert_eq!(normalize_postponed_note(""), Ok(String::new()));
    assert_eq!(normalize_postponed_note("  \r\n\t "), Ok(String::new()));
}

#[test]
fn thai_notice_survives_and_crlf_folds() {
    assert_eq!(
        normalize_postponed_note(RTM6_NOTE),
        Ok(RTM6_NOTE.to_string())
    );
    assert_eq!(
        normalize_postponed_note("  Moved\r\nNew date 4 Oct  "),
        Ok("Moved\nNew date 4 Oct".to_string())
    );
}

/// A banner is a sentence or two, so it gets a far smaller cap than the
/// ticket announcement — and the cap counts characters, not bytes.
#[test]
fn cap_is_smaller_than_the_ticket_note_and_counts_characters() {
    const { assert!(MAX_POSTPONED_NOTE_CHARS < MAX_TICKET_NOTE_CHARS) };

    let at_limit = "ก".repeat(MAX_POSTPONED_NOTE_CHARS);
    assert_eq!(normalize_postponed_note(&at_limit), Ok(at_limit.clone()));

    let err = normalize_postponed_note(&"ก".repeat(MAX_POSTPONED_NOTE_CHARS + 1)).unwrap_err();
    assert!(err.contains("postponed notice"), "{err}");
    assert!(err.contains(&MAX_POSTPONED_NOTE_CHARS.to_string()), "{err}");
    // Error text reaches the organizer through the URL redactor.
    assert!(!err.contains("://"), "{err}");
}

#[test]
fn markup_is_stored_verbatim() {
    let hostile = "<img src=x onerror=alert(1)>";
    assert_eq!(normalize_postponed_note(hostile), Ok(hostile.to_string()));
}

// ── serde contract ──────────────────────────────────────────────────────────

/// Every event already in KV was written without the field; it must still load,
/// as "not postponed".
#[test]
fn kv_json_without_the_field_deserializes_as_not_postponed() {
    let mut json = serde_json::to_value(sample_config()).unwrap();
    json.as_object_mut().unwrap().remove("postponed_note");
    let config: EventConfig = serde_json::from_value(json).unwrap();
    assert!(config.postponed_note.is_empty());
}

/// Empty is not serialized, so events that were never postponed keep the same
/// cached JSON they have today.
#[test]
fn empty_note_is_not_serialized() {
    let config = sample_config();
    assert!(config.postponed_note.is_empty());
    let json = serde_json::to_value(&config).unwrap();
    assert!(json.get("postponed_note").is_none(), "{json}");

    let meta = serde_json::to_value(config.to_meta()).unwrap();
    assert!(meta.get("postponed_note").is_none(), "{meta}");
}

#[test]
fn a_set_note_round_trips_through_config_and_index_meta() {
    let mut config = sample_config();
    config.postponed_note = RTM6_NOTE.to_string();

    let back: EventConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
    assert_eq!(back.postponed_note, RTM6_NOTE);

    // The KV fallback of the public listing badges from the index entry.
    let meta: EventMeta =
        serde_json::from_str(&serde_json::to_string(&config.to_meta()).unwrap()).unwrap();
    assert_eq!(meta.postponed_note, RTM6_NOTE);
}

/// `None` leaves the notice alone; `Some("")` clears it (un-postpones).
#[test]
fn update_request_distinguishes_absent_from_cleared() {
    let absent: UpdateEventRequest = serde_json::from_str("{}").unwrap();
    assert_eq!(absent.postponed_note, None);

    let cleared: UpdateEventRequest = serde_json::from_str(r#"{"postponed_note":""}"#).unwrap();
    assert_eq!(cleared.postponed_note, Some(String::new()));
}
