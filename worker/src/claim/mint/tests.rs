use super::helpers::{crossmint_image_url, orb_nft_url};

// ── crossmint_image_url: SVG→PNG rewrite for the mint payload ──
#[test]
fn image_url_rewrites_hd_svg_to_png() {
    assert_eq!(
        crossmint_image_url("https://x.dev/api/badge-hd.svg"),
        "https://x.dev/api/badge-hd.png"
    );
}

#[test]
fn image_url_rewrites_plain_svg_to_png() {
    assert_eq!(crossmint_image_url("/api/badge.svg"), "/api/badge.png");
}

#[test]
fn image_url_passes_through_png() {
    assert_eq!(
        crossmint_image_url("https://x.dev/a.png"),
        "https://x.dev/a.png"
    );
}

#[test]
fn image_url_passes_through_empty_and_non_svg() {
    assert_eq!(crossmint_image_url(""), "");
    assert_eq!(
        crossmint_image_url("https://x.dev/img"),
        "https://x.dev/img"
    );
    // Only a trailing .svg is rewritten — a mid-string ".svg" is untouched.
    assert_eq!(
        crossmint_image_url("https://x.dev/a.svg.jpg"),
        "https://x.dev/a.svg.jpg"
    );
}

// ── orb_nft_url: explorer link cluster mapping ──
#[test]
fn orb_url_mainnet_uses_mainnet_param() {
    let u = orb_nft_url("AID", "mainnet-beta");
    assert!(u.contains("AID"));
    assert!(u.contains("cluster=mainnet"));
    assert!(!u.contains("cluster=devnet"));
}

#[test]
fn orb_url_devnet_default() {
    assert!(orb_nft_url("AID", "devnet").contains("cluster=devnet"));
}
