//! An Ontime room's attendee feed, linked from an event (plan 050).
//!
//! Organisers run the show in Ontime (ontime-rs). When the room turns on its
//! attendee page (Settings → Network → Attendee page), Ontime shows two links:
//! the page, `https://<host>/r/<room>/public/agenda/`, and the feed,
//! `https://<host>/r/<room>/public`. Either may be pasted into the event; it
//! is stored as the feed URL, and the event page listens to the room's
//! listen-only socket, `wss://<host>/public/ws?room=<room>`.
//!
//! Only the room's public feed is ever stored: it needs no token and carries
//! no notes. A link with a token (`?token=`) is refused, because whoever holds
//! the token can change the show.
//!
//! The URL becomes a socket address on a public page, so anything that is not
//! a plain `https://` host and a valid room name is refused at write time and
//! dropped at read time.

/// Longest feed URL accepted: a host and a room of at most 64 characters.
pub const MAX_ONTIME_FEED_URL_CHARS: usize = 512;

/// Longest room name, as Ontime (`registry::is_valid_room`) allows.
const MAX_ROOM_CHARS: usize = 64;

/// Where an Ontime room's attendee feed lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OntimeFeed {
    /// `host` or `host:port`, no scheme.
    pub host: String,
    pub room: String,
}

impl OntimeFeed {
    /// `https://<host>/r/<room>/public`: the agenda and the show, as JSON.
    pub fn feed_url(&self) -> String {
        let OntimeFeed { host, room } = self;
        format!("https://{host}/r/{room}/public")
    }

    /// The listen-only socket that sends the feed again whenever it changes.
    pub fn socket_url(&self) -> String {
        let OntimeFeed { host, room } = self;
        format!("wss://{host}/public/ws?room={room}")
    }

    /// The room's own attendee page, in `lang` (`th` or `en`).
    pub fn agenda_url(&self, lang: &str) -> String {
        let OntimeFeed { host, room } = self;
        format!("https://{host}/r/{room}/public/agenda/?lang={lang}")
    }
}

/// Ontime's rule for a room name: 1–64 ASCII letters, digits, `-` or `_`.
fn is_valid_room(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_ROOM_CHARS
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A host name with an optional port: letters, digits, `-` and `.`, then
/// `:digits`. No user info, no brackets, nothing a URL parser could disagree
/// with.
fn is_valid_host(host: &str) -> bool {
    let (name, port) = match host.split_once(':') {
        Some((name, port)) => (name, Some(port)),
        None => (host, None),
    };
    let name_ok = !name.is_empty()
        && !name.starts_with(['.', '-'])
        && !name.ends_with(['.', '-'])
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    let port_ok = match port {
        None => true,
        Some(port) => {
            // a port the browser accepts: `WebSocket::new` throws on 99999
            port.parse::<u16>().is_ok_and(|n| n > 0) && port.chars().all(|c| c.is_ascii_digit())
        }
    };
    name_ok && port_ok
}

/// Reads a pasted Ontime link: the feed or the attendee page, with or without
/// a trailing slash or a `?lang=` query.
///
/// # Errors
///
/// A message for the organiser looking at the form field. It never contains
/// a URL scheme: error text with one is redacted on its way out.
pub fn parse_ontime_feed_url(raw: &str) -> Result<OntimeFeed, String> {
    let url = raw.trim();
    let chars = url.chars().count();
    if chars > MAX_ONTIME_FEED_URL_CHARS {
        return Err(format!(
            "Ontime link is {chars} characters; at most {MAX_ONTIME_FEED_URL_CHARS} allowed"
        ));
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("Ontime link cannot contain spaces".to_string());
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return Err("Ontime link must be a secure (https) web address".to_string());
    };
    let (path_part, query) = match rest.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (rest, None),
    };
    if path_part.contains('#') || query.is_some_and(|q| q.contains('#')) {
        return Err("Ontime link cannot contain a #".to_string());
    }
    if query.is_some_and(|q| q.split('&').any(|pair| pair.starts_with("token="))) {
        return Err(
            "That link carries the room's token, which lets anyone change the show. \
             Copy the attendee page link from Ontime instead"
                .to_string(),
        );
    }
    let Some((host, path)) = path_part.split_once('/') else {
        return Err(not_a_feed());
    };
    if !is_valid_host(host) {
        return Err("Ontime link has an unreadable host name".to_string());
    }
    let mut segments = path.split('/');
    let room = match (segments.next(), segments.next(), segments.next()) {
        (Some("r"), Some(room), Some("public")) => room,
        _ => return Err(not_a_feed()),
    };
    // after `public`: nothing, or the attendee page
    let tail: Vec<&str> = segments.filter(|s| !s.is_empty()).collect();
    if !matches!(tail.as_slice(), [] | ["agenda"]) {
        return Err(not_a_feed());
    }
    if !is_valid_room(room) {
        return Err("Ontime room names are letters, digits, - and _ (at most 64)".to_string());
    }
    Ok(OntimeFeed {
        host: host.to_ascii_lowercase(),
        room: room.to_string(),
    })
}

fn not_a_feed() -> String {
    "Not an Ontime attendee link. In Ontime, open Settings, Network, Attendee page, \
     turn it on and copy the page link"
        .to_string()
}

/// Trim and validate a pasted Ontime link, as the feed URL to store. Blank
/// means "no live agenda".
///
/// # Errors
///
/// See [`parse_ontime_feed_url`].
pub fn normalize_ontime_feed_url(raw: &str) -> Result<String, String> {
    if raw.trim().is_empty() {
        return Ok(String::new());
    }
    parse_ontime_feed_url(raw).map(|feed| feed.feed_url())
}

/// The stored link if it is still a valid Ontime feed, else `None`. Use this
/// before opening a socket: stored data may predate validation.
pub fn safe_ontime_feed(stored: &str) -> Option<OntimeFeed> {
    match stored.trim() {
        "" => None,
        url => parse_ontime_feed_url(url).ok(),
    }
}
