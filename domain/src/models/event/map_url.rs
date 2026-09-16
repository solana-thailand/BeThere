//! Venue map link (e.g. a Google Maps share URL) shown next to the location.
//!
//! The link is rendered as an `href` on public pages, so anything that is not
//! a plain `https://` URL is refused at write time and dropped at read time —
//! event data also arrives from Sheets and older API clients.

/// Longest map URL accepted. Google Maps place URLs with coordinates run to a
/// few hundred characters; this leaves room without storing arbitrary blobs.
pub const MAX_MAP_URL_CHARS: usize = 2048;

/// Trim `raw` and validate it as a venue map link. Blank means "no link".
///
/// # Errors
///
/// Returns a message written for the organiser looking at the form field.
pub fn normalize_map_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Ok(String::new());
    }

    let chars = url.chars().count();
    if chars > MAX_MAP_URL_CHARS {
        return Err(format!(
            "map link is {chars} characters; at most {MAX_MAP_URL_CHARS} allowed"
        ));
    }

    match url.strip_prefix("https://") {
        Some(rest) if !rest.is_empty() && !rest.starts_with('/') => {}
        _ => return Err("map link must start with https://".to_string()),
    }

    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("map link cannot contain spaces".to_string());
    }

    Ok(url.to_string())
}

/// The stored map link if it is still a safe `https://` URL, else `None`.
/// Use this before rendering a link — stored data may predate validation.
pub fn safe_map_url(stored: &str) -> Option<String> {
    normalize_map_url(stored).ok().filter(|url| !url.is_empty())
}
