//! The service worker must not force `no-store` on R2 objects. Posters are
//! served `public, max-age=86400`; the SW's network-only `/api/*` branch used
//! to swallow `/api/storage/*` too, so a 3.9 MB poster was re-downloaded on
//! every page view. Slips stay uncached because the worker marks them
//! `private, no-store` itself.

const SW: &str = include_str!("../sw.js");

#[test]
fn storage_is_passed_through_before_the_network_only_branch() {
    let pass = SW
        .find("if (url.pathname.startsWith(\"/api/storage/\")) return;")
        .expect("/api/storage/ must be left to the browser");
    let api = SW
        .find("if (url.pathname.startsWith(\"/api/\")) {")
        .expect("network-only /api/ branch");
    assert!(pass < api, "the storage pass-through must come first");
}

#[test]
fn other_api_calls_stay_network_only() {
    let api = SW.find("if (url.pathname.startsWith(\"/api/\")) {").unwrap();
    assert!(SW[api..].contains("event.respondWith(networkOnly(req));"));
}
