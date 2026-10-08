//! The landing's event photos (.plans/043 L9): the list is one file in the
//! repo, `worker/landing-photos.jsonl`, one photo per line; the images live in
//! R2 under `landing-photos/`, never in git (the repo is public, and a photo
//! taken down must not stay in its history).
//!
//! Taking a photo down: delete its line here and its two objects in R2
//! (`npx wrangler r2 object delete <bucket>/landing-photos/<file> --remote`,
//! and the thumbnail). Deleting the line alone already stops it being served:
//! the storage route serves only names on this list, and caches keep a copy
//! for an hour at most ([`crate::storage::Visibility::Removable`]).
//!
//! Owner's rules (6 Oct 2026): only low-risk room shots and group photos that
//! were already posted; captions are the event label only; all metadata is
//! stripped before upload (`scripts/landing_photos_upload.py` checks each
//! object's sha256 against this list).

pub use event_checkin_domain::models::landing_photo::{LandingPhoto, PhotoKind};

/// R2 key prefix for the landing photos.
pub const PREFIX: &str = "landing-photos/";

const LIST: &str = include_str!("../landing-photos.jsonl");

/// Parse the list: one JSON object per non-empty line.
pub fn parse(text: &str) -> Result<Vec<LandingPhoto>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            serde_json::from_str(line)
                .map_err(|e| format!("landing-photos.jsonl line {}: {e}", i + 1))
        })
        .collect()
}

/// The committed list. A line that does not parse leaves the reel empty
/// rather than serving a half-read list.
pub fn photos() -> Vec<LandingPhoto> {
    parse(LIST).unwrap_or_else(|e| {
        tracing::error!(error = %e, "landing photo list does not parse");
        Vec::new()
    })
}

/// Is `name` (a bare file name, no path) a full-size image or thumbnail on
/// the list?
pub fn is_listed(photos: &[LandingPhoto], name: &str) -> bool {
    photos.iter().any(|p| p.file == name || p.thumb == name)
}
