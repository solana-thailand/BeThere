//! `/showed/{token}` (Super GOAT SG4): only a token-shaped path is a share
//! page; its tags name the event and never the person; the image is the card
//! when there is one; the routes sit where a revoke takes effect at once.

use event_checkin_worker::showed_page::{meta, page_token};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn only_token_shaped_paths() {
    assert_eq!(
        page_token(&format!("/showed/{TOKEN}")).as_deref(),
        Some(TOKEN)
    );
    assert_eq!(
        page_token(&format!("/showed/{TOKEN}/")).as_deref(),
        Some(TOKEN)
    );
    assert_eq!(page_token("/showed/abc"), None);
    assert_eq!(page_token(&format!("/showed/{TOKEN}/x")), None);
    assert_eq!(page_token(&format!("/e/{TOKEN}")), None);
}

#[test]
fn tags_name_the_event_not_the_person() {
    let m = meta(
        "https://x.example",
        TOKEN,
        "RTM  #6\n(Bangkok)",
        1_759_550_400_000,
        "/api/storage/og-showed/e6",
    );
    assert_eq!(m.title, "I showed up · RTM #6 (Bangkok)");
    assert!(m.description.starts_with("RTM #6 (Bangkok), "));
    assert_eq!(m.url, format!("https://x.example/showed/{TOKEN}"));
    let image = m.image.expect("card");
    assert_eq!(image.url, "https://x.example/api/storage/og-showed/e6");
    assert_eq!(image.size, Some((1200, 630)));
    assert!(meta("https://x.example", TOKEN, "E", 0, "").image.is_none());
}

#[test]
fn routes_are_authed_and_uncached() {
    let handlers = include_str!("../src/handlers/mod.rs");
    let me = handlers.find(r#""/me/showed/{key}","#).expect("me route");
    let authed = handlers
        .find(r#".route("/me/record""#)
        .expect("authed anchor");
    assert!(!handlers[authed.min(me)..authed.max(me)].contains("Router::new()"));
    // the public read sits in the no-store group, never a cached one
    let public = handlers
        .find(r#".route("/public/showed/{token}""#)
        .expect("public route");
    let layer_end = public + handlers[public..].find(".layer(").expect("its layer");
    assert!(handlers[layer_end..layer_end + 120].contains("cache_no_store_layer"));
    // the share handler decides with door_history before it makes a link
    let src = include_str!("../src/handlers/showed.rs");
    let check = src
        .find("may_share(&record.events, &slug)")
        .expect("came check");
    assert!(check < src.find("crate::showed::link(").expect("link"));
}

#[test]
fn only_a_token_shape_is_a_page_route() {
    use event_checkin_worker::crawl::is_app_route;
    assert!(is_app_route(&format!("/showed/{TOKEN}")));
    assert!(!is_app_route("/showed/xyz"), "a non-token is a real 404");
    assert!(!is_app_route("/showed/0123456789ABCDEF0123456789ABCDEF"));
    // the other :token routes keep matching any segment
    assert!(is_app_route("/claim/anything") && is_app_route("/unsubscribe/anything"));
}
