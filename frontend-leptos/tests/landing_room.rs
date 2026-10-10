//! The lit room (.plans/045 R4.1): code and image stay off the first load,
//! the bridge room.js defines is the one room.rs calls, the image it fetches
//! is the one shipped, and the hero gives it the hooks it reads.

use event_checkin_frontend::pages::landing::room::{ROOM_GLOBAL, ROOM_JS_URL};

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn room_is_lazy_and_served_from_the_copied_dir() {
    let index = read("index.html");
    assert!(index.contains(r#"<link data-trunk rel="copy-dir" href="room" />"#));
    assert!(
        !index.contains("room.js\""),
        "room.js must not be a first-load tag"
    );
    assert!(
        !index.contains("rtm6-room"),
        "the room image must not be on the first load"
    );
    assert!(ROOM_JS_URL.starts_with("/room/room.js?v="));
}

#[test]
fn bridge_and_image_match() {
    let js = read("room/room.js");
    assert!(js.contains(&format!("window.{ROOM_GLOBAL} = {{ mount: mount }}")));
    // The image URL room.js fetches is a file in room/.
    let url = js
        .split("IMG_URL = \"/room/")
        .nth(1)
        .and_then(|rest| rest.split(['?', '"']).next())
        .expect("IMG_URL");
    let image = std::fs::read(format!("{}/room/{url}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert_eq!(&image[..4], b"RIFF");
    assert_eq!(&image[8..12], b"WEBP");
}

#[test]
fn hero_carries_the_hooks_room_js_reads() {
    let hero = read("src/pages/landing/hero.rs");
    let js = read("room/room.js");
    assert!(hero.contains(r#"class="lp-room""#));
    assert!(hero.contains(r#"aria-hidden="true""#));
    // The quiet boxes are the children of this column.
    assert!(js.contains(".lp-hero-text > *"));
    assert!(hero.contains(r#"class="lp-hero-text""#));
    // Reduced motion: drawn once, never lit.
    assert!(js.contains("prefers-reduced-motion: reduce"));
}
