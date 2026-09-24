//! Organizer's day-of announcement for the ticket page (migration 0049).
//!
//! Free prose, rendered as text and never as HTML, so there is nothing to
//! sanitise here — the renderer builds DOM nodes from literal strings. What
//! this module does is bound the size and normalise line endings.
//!
//! The bound is not cosmetic. Both notes live in the event JSON cached in KV,
//! which is read on *every* ticket page load, so an organizer who pastes a
//! whole document into the textarea would slow the page down for every
//! attendee of that event, not just themselves.

/// Longest announcement accepted, in characters.
///
/// A day-of announcement is a few paragraphs: which door, where to park, when
/// the stream goes up. 4000 characters is roughly two pages of prose — far
/// more than anyone has needed — while keeping the cached event JSON small.
pub const MAX_TICKET_NOTE_CHARS: usize = 4_000;

/// Trim `raw`, normalise its line endings and check its length.
/// Blank means "no announcement", which renders no card at all.
///
/// `\r\n` and bare `\r` both become `\n`: the card is rendered with CSS
/// `white-space: pre-wrap`, so a stray `\r` from a browser textarea would show
/// up as an extra blank line the organizer never typed.
///
/// # Errors
///
/// Returns a message written for the organizer looking at the form field.
pub fn normalize_ticket_note(raw: &str) -> Result<String, String> {
    let note = raw.replace("\r\n", "\n").replace('\r', "\n");
    let note = note.trim();
    if note.is_empty() {
        return Ok(String::new());
    }

    let chars = note.chars().count();
    match chars > MAX_TICKET_NOTE_CHARS {
        true => Err(format!(
            "announcement is {chars} characters; at most {MAX_TICKET_NOTE_CHARS} allowed"
        )),
        false => Ok(note.to_string()),
    }
}
