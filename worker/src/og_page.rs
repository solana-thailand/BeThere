//! `/e/{slug}` with the event's own social tags (`.issues/183` Part 1).
//!
//! The page path already reaches the Worker: it is not a file, and
//! `[assets] not_found_handling = "none"` hands it to `fetch`, which serves the
//! embedded shell (`crawl::route_kind` → `RouteKind::App`). This module only
//! decides what goes in the head; `lib.rs` keeps sending the same headers.
//!
//! Reads: the event KV-first, through the same resolver as
//! `GET /api/public/event/{slug}` (KV masks direct D1 writes), and at most
//! three R2 HEADs (stored poster as PNG / JPEG, the card), issued together.
//! Any miss or error serves the stock page: a share preview must never cost
//! the page itself.

use worker::Env;

use crate::og_meta::{
    self, ImageChoice, PosterSource, RasterKind, choose_image, classify_poster, is_shareable,
    meta_for_event, splice,
};
use crate::storage;

/// The spliced shell for `slug`, or `None` to serve `stock` unchanged.
pub async fn render(env: &Env, slug: &str, stock: &str) -> Option<String> {
    let kv = env.kv("EVENTS").ok();
    let d1 = env.d1("DB").ok();
    let event =
        match crate::event_store::read::resolve_event_by_slug(kv.as_ref(), slug, d1.as_ref()).await
        {
            Ok(event) => event,
            Err(crate::event_store::read::ResolveError::NotFound(_)) => return None,
            Err(crate::event_store::read::ResolveError::Backend(e)) => {
                tracing::warn!(error = %e, "og: event read failed, serving the stock head");
                return None;
            }
        };
    if !is_shareable(&event) || !og_meta::is_safe_id(&event.id) {
        return None;
    }
    let origin = origin(env);
    let image = pick_image(env, &event.id, &event.poster_url, &origin).await;
    let meta = meta_for_event(&event, &origin, image);
    let spliced = splice(stock, &meta);
    if spliced.is_none() {
        tracing::error!("og: index.html lost a target tag, serving the stock head");
    }
    spliced
}

/// `SERVER_URL` (prod or staging), without a trailing slash.
fn origin(env: &Env) -> String {
    env.var("SERVER_URL")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| crate::crawl::CANONICAL_ORIGIN.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Raster poster → `og/{event_id}.png` → stock.
async fn pick_image(env: &Env, event_id: &str, poster_url: &str, origin: &str) -> ImageChoice {
    let source = classify_poster(poster_url);
    if let PosterSource::External { url, kind } = source {
        return choose_image(Some((url, kind)), false);
    }
    let Ok(bucket) = env.bucket("ASSETS_BUCKET") else {
        return ImageChoice::Stock;
    };
    let card_key = storage::og_card_key(event_id);
    let PosterSource::Stored {
        event_id: poster_id,
    } = source
    else {
        // No poster, or one a crawler cannot use (SVG, WebP, http, …).
        return choose_image(None, present(&bucket, &card_key).await);
    };
    // The upload route keeps one format per event, so at most one of the two
    // poster keys exists.
    let png_key = storage::poster_key(&poster_id, "png");
    let jpg_key = storage::poster_key(&poster_id, "jpg");
    let (png, jpg, card) = futures_util::future::join3(
        present(&bucket, &png_key),
        present(&bucket, &jpg_key),
        present(&bucket, &card_key),
    )
    .await;
    let kind = match (png, jpg) {
        (true, _) => Some(RasterKind::Png),
        (false, true) => Some(RasterKind::Jpeg),
        (false, false) => None,
    };
    let url = format!("{origin}{}{poster_id}", og_meta::STORED_POSTER_PREFIX);
    choose_image(kind.map(|k| (url, k)), card)
}

/// Whether R2 holds `key`; an error counts as absent.
async fn present(bucket: &worker::Bucket, key: &str) -> bool {
    match storage::exists(bucket, key).await {
        Ok(found) => found,
        Err(e) => {
            tracing::warn!(key = %key, error = ?e, "og: R2 head failed");
            false
        }
    }
}
