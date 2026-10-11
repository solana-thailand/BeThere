//! `/showed/{token}` (Super GOAT SG4): the shell with the event's "I showed
//! up" tags while the link is live; `None` (a 404) for an unknown or revoked
//! token, so revoking kills the page and its preview. The tags name the event,
//! never the person. `crate::showed` holds the links.

use event_checkin_domain::models::record::is_showed_token;
use event_checkin_domain::og_card::{OG_HEIGHT, OG_WIDTH, event_summary};
use worker::Env;

use crate::og_meta::{OgImage, OgMeta, RasterKind, splice, truncate_chars};

/// The token of a `/showed/{token}` path with a token's shape.
pub fn page_token(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/showed/")?;
    let token = rest.strip_suffix('/').unwrap_or(rest);
    is_showed_token(token).then(|| token.to_string())
}

/// The tags for a live link (pure; tested in `tests/showed_page.rs`).
pub fn meta(origin: &str, token: &str, name: &str, start_ms: i64, card: &str) -> OgMeta {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let when = event_summary(start_ms, false, "");
    OgMeta {
        title: format!("I showed up · {name}"),
        description: truncate_chars(
            &format!(
                "{name}, {when}. Free events on BeThere: hold your seat with a deposit, show up, get it all back."
            ),
            200,
        ),
        url: format!("{origin}/showed/{token}"),
        image: (!card.is_empty()).then(|| OgImage {
            url: format!("{origin}{card}"),
            mime: RasterKind::Png.mime(),
            size: Some((OG_WIDTH, OG_HEIGHT)),
            alt: format!("I showed up: {name}"),
        }),
    }
}

/// The spliced shell, or `None` for a 404 (unknown, revoked, any failure).
pub async fn render(env: &Env, token: &str, stock: &str) -> Option<String> {
    let db = env.d1("DB").ok()?;
    let (event_id, page) = match crate::showed::page(&db, token).await {
        Ok(found) => found?,
        Err(e) => {
            tracing::warn!(error = %e, "showed: link read failed");
            return None;
        }
    };
    let card = match env.bucket("ASSETS_BUCKET") {
        Ok(bucket) => crate::showed::card(&bucket, &event_id).await,
        Err(_) => String::new(),
    };
    let origin = env
        .var("SERVER_URL")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| crate::crawl::CANONICAL_ORIGIN.to_string())
        .trim_end_matches('/')
        .to_string();
    splice(
        stock,
        &meta(&origin, token, &page.event_name, page.start_ms, &card),
    )
}
