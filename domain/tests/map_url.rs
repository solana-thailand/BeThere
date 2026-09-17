//! Venue map link validation — the stored value becomes a public `href`.

use event_checkin_domain::models::event::{MAX_MAP_URL_CHARS, normalize_map_url, safe_map_url};

#[test]
fn blank_means_no_link() {
    assert_eq!(normalize_map_url("   "), Ok(String::new()));
    assert_eq!(safe_map_url(""), None);
}

#[test]
fn accepts_and_trims_https_map_links() {
    assert_eq!(
        normalize_map_url("  https://maps.app.goo.gl/abc123  "),
        Ok("https://maps.app.goo.gl/abc123".to_string())
    );
    let place = "https://www.google.com/maps/place/Bangkok/@13.7563,100.5018,12z?entry=ttu";
    assert_eq!(safe_map_url(place), Some(place.to_string()));
}

#[test]
fn rejects_non_https_schemes() {
    for bad in [
        "http://maps.google.com/?q=bangkok",
        "javascript:alert(1)",
        "maps.app.goo.gl/abc123",
        "https://",
        "https:///evil",
        "data:text/html,hi",
    ] {
        assert!(normalize_map_url(bad).is_err(), "{bad} should be rejected");
        assert_eq!(safe_map_url(bad), None, "{bad} should not render");
    }
}

#[test]
fn rejects_embedded_whitespace_and_oversize() {
    assert!(normalize_map_url("https://maps.app.goo.gl/a b").is_err());
    assert!(normalize_map_url("https://maps.app.goo.gl/a\nb").is_err());
    let long = format!("https://maps.app.goo.gl/{}", "a".repeat(MAX_MAP_URL_CHARS));
    assert!(normalize_map_url(&long).is_err());
}
