//! Per-event social tags for `/e/{slug}` (`.issues/183` Part 1).
//!
//! Crawlers do not run the SPA, so the head they read is the one the Worker
//! sends. For an event page the Worker takes the stock `index.html` and
//! replaces the `og:*` / `twitter:*` tags with the event's own values. Pure
//! string work on a ~10 KB head: no HTML parser, no `lol-html`.
//!
//! The tags are located by content (`property="og:title"`), not by a marker
//! comment, so `index.html` needs no special block. If any target tag is
//! missing or appears twice, [`splice`] returns `None` and the stock page is
//! served; `tests/og_splice.rs` reads the real `frontend-leptos/index.html`
//! and fails when a target moves.
//!
//! Image order: a raster poster (PNG/JPEG; SVG and WebP skipped, the
//! platforms do not all render them), then the browser-made card at
//! `og/{event_id}.png`, then the stock `/og-image.png` (tags left as they are).

use event_checkin_domain::models::event::{EventConfig, EventStatus, EventVisibility};
use event_checkin_domain::og_card::{OG_HEIGHT, OG_WIDTH, event_summary};

/// Prefix of a poster stored in R2 (`handlers/events/poster.rs`).
pub const STORED_POSTER_PREFIX: &str = "/api/storage/posters/";

/// Served path of the browser-made card (`storage::serve_og_card`).
pub const OG_CARD_PREFIX: &str = "/api/storage/og/";

/// Longest `og:description` we send; platforms cut near 200 anyway.
pub const MAX_DESCRIPTION_CHARS: usize = 200;

/// Which attribute names a tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagAttr {
    Property,
    Name,
}

impl TagAttr {
    fn as_str(self) -> &'static str {
        match self {
            Self::Property => "property",
            Self::Name => "name",
        }
    }
}

/// What a target tag carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagField {
    Title,
    Description,
    Url,
    Image,
    ImageType,
    ImageWidth,
    ImageHeight,
    ImageAlt,
}

/// Every tag the splice rewrites. `index.html` must carry each exactly once.
pub const SPLICE_TARGETS: [(TagAttr, &str, TagField); 12] = [
    (TagAttr::Property, "og:title", TagField::Title),
    (TagAttr::Property, "og:description", TagField::Description),
    (TagAttr::Property, "og:url", TagField::Url),
    (TagAttr::Property, "og:image", TagField::Image),
    (TagAttr::Property, "og:image:type", TagField::ImageType),
    (TagAttr::Property, "og:image:width", TagField::ImageWidth),
    (TagAttr::Property, "og:image:height", TagField::ImageHeight),
    (TagAttr::Property, "og:image:alt", TagField::ImageAlt),
    (TagAttr::Name, "twitter:title", TagField::Title),
    (TagAttr::Name, "twitter:description", TagField::Description),
    (TagAttr::Name, "twitter:image", TagField::Image),
    (TagAttr::Name, "twitter:image:alt", TagField::ImageAlt),
];

/// Image format the platforms render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterKind {
    Png,
    Jpeg,
}

impl RasterKind {
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

/// What `poster_url` points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PosterSource {
    /// No poster.
    None,
    /// Uploaded to R2 under this event id (may differ from the event's own
    /// id after a duplicate). Its format is only known from R2.
    Stored { event_id: String },
    /// An https URL whose path ends in a raster extension.
    External { url: String, kind: RasterKind },
    /// Anything a crawler cannot use: SVG, WebP, http, relative, unknown.
    Unusable,
}

/// Classify a poster URL without I/O.
pub fn classify_poster(poster_url: &str) -> PosterSource {
    let url = poster_url.trim();
    if url.is_empty() {
        return PosterSource::None;
    }
    if let Some(id) = url.strip_prefix(STORED_POSTER_PREFIX) {
        return match is_safe_id(id) {
            true => PosterSource::Stored {
                event_id: id.to_string(),
            },
            false => PosterSource::Unusable,
        };
    }
    if !url.starts_with("https://") || url.contains(['"', '<', '>', ' ']) {
        return PosterSource::Unusable;
    }
    let path = url.split(['?', '#']).next().unwrap_or_default();
    match raster_kind_from_path(path) {
        Some(kind) => PosterSource::External {
            url: url.to_string(),
            kind,
        },
        None => PosterSource::Unusable,
    }
}

/// `png` / `jpg` / `jpeg` at the end of a path, any case.
pub fn raster_kind_from_path(path: &str) -> Option<RasterKind> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some(RasterKind::Png),
        "jpg" | "jpeg" => Some(RasterKind::Jpeg),
        _ => None,
    }
}

/// An id that is safe inside an R2 key and a URL path.
pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The image chosen for the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageChoice {
    /// A raster poster at this absolute URL; its size is unknown.
    Poster { url: String, kind: RasterKind },
    /// The browser-made 1200×630 card.
    Card,
    /// Keep the stock image tags.
    Stock,
}

/// Apply the image order to what R2 said.
///
/// `poster` is the usable raster poster if there is one (an external raster
/// URL, or a stored poster R2 holds as PNG/JPEG); `card_present` says whether
/// `og/{event_id}.png` exists.
pub fn choose_image(poster: Option<(String, RasterKind)>, card_present: bool) -> ImageChoice {
    match (poster, card_present) {
        (Some((url, kind)), _) => ImageChoice::Poster { url, kind },
        (None, true) => ImageChoice::Card,
        (None, false) => ImageChoice::Stock,
    }
}

/// The image tags' values. `size: None` drops the width/height tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OgImage {
    pub url: String,
    pub mime: &'static str,
    pub size: Option<(u32, u32)>,
    pub alt: String,
}

/// Everything the splice writes. `image: None` keeps the stock image tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OgMeta {
    pub title: String,
    pub description: String,
    pub url: String,
    pub image: Option<OgImage>,
}

/// Whether `/e/{slug}` may show this event to a crawler: the same rule as
/// `GET /api/public/event/{slug}` without a session (no drafts, no archived,
/// no private events).
pub fn is_shareable(event: &EventConfig) -> bool {
    !matches!(event.status, EventStatus::Draft | EventStatus::Archived)
        && event.visibility != EventVisibility::Private
}

/// The slug of an event page path (`/e/{slug}`, trailing slash allowed),
/// percent-decoded.
pub fn page_slug(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/e/")?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    let slug = urlencoding::decode(rest).ok()?.into_owned();
    match slug.trim().is_empty() {
        true => None,
        false => Some(slug),
    }
}

/// Build the tags for `event` served from `origin` (no trailing slash).
pub fn meta_for_event(event: &EventConfig, origin: &str, image: ImageChoice) -> OgMeta {
    let name = one_line(&event.name);
    let summary = event_summary(event.event_start_ms, event.time_tba, &event.location);
    let blurb = match one_line(&event.tagline) {
        t if !t.is_empty() => t,
        _ => one_line(&event.description),
    };
    let description = match blurb.is_empty() {
        true => summary.clone(),
        false => format!("{summary}. {blurb}"),
    };
    let image = match image {
        ImageChoice::Stock => None,
        ImageChoice::Poster { url, kind } => Some(OgImage {
            url,
            mime: kind.mime(),
            size: None,
            alt: format!("Poster for {name}"),
        }),
        ImageChoice::Card => Some(OgImage {
            url: format!("{origin}{OG_CARD_PREFIX}{}", event.id),
            mime: RasterKind::Png.mime(),
            size: Some((OG_WIDTH, OG_HEIGHT)),
            alt: format!("{name}: {summary}"),
        }),
    };
    OgMeta {
        title: format!("{name} · BeThere"),
        description: truncate_chars(&description, MAX_DESCRIPTION_CHARS),
        url: format!("{origin}/e/{}", urlencoding::encode(&event.slug)),
        image,
    }
}

/// Collapse whitespace (newlines included) to single spaces.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// At most `max` chars, ending in `…` when cut.
pub fn truncate_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max.saturating_sub(1)) {
        Some((cut, _)) if text.chars().count() > max => {
            format!("{}…", text[..cut].trim_end())
        }
        _ => text.to_string(),
    }
}

/// Escape a value for a double-quoted HTML attribute.
pub fn html_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 16);
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Where one target tag sits: the whole `<meta …>` element and its
/// `content` value, as byte ranges into the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSpan {
    pub element: std::ops::Range<usize>,
    pub content: std::ops::Range<usize>,
}

/// Find the single `<meta {attr}="{key}" … content="…">` element.
/// `None` if it is missing, ambiguous or malformed.
pub fn find_tag(html: &str, attr: TagAttr, key: &str) -> Option<TagSpan> {
    let needle = format!("{}=\"{key}\"", attr.as_str());
    let mut hits = html.match_indices(&needle);
    let (at, _) = hits.next()?;
    if hits.next().is_some() {
        return None;
    }
    let start = html[..at].rfind("<meta")?;
    // The attribute must sit inside that same element.
    if html[start..at].contains('>') {
        return None;
    }
    let end = at + html[at..].find('>')? + 1;
    let element = &html[start..end];
    let value_at = element.find("content=\"")? + "content=\"".len();
    let value_len = element[value_at..].find('"')?;
    Some(TagSpan {
        element: start..end,
        content: start + value_at..start + value_at + value_len,
    })
}

/// The stock page with `meta`'s values in the target tags, or `None` when a
/// target is missing (the caller then serves the stock page unchanged).
pub fn splice(html: &str, meta: &OgMeta) -> Option<String> {
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::with_capacity(12);
    for (attr, key, field) in SPLICE_TARGETS {
        let span = find_tag(html, attr, key)?;
        let image = meta.image.as_ref();
        let edit = match (field, image) {
            (TagField::Title, _) => Some((span.content, html_escape(&meta.title))),
            (TagField::Description, _) => Some((span.content, html_escape(&meta.description))),
            (TagField::Url, _) => Some((span.content, html_escape(&meta.url))),
            (_, None) => None,
            (TagField::Image, Some(img)) => Some((span.content, html_escape(&img.url))),
            (TagField::ImageType, Some(img)) => Some((span.content, html_escape(img.mime))),
            (TagField::ImageAlt, Some(img)) => Some((span.content, html_escape(&img.alt))),
            (TagField::ImageWidth, Some(img)) => Some(match img.size {
                Some((w, _)) => (span.content, w.to_string()),
                None => (span.element, String::new()),
            }),
            (TagField::ImageHeight, Some(img)) => Some(match img.size {
                Some((_, h)) => (span.content, h.to_string()),
                None => (span.element, String::new()),
            }),
        };
        if let Some(edit) = edit {
            edits.push(edit);
        }
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut out = String::with_capacity(html.len() + 512);
    let mut cursor = 0;
    for (range, value) in edits {
        if range.start < cursor {
            return None;
        }
        out.push_str(&html[cursor..range.start]);
        out.push_str(&value);
        cursor = range.end;
    }
    out.push_str(&html[cursor..]);
    Some(out)
}
