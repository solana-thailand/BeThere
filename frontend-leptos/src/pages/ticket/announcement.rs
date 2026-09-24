//! Organizer announcement card on the ticket page (migration 0049).
//!
//! Free text the organizer writes for the day of the event: travel and parking
//! for people coming in person, livestream timing for people watching. The
//! Worker picks which of the two to send based on `is_in_person`, so this
//! component never has to know which audience it is rendering for.
//!
//! **Rendered as text, never as HTML.** The organizer is trusted; the storage
//! is not. This string arrives from D1, survives a duplicate-event copy and
//! could be reached by any future import path, and `inner_html` on a field
//! nobody would think to audit again is how stored XSS gets in. Bare URLs are
//! turned into real anchors instead, which is the only markup anyone asked
//! for. Line breaks are preserved by `white-space: pre-wrap` in CSS rather
//! than by generating `<br>`, so the organizer's paragraphs survive untouched.

use leptos::prelude::*;

use crate::icons::{Icon, IconName};

/// One run of the announcement: literal text, or a URL to linkify.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Segment {
    Text(String),
    Link(String),
}

/// Trailing characters stripped off a detected URL.
///
/// A URL at the end of a sentence is almost always followed by punctuation the
/// author did not mean to include, and a closing bracket usually belongs to an
/// opening one outside the link. Getting this wrong produces a 404 for the
/// attendee, which is worse than a slightly short link.
///
/// A `&str` rather than a `&[char]` on purpose: `tests/css_class_audit.rs`
/// scans every `.rs` file in `src/` concatenated into one string, looking for
/// string literals, and it does not understand char literals. A char literal
/// holding a double-quote character anywhere in `src/` desynchronises that scan
/// for every file sorted after it
/// and reports their live classes as dead CSS. Quote characters are left out
/// of this set entirely; a URL followed by a quotation mark is not a case any
/// announcement has needed.
const TRAILING_PUNCTUATION: &str = ".,;:!?)]}";

/// Split the note into literal text and the URLs inside it.
///
/// Only `http://` and `https://` are recognised. A bare `example.com` is left
/// as text on purpose: guessing a scheme for it would silently send an
/// attendee somewhere the organizer never wrote.
fn segments(note: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut rest = note;
    while let Some(start) = find_scheme(rest) {
        if start > 0 {
            out.push(Segment::Text(rest[..start].to_string()));
        }
        let after = &rest[start..];
        let end = after
            .find(|c: char| c.is_whitespace())
            .unwrap_or(after.len());
        let url = after[..end].trim_end_matches(|c| TRAILING_PUNCTUATION.contains(c));
        // A scheme with nothing after it is not a link, it is the word https.
        match url.len() > scheme_len(url) {
            true => out.push(Segment::Link(url.to_string())),
            false => out.push(Segment::Text(url.to_string())),
        }
        rest = &after[url.len()..];
    }
    if !rest.is_empty() {
        out.push(Segment::Text(rest.to_string()));
    }
    out
}

/// Byte offset of the next `http://` or `https://`, if any.
fn find_scheme(text: &str) -> Option<usize> {
    let http = text.find("http://");
    let https = text.find("https://");
    match (http, https) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Length of the scheme prefix on a string `find_scheme` matched.
fn scheme_len(url: &str) -> usize {
    match url.starts_with("https://") {
        true => "https://".len(),
        false => "http://".len(),
    }
}

/// Render the announcement card, or nothing at all when the organizer has not
/// written one. An empty card is worse than no card: it is a thing the
/// attendee has to read to discover it says nothing.
pub fn announcement_section(note: String) -> impl IntoView {
    let note = note.trim().to_string();
    if note.is_empty() {
        return ().into_any();
    }

    let body: Vec<_> = segments(&note)
        .into_iter()
        .map(|segment| match segment {
            Segment::Text(text) => view! { <span>{text}</span> }.into_any(),
            Segment::Link(url) => {
                let label = url.clone();
                view! {
                    <a
                        href=url
                        target="_blank"
                        rel="noopener noreferrer"
                        class="ticket-announcement-link"
                    >
                        {label}
                    </a>
                }
                .into_any()
            }
        })
        .collect();

    view! {
        <div class="ticket-action-card ticket-action-card--info ticket-announcement-card">
            <div class="ticket-announcement-inner">
                <div class="ticket-announcement-title">
                    <Icon icon=IconName::Info class="icon-sm" />
                    <span>"From the organizer"</span>
                </div>
                <p class="ticket-announcement-body">{body}</p>
            </div>
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Segment {
        Segment::Text(s.to_string())
    }
    fn link(s: &str) -> Segment {
        Segment::Link(s.to_string())
    }

    #[test]
    fn a_note_with_no_url_is_one_run_of_text() {
        assert_eq!(
            segments("Parking is free in the basement. Use door B."),
            vec![text("Parking is free in the basement. Use door B.")]
        );
        assert_eq!(segments(""), vec![]);
    }

    #[test]
    fn the_links_an_organizer_actually_pastes_become_anchors() {
        assert_eq!(
            segments("Slides: https://example.com/deck and the stream is http://yt.be/x"),
            vec![
                text("Slides: "),
                link("https://example.com/deck"),
                text(" and the stream is "),
                link("http://yt.be/x"),
            ]
        );
    }

    /// A URL ending a sentence must not swallow the full stop, and a URL in
    /// brackets must not swallow the bracket — both produce a 404 the attendee
    /// sees and the organizer never does.
    #[test]
    fn sentence_punctuation_stays_out_of_the_href() {
        assert_eq!(
            segments("Join at https://example.com/group."),
            vec![
                text("Join at "),
                link("https://example.com/group"),
                text(".")
            ]
        );
        assert_eq!(
            segments("(https://example.com/a)"),
            vec![text("("), link("https://example.com/a"), text(")")]
        );
    }

    /// Nothing is invented. A hostname without a scheme stays text rather than
    /// being sent somewhere the organizer did not write.
    #[test]
    fn a_bare_hostname_is_not_turned_into_a_link() {
        assert_eq!(
            segments("See example.com for details"),
            vec![text("See example.com for details")]
        );
        assert_eq!(segments("https://"), vec![text("https://")]);
    }

    /// The whole reason this splits text instead of using `inner_html`: markup
    /// in the note has to come out the other side as characters.
    #[test]
    fn markup_in_the_note_stays_literal() {
        let hostile = "<script>alert(1)</script> and <img src=x onerror=alert(1)>";
        assert_eq!(segments(hostile), vec![text(hostile)]);
    }

    /// Thai text is the common case here and must survive byte slicing — a
    /// naive index into a multi-byte string would panic and take the ticket
    /// page down with it.
    #[test]
    fn multibyte_text_around_a_url_does_not_split_a_character() {
        assert_eq!(
            segments("ที่จอดรถ https://example.com/map ชั้นใต้ดิน"),
            vec![
                text("ที่จอดรถ "),
                link("https://example.com/map"),
                text(" ชั้นใต้ดิน"),
            ]
        );
    }

    #[test]
    fn line_breaks_are_left_in_the_text_for_css_to_render() {
        assert_eq!(
            segments("Door B\nSecond floor"),
            vec![text("Door B\nSecond floor")]
        );
    }
}
