//! The self-hosted Mobile Wallet Adapter bundle (`.issues/189`) must be the
//! bytes its rebuild recipe produces, and must be served like the other
//! versioned vendor file (immutable cache).
//!
//! The sha256 here is the one in
//! `frontend-leptos/vendor/mwa-wallet-standard-mobile-0.5.3.LICENSE.txt`. A
//! hand edit or a rebuild with different dependency versions changes it; then
//! update the recipe, the LICENSE list and this pin together.

use sha2::{Digest, Sha256};

const VENDORED: &[u8] =
    include_bytes!("../../frontend-leptos/vendor/mwa-wallet-standard-mobile-0.5.3.js");
const LICENSE: &str =
    include_str!("../../frontend-leptos/vendor/mwa-wallet-standard-mobile-0.5.3.LICENSE.txt");
const HEADERS: &str = include_str!("../../frontend-leptos/_headers");

const SHA256: &str = "bae6e70448226d5630f837ea13d7d2f0010a559bf5af295ee81bac9dd48a4633";

#[test]
fn vendored_bytes_match_the_recipe_hash() {
    let actual: String = Sha256::digest(VENDORED)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        actual, SHA256,
        "vendor/mwa-wallet-standard-mobile-0.5.3.js changed"
    );
    assert!(
        LICENSE.contains(&format!("sha256: {SHA256}")),
        "the LICENSE build note must name the same hash"
    );
    assert!(LICENSE.contains(&format!("Output: {} bytes", VENDORED.len())));
}

#[test]
fn bundle_is_cached_immutably() {
    assert!(HEADERS.contains("/mwa-*.js\n  Cache-Control: public, max-age=31536000, immutable"));
}
