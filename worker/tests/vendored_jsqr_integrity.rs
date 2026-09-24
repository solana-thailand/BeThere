//! The self-hosted jsQR decoder (`.plans/029` §2) must be the bytes its SRI pin
//! names, and must actually ship.
//!
//! A mismatch would not fail any build: the browser would refuse the script at
//! scan time, and only on iOS, which is the one platform that needs jsQR.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha384};

const VENDORED: &[u8] = include_bytes!("../../frontend-leptos/vendor/jsqr-1.4.0.js");
const LOADER: &str = include_str!("../../frontend-leptos/js/lazy_assets.js");
const INDEX: &str = include_str!("../../frontend-leptos/index.html");
const HEADERS: &str = include_str!("../../frontend-leptos/_headers");

fn js_string_var(src: &str, name: &str) -> String {
    let start = src
        .find(&format!("var {name} ="))
        .unwrap_or_else(|| panic!("{name} not found"));
    let rest = &src[start..];
    let open = rest.find('"').expect("opening quote") + 1;
    let close = rest[open..].find('"').expect("closing quote") + open;
    rest[open..close].to_string()
}

#[test]
fn vendored_bytes_match_the_integrity_pin() {
    let pin = js_string_var(LOADER, "JSQR_INTEGRITY");
    let actual = format!("sha384-{}", STANDARD.encode(Sha384::digest(VENDORED)));
    assert_eq!(
        actual, pin,
        "vendor/jsqr-1.4.0.js does not match JSQR_INTEGRITY"
    );
}

#[test]
fn loader_points_at_the_same_origin_copy_that_the_build_ships() {
    let url = js_string_var(LOADER, "JSQR_URL");
    assert_eq!(url, "/jsqr-1.4.0.js", "jsQR must be same-origin");
    assert!(
        INDEX.contains(r#"<link data-trunk rel="copy-file" href="vendor/jsqr-1.4.0.js" />"#),
        "index.html must copy the vendored file into dist/"
    );
    assert!(
        INDEX.contains("vendor/jsqr-1.4.0.LICENSE.txt"),
        "Apache-2.0 requires shipping the license with the file"
    );
    assert!(HEADERS.contains("/jsqr-*.js\n  Cache-Control: public, max-age=31536000, immutable"));
    assert!(!LOADER.contains("cdn.jsdelivr.net"));
}
