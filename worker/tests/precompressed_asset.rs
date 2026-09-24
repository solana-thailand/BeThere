//! The Worker serves the frontend wasm pre-compressed (`.issues/135` §6.3).
//! These pin the two pure decisions that route and negotiate it: routing the
//! wrong path through the Worker, or sending brotli to a client that refused
//! it, both end in a blank app rather than a slow one.

use event_checkin_worker::precompressed::{accepts_brotli, is_precompressed_asset};

#[test]
fn only_the_trunk_wasm_is_routed() {
    assert!(is_precompressed_asset(
        "/event-checkin-frontend-9ca2d4c145396600_bg.wasm"
    ));
    for path in [
        "/event-checkin-frontend-9ca2d4c145396600.js",
        "/event-checkin-frontend-_bg.wasm",
        "/event-checkin-frontend-9ca2d4c145396600_bg.wasm.br",
        "/snippets/event-checkin-frontend-x/event-checkin-frontend-1_bg.wasm",
        "/api/event-checkin-frontend-1_bg.wasm",
        "/",
    ] {
        assert!(!is_precompressed_asset(path), "{path} must not be routed");
    }
}

#[test]
fn brotli_is_sent_only_when_accepted() {
    for accepted in [
        "gzip, deflate, br, zstd",
        "br",
        "BR",
        "gzip;q=1.0, br;q=0.5",
        " br ; q=1",
    ] {
        assert!(accepts_brotli(accepted), "{accepted:?} accepts br");
    }
    for refused in [
        "",
        "gzip, deflate",
        "br;q=0",
        "br; q=0.0",
        "brotli",
        "gzip;q=0.5, *;q=0.1",
    ] {
        assert!(!accepts_brotli(refused), "{refused:?} does not accept br");
    }
}
