//! The Worker serves the frontend wasm pre-compressed (`.issues/135` §6.3).
//! These pin the two pure decisions that route and negotiate it: routing the
//! wrong path through the Worker, or sending brotli to a client that refused
//! it, both end in a blank app rather than a slow one.

use event_checkin_worker::precompressed::{
    PRECOMPRESSED_ASSETS, accepts_brotli, is_precompressed_asset, precompressed_asset,
};

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

#[test]
fn the_vendored_jsqr_is_routed_as_javascript() {
    let asset = precompressed_asset("/jsqr-1.4.0.js").expect("jsQR is routed");
    assert_eq!(asset.content_type, "text/javascript");
    assert_eq!(
        precompressed_asset("/event-checkin-frontend-1_bg.wasm").map(|a| a.content_type),
        Some("application/wasm")
    );
    for path in [
        "/jsqr-1.4.0.LICENSE.txt",
        "/jsqr-1.4.0.js.br",
        "/jsqr-.js",
        "/snippets/x/jsqr-1.4.0.js",
        "/api/jsqr-1.4.0.js",
    ] {
        assert!(!is_precompressed_asset(path), "{path} must not be routed");
    }
}

/// The table, `wrangler.toml`'s `run_worker_first` and `build.sh`'s
/// precompress list must name the same files. A table entry without its glob
/// is never routed to the Worker (a silent loss of the saving); a glob
/// without an entry sends that path to the SPA fallback instead of the file.
#[test]
fn table_routes_and_build_list_agree() {
    const WRANGLER: &str = include_str!("../wrangler.toml");
    const BUILD: &str = include_str!("../../frontend-leptos/build.sh");
    let line = WRANGLER
        .lines()
        .find(|l| l.trim_start().starts_with("run_worker_first"))
        .expect("run_worker_first moved");
    let globs: Vec<&str> = line
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|g| *g != "/api/*")
        .collect();
    let expected: Vec<String> = PRECOMPRESSED_ASSETS
        .iter()
        .map(|a| format!("{}*{}", a.prefix, a.suffix))
        .collect();
    assert_eq!(globs, expected);
    for a in &PRECOMPRESSED_ASSETS {
        let dist = format!("dist{}*{}", a.prefix, a.suffix);
        assert!(
            BUILD.contains(&dist),
            "build.sh does not precompress {dist}"
        );
    }
}
