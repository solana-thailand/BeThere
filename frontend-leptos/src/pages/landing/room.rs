//! The lit room behind the hero (.plans/045 R4.1): the mount bridge only.
//! The drawing is `room/room.js` and its image `room/rtm6-room.webp`, both
//! copied to `dist/room/` with no tag, so neither is on the first load; this
//! injects the script once the hero is in the page and hands it the hero
//! and its canvas. Until the image is in, the hero shows its night colour.

use leptos::html::{Canvas, Header};
use leptos::prelude::*;
use wasm_bindgen::JsValue;

/// Bump `v` whenever `room/room.js` changes: the file is not content-hashed,
/// and the service worker keeps whatever URL it saw (stale-while-revalidate).
/// The image's own `?v=` is in room.js.
pub const ROOM_JS_URL: &str = "/room/room.js?v=1";
/// The global `room.js` defines.
pub const ROOM_GLOBAL: &str = "bethereRoom";

/// Light the room on `canvas`, with `area` (the hero) taking the pointer.
pub fn use_lit_room(area: NodeRef<Header>, canvas: NodeRef<Canvas>) {
    Effect::new(move |_| {
        let (Some(area), Some(canvas)) = (area.get(), canvas.get()) else {
            return;
        };
        crate::utils::lazy_script::with_mount(ROOM_JS_URL, ROOM_GLOBAL, move |mount| {
            if let Err(e) = mount.call2(&JsValue::NULL, &area, &canvas) {
                log::warn!("[landing] room mount: {e:?}");
            }
        });
    });
}
