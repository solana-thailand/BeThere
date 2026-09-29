//! `.plans/028` W4 — the edge cache is shared by every visitor, so what it
//! keys and what it stores decide whether one viewer's response can reach
//! another. These pin the two pure rules the layer applies.

use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use event_checkin_worker::{edge_cache_key, is_edge_storable};

const PATH: &str = "/api/public/event/solana-bangkok";

fn headers(pairs: &[(header::HeaderName, &'static str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.append(name.clone(), HeaderValue::from_static(value));
    }
    map
}

fn anon() -> HeaderMap {
    headers(&[(header::HOST, "bethere.example.workers.dev")])
}

#[test]
fn anonymous_get_is_keyed_by_host_and_path() {
    assert_eq!(
        edge_cache_key(&Method::GET, &anon(), PATH).as_deref(),
        Some("https://bethere.example.workers.dev/api/public/event/solana-bangkok")
    );
}

#[test]
fn bearer_token_bypasses_the_cache() {
    let mut map = anon();
    map.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer abc"),
    );
    assert_eq!(edge_cache_key(&Method::GET, &map, PATH), None);
}

#[test]
fn session_cookie_bypasses_the_cache() {
    let mut map = anon();
    map.insert(
        header::COOKIE,
        HeaderValue::from_static("theme=dark; event_checkin_token=abc"),
    );
    assert_eq!(edge_cache_key(&Method::GET, &map, PATH), None);
}

#[test]
fn unrelated_cookie_still_uses_the_cache() {
    let mut map = anon();
    map.insert(header::COOKIE, HeaderValue::from_static("theme=dark"));
    assert!(edge_cache_key(&Method::GET, &map, PATH).is_some());
}

#[test]
fn non_get_and_missing_host_bypass_the_cache() {
    assert_eq!(edge_cache_key(&Method::HEAD, &anon(), PATH), None);
    assert_eq!(edge_cache_key(&Method::POST, &anon(), PATH), None);
    assert_eq!(edge_cache_key(&Method::GET, &HeaderMap::new(), PATH), None);
}

#[test]
fn plain_ok_is_storable() {
    assert!(is_edge_storable(StatusCode::OK, &HeaderMap::new()));
}

#[test]
fn handler_chosen_cache_control_is_never_stored() {
    let map = headers(&[(header::CACHE_CONTROL, "private, no-store")]);
    assert!(!is_edge_storable(StatusCode::OK, &map));
}

#[test]
fn set_cookie_is_never_stored() {
    let map = headers(&[(header::SET_COOKIE, "a=b")]);
    assert!(!is_edge_storable(StatusCode::OK, &map));
}

#[test]
fn errors_and_revalidations_are_never_stored() {
    for status in [
        StatusCode::NOT_MODIFIED,
        StatusCode::NO_CONTENT,
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::NOT_FOUND,
        StatusCode::INTERNAL_SERVER_ERROR,
    ] {
        assert!(!is_edge_storable(status, &HeaderMap::new()), "{status}");
    }
}
