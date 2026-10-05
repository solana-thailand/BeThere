//! Event sponsors (migration 0057): a name, an optional logo and an optional
//! link, rendered as a logo row on the public event page.
//!
//! Both URLs land in `src`/`href` attributes on a public page, so they are
//! held to `https://` here — at the write — rather than trusted at render.
//! The list rides in the event JSON cached in KV and is read on every public
//! page load, so its size is bounded too.

use serde::{Deserialize, Serialize};

/// Most sponsors one event may list.
pub const MAX_SPONSORS: usize = 12;

/// Longest sponsor name accepted, in characters.
pub const MAX_SPONSOR_NAME_CHARS: usize = 80;

/// Longest logo or link URL accepted, in characters.
pub const MAX_SPONSOR_URL_CHARS: usize = 500;

/// One sponsor of an event.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sponsor {
    /// Display name; also the logo's alt text and the fallback when there is
    /// no logo.
    pub name: String,
    /// `https://` image URL. Empty = the name is shown instead of a logo.
    #[serde(default)]
    pub logo_url: String,
    /// `https://` link to the sponsor. Empty = the logo is not a link.
    #[serde(default)]
    pub link: String,
}

/// Trim every sponsor, drop rows that are entirely blank, and check names,
/// URLs and the count. Refuses rather than truncates.
///
/// # Errors
///
/// Returns a message written for the organizer looking at the form.
pub fn normalize_sponsors(raw: &[Sponsor]) -> Result<Vec<Sponsor>, String> {
    let sponsors: Vec<Sponsor> = raw
        .iter()
        .map(|s| Sponsor {
            name: s.name.trim().to_string(),
            logo_url: s.logo_url.trim().to_string(),
            link: s.link.trim().to_string(),
        })
        .filter(|s| !(s.name.is_empty() && s.logo_url.is_empty() && s.link.is_empty()))
        .collect();

    if sponsors.len() > MAX_SPONSORS {
        return Err(format!(
            "{} sponsors listed; at most {MAX_SPONSORS} allowed",
            sponsors.len()
        ));
    }
    for (i, s) in sponsors.iter().enumerate() {
        let row = i + 1;
        let name_chars = s.name.chars().count();
        match name_chars {
            0 => return Err(format!("sponsor {row} needs a name")),
            n if n > MAX_SPONSOR_NAME_CHARS => {
                return Err(format!(
                    "sponsor {row} name is {n} characters; at most {MAX_SPONSOR_NAME_CHARS} allowed"
                ));
            }
            _ => {}
        }
        check_sponsor_url(&s.logo_url, row, "logo")?;
        check_sponsor_url(&s.link, row, "link")?;
    }
    Ok(sponsors)
}

/// Empty, or an `https` URL with a host and no whitespace. The message avoids
/// a scheme separator on purpose: the API error redactor rewrites it.
fn check_sponsor_url(url: &str, row: usize, what: &str) -> Result<(), String> {
    if url.is_empty() {
        return Ok(());
    }
    if url.chars().count() > MAX_SPONSOR_URL_CHARS {
        return Err(format!(
            "sponsor {row} {what} is longer than {MAX_SPONSOR_URL_CHARS} characters"
        ));
    }
    let host_ok = url
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .is_some_and(|host| !host.is_empty());
    match host_ok && !url.chars().any(char::is_whitespace) {
        true => Ok(()),
        false => Err(format!("sponsor {row} {what} must be an https address")),
    }
}
