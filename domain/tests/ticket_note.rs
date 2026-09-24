//! Bounds and line-ending normalisation for the ticket-page announcement
//! (migration 0049, `.issues/132`).

use event_checkin_domain::models::event::{MAX_TICKET_NOTE_CHARS, normalize_ticket_note};

#[test]
fn blank_means_no_announcement() {
    assert_eq!(normalize_ticket_note(""), Ok(String::new()));
    assert_eq!(normalize_ticket_note("   \n\t  "), Ok(String::new()));
    // A textarea the organizer emptied but left a newline in must clear the
    // card, not render an empty one.
    assert_eq!(normalize_ticket_note("\r\n"), Ok(String::new()));
}

#[test]
fn ordinary_prose_survives_untouched() {
    let note = "Door B is the only one unlocked.\n\nParking: basement, free after 6pm.";
    assert_eq!(normalize_ticket_note(note), Ok(note.to_string()));
}

/// The card renders with `white-space: pre-wrap`, so a `\r` a browser textarea
/// left behind would show up as a blank line the organizer never typed.
#[test]
fn carriage_returns_are_folded_into_newlines() {
    assert_eq!(
        normalize_ticket_note("Door B\r\nSecond floor"),
        Ok("Door B\nSecond floor".to_string())
    );
    assert_eq!(
        normalize_ticket_note("Door B\rSecond floor"),
        Ok("Door B\nSecond floor".to_string())
    );
}

/// Both notes ride in the event JSON that KV serves on every ticket page load.
/// The cap is what stops one organizer's pasted document from slowing the page
/// for every attendee of that event.
#[test]
fn an_oversized_note_is_refused_not_truncated() {
    let at_limit = "a".repeat(MAX_TICKET_NOTE_CHARS);
    assert_eq!(normalize_ticket_note(&at_limit), Ok(at_limit.clone()));

    let over = "a".repeat(MAX_TICKET_NOTE_CHARS + 1);
    let err = normalize_ticket_note(&over).unwrap_err();
    assert!(
        err.contains(&(MAX_TICKET_NOTE_CHARS + 1).to_string()),
        "{err}"
    );
    assert!(err.contains(&MAX_TICKET_NOTE_CHARS.to_string()), "{err}");
}

/// The limit counts characters, not bytes — a Thai announcement is ~3 bytes
/// per character and must not be refused at a third of the stated length.
#[test]
fn the_limit_counts_characters_not_bytes() {
    let thai = "ก".repeat(MAX_TICKET_NOTE_CHARS);
    assert!(
        thai.len() > MAX_TICKET_NOTE_CHARS,
        "fixture must be multibyte"
    );
    assert_eq!(normalize_ticket_note(&thai), Ok(thai.clone()));
    assert!(normalize_ticket_note(&"ก".repeat(MAX_TICKET_NOTE_CHARS + 1)).is_err());
}

/// Markup is stored verbatim; the renderer builds text nodes, never HTML.
#[test]
fn markup_is_stored_verbatim() {
    let hostile = "<script>alert(1)</script>";
    assert_eq!(normalize_ticket_note(hostile), Ok(hostile.to_string()));
}
