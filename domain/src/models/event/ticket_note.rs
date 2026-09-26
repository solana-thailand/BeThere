//! Organizer-written free-text notes on an event: the day-of ticket
//! announcement (migration 0049) and the postponed notice (migration 0053).
//!
//! Free prose, rendered as text and never as HTML, so there is nothing to
//! sanitise here — the renderer builds DOM nodes from literal strings. What
//! this module does is bound the size and normalise line endings.
//!
//! The bound is not cosmetic. Every note lives in the event JSON cached in KV,
//! which is read on *every* ticket page load, so an organizer who pastes a
//! whole document into the textarea would slow the page down for every
//! attendee of that event, not just themselves.

/// Longest announcement accepted, in characters.
///
/// A day-of announcement is a few paragraphs: which door, where to park, when
/// the stream goes up. 4000 characters is roughly two pages of prose — far
/// more than anyone has needed — while keeping the cached event JSON small.
pub const MAX_TICKET_NOTE_CHARS: usize = 4_000;

/// Longest postponed notice accepted, in characters.
///
/// The notice is a banner, not an announcement: one or two sentences giving
/// the old date, the new date and the reason. It also rides in the public
/// event list, so it is held to a much smaller cap than the ticket note.
pub const MAX_POSTPONED_NOTE_CHARS: usize = 500;

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
    normalize_bounded_note(raw, MAX_TICKET_NOTE_CHARS, "announcement")
}

/// Same normalisation as [`normalize_ticket_note`], with the smaller
/// [`MAX_POSTPONED_NOTE_CHARS`] cap. Blank means "not postponed".
///
/// # Errors
///
/// Returns a message written for the organizer looking at the form field.
pub fn normalize_postponed_note(raw: &str) -> Result<String, String> {
    normalize_bounded_note(raw, MAX_POSTPONED_NOTE_CHARS, "postponed notice")
}

/// Shared body of the note normalisers: fold CRLF/CR to LF, trim, and refuse
/// (never truncate) anything over `max_chars` characters. Characters, not
/// bytes, so Thai text is not refused at a third of the stated length.
fn normalize_bounded_note(raw: &str, max_chars: usize, label: &str) -> Result<String, String> {
    let note = raw.replace("\r\n", "\n").replace('\r', "\n");
    let note = note.trim();
    if note.is_empty() {
        return Ok(String::new());
    }

    let chars = note.chars().count();
    match chars > max_chars {
        true => Err(format!(
            "{label} is {chars} characters; at most {max_chars} allowed"
        )),
        false => Ok(note.to_string()),
    }
}
