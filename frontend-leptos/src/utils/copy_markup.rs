//! The landing copy markup (ASKS-4 §15): `{word|#id}` is a keyword link to a
//! section, `{word|/page#id}` one to a section of another site page (never
//! off the site: `//host` is refused), `**word**` a highlight, and a newline a deliberate line break
//! (rendered by `white-space: pre-line`, so it stays in the text).
//!
//! The strings come from our own catalog, not from users; a marker that does
//! not close is kept as plain text rather than guessed at.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment<'a> {
    Text(&'a str),
    /// `{text|#anchor}` or `{text|/page#anchor}`; `href` is kept as written.
    Link {
        text: &'a str,
        href: &'a str,
    },
    Highlight(&'a str),
}

pub fn parse(src: &str) -> Vec<Segment<'_>> {
    let mut out = Vec::new();
    let mut rest = src;
    while !rest.is_empty() {
        let next = [rest.find('{'), rest.find("**")]
            .into_iter()
            .flatten()
            .min();
        let Some(at) = next else {
            out.push(Segment::Text(rest));
            break;
        };
        let (marked, consumed) = match rest[at..].starts_with("**") {
            true => highlight(&rest[at..]),
            false => link(&rest[at..]),
        };
        match marked {
            Some(seg) => {
                if at > 0 {
                    out.push(Segment::Text(&rest[..at]));
                }
                out.push(seg);
                rest = &rest[at + consumed..];
            }
            // Not a marker: keep everything up to and including it as text.
            None => {
                let keep = at + consumed;
                out.push(Segment::Text(&rest[..keep]));
                rest = &rest[keep..];
            }
        }
    }
    out
}

/// `**word**` → highlight. On no close, consumes the `**` as text.
fn highlight(s: &str) -> (Option<Segment<'_>>, usize) {
    let body = &s[2..];
    match body.find("**") {
        Some(end) if end > 0 => (Some(Segment::Highlight(&body[..end])), end + 4),
        _ => (None, 2),
    }
}

/// `{text|#id}` or `{text|/path}` → link. On anything else, consumes the
/// `{` as text.
fn link(s: &str) -> (Option<Segment<'_>>, usize) {
    let Some(close) = s.find('}') else {
        return (None, 1);
    };
    let inner = &s[1..close];
    match inner.split_once('|') {
        Some((text, href))
            if !text.is_empty() && href.len() > 1 && is_site_href(href) && !inner.contains('{') =>
        {
            (Some(Segment::Link { text, href }), close + 1)
        }
        _ => (None, 1),
    }
}

/// A same-page anchor, or a path on this site; never a scheme or `//host`.
fn is_site_href(href: &str) -> bool {
    match href.as_bytes() {
        [b'#', ..] => true,
        [b'/', b'/', ..] => false,
        [b'/', ..] => !href.contains(':'),
        _ => false,
    }
}
