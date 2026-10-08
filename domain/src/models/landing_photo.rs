//! A landing photo as `worker/landing-photos.jsonl` lists it (.plans/043
//! L9): shared by the worker, which serves the list and only the images on
//! it, and the frontend, which draws the reel from it.

use serde::{Deserialize, Serialize};

/// What a photo shows; only these kinds are allowed on the landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhotoKind {
    Room,
    Group,
    Food,
}

/// One line of `landing-photos.jsonl`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandingPhoto {
    /// Full size (long side ≤ 1600 px), shown only on tap.
    pub file: String,
    /// 480 px thumbnail for the reel.
    pub thumb: String,
    /// The caption: an event label such as "RTM #1", nothing else.
    pub event: String,
    pub kind: PhotoKind,
    pub w: u32,
    pub h: u32,
    pub alt_en: String,
    pub alt_th: String,
    pub sha256: String,
    pub thumb_sha256: String,
}
