//! The per-event share card (`.issues/183` option B): what goes where on the
//! 1200×630 canvas the event editor draws on save.
//!
//! Only the pure part lives here (geometry, text fitting, which art to use),
//! so `tests/og_card_layout.rs` can pin it natively. The canvas drawing and
//! the upload are in `pages/event_form/og_card.rs`, compiled into the staff
//! shell only.
//!
//! The look follows the stock card (`share/og-card.html`): paper ground, ink
//! text, the field-violet bar, Anuphan for Thai. The art is the organizer's
//! uploaded poster when there is one, else the event's generative riso
//! poster (`utils/poster.rs`).

pub use event_checkin_domain::og_card::{OG_HEIGHT, OG_WIDTH};

/// Colours of the stock card (`share/og-card.html` `:root`).
pub const PAPER: &str = "#f4f0e6";
pub const INK: &str = "#121212";
pub const INK_2: &str = "#4a4740";
pub const FIELD: &str = "#6b63f0";

/// Fonts. Thai falls through to Anuphan, which the page already loads.
pub const LOGO_FONT: &str = "800 40px Inter, Anuphan, sans-serif";
pub const TITLE_FONT: &str = "700 64px Anuphan, Inter, sans-serif";
pub const LINE_FONT: &str = "500 28px Anuphan, Inter, sans-serif";

/// A box on the card, in canvas pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Left margin of the text column.
pub const TEXT_X: f64 = 80.0;
/// Widest a text line may be: up to the art, with a 56 px gutter.
pub const TEXT_MAX_W: f64 = ART.x - TEXT_X - 56.0;
/// The art: 4:5 like the posters, right-hand side.
pub const ART: Rect = Rect {
    x: 800.0,
    y: 80.0,
    w: 320.0,
    h: 400.0,
};
/// Baselines.
pub const LOGO_Y: f64 = 118.0;
pub const TITLE_Y: f64 = 222.0;
pub const TITLE_LINE_H: f64 = 76.0;
pub const TITLE_MAX_LINES: usize = 3;
pub const WHEN_Y: f64 = 478.0;
pub const PLACE_Y: f64 = 520.0;
/// The violet bar under the text.
pub const BAR: Rect = Rect {
    x: TEXT_X,
    y: 560.0,
    w: 180.0,
    h: 10.0,
};

/// What the editor knows about the event when it saves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardEvent {
    pub name: String,
    pub slug: String,
    pub start_ms: i64,
    pub time_tba: bool,
    pub location: String,
    pub poster_url: String,
}

/// The text on the card, already fitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardText {
    pub title: Vec<String>,
    pub when: String,
    pub place: Option<String>,
}

/// Fit the event's text. `fits_title` / `fits_line` say whether a string fits
/// [`TEXT_MAX_W`] in [`TITLE_FONT`] / [`LINE_FONT`] (the canvas measures; the
/// tests count characters).
pub fn card_text(
    event: &CardEvent,
    fits_title: &dyn Fn(&str) -> bool,
    fits_line: &dyn Fn(&str) -> bool,
) -> CardText {
    let name = match event.name.trim() {
        "" => "BeThere event",
        name => name,
    };
    let when = event_checkin_domain::og_card::event_when(event.start_ms, event.time_tba);
    let place = wrap_text(&event.location, 1, fits_line).into_iter().next();
    CardText {
        title: wrap_text(name, TITLE_MAX_LINES, fits_title),
        when: wrap_text(&when, 1, fits_line)
            .into_iter()
            .next()
            .unwrap_or_default(),
        place,
    }
}

/// Where the art comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtSource {
    /// The organizer's poster in our R2 (same origin, so the canvas stays
    /// exportable).
    Uploaded(String),
    /// The generative poster for the slug. Also the fallback when the
    /// uploaded one fails to load.
    Generative,
}

/// An external poster would taint the canvas (no CORS), so only our own is
/// drawn.
pub fn art_source(poster_url: &str) -> ArtSource {
    let url = poster_url.trim();
    match url.starts_with("/api/storage/posters/") && !url.contains(['"', '<', '>', ' ']) {
        true => ArtSource::Uploaded(url.to_string()),
        false => ArtSource::Generative,
    }
}

/// The source rectangle `(sx, sy, sw, sh)` that fills a `box_w`×`box_h` box
/// without distortion, cropping the overflow equally on both sides.
pub fn cover_crop(src_w: f64, src_h: f64, box_w: f64, box_h: f64) -> (f64, f64, f64, f64) {
    if src_w <= 0.0 || src_h <= 0.0 || box_w <= 0.0 || box_h <= 0.0 {
        return (0.0, 0.0, src_w.max(0.0), src_h.max(0.0));
    }
    let scale = (box_w / src_w).max(box_h / src_h);
    let (crop_w, crop_h) = (box_w / scale, box_h / scale);
    (
        (src_w - crop_w) / 2.0,
        (src_h - crop_h) / 2.0,
        crop_w,
        crop_h,
    )
}

/// Whether `c` draws on the character before it (Thai vowels and tone marks
/// above/below, Latin combining accents, variation selectors, ZWJ).
fn is_combining(c: char) -> bool {
    matches!(c,
        '\u{0E31}' | '\u{0E34}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}'
        | '\u{0300}'..='\u{036F}' | '\u{FE00}'..='\u{FE0F}' | '\u{200D}')
}

/// Split `text` into units a line may break between: a base character with
/// the marks that draw on it. Breaking inside one would leave a Thai tone
/// mark floating at the start of the next line.
pub fn clusters(text: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut joined = false;
    for (at, c) in text.char_indices() {
        if at > 0 && !is_combining(c) && !joined {
            out.push(&text[start..at]);
            start = at;
        }
        joined = c == '\u{200D}';
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Greedy wrap into at most `max_lines` lines that each `fit`. Breaks at
/// spaces; a word too wide for a line (Thai has no spaces between words)
/// breaks between [`clusters`]. Text left over ends the last line with `…`.
pub fn wrap_text(text: &str, max_lines: usize, fits: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    if max_lines == 0 {
        return lines;
    }
    let mut line = String::new();
    let mut overflow = false;
    'words: for word in text.split_whitespace() {
        let candidate = match line.is_empty() {
            true => word.to_string(),
            false => format!("{line} {word}"),
        };
        if fits(&candidate) {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            if lines.len() + 1 == max_lines {
                overflow = true;
                break;
            }
            lines.push(std::mem::take(&mut line));
            if fits(word) {
                line = word.to_string();
                continue;
            }
        }
        // The word alone is too wide: break it between clusters.
        for unit in clusters(word) {
            let candidate = format!("{line}{unit}");
            if line.is_empty() || fits(&candidate) {
                line = candidate;
                continue;
            }
            if lines.len() + 1 == max_lines {
                overflow = true;
                break 'words;
            }
            lines.push(std::mem::replace(&mut line, unit.to_string()));
        }
    }
    match overflow {
        true => lines.push(ellipsize(&line, fits)),
        false if !line.is_empty() => lines.push(line),
        false => {}
    }
    lines
}

/// `line` shortened until `line…` fits.
fn ellipsize(line: &str, fits: &dyn Fn(&str) -> bool) -> String {
    let mut units = clusters(line);
    loop {
        let shown = format!("{}…", units.concat().trim_end());
        if units.is_empty() || fits(&shown) {
            return shown;
        }
        units.pop();
    }
}
