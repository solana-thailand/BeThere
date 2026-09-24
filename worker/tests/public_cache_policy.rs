//! `.plans/028` W4 — the public-route cache layers used to stamp
//! `public, max-age=120` on every response, including a private event served
//! to an authorised member and the 401/403 an anonymous caller gets for it.
//! A shared cache could then hand one viewer's response to another.

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use event_checkin_worker::{CACHE_PRIVATE_NO_STORE, with_public_cache};

const PUBLIC_120: HeaderValue = HeaderValue::from_static("public, max-age=120");

fn response(status: StatusCode, cache_control: Option<&HeaderValue>) -> Response {
    let mut builder = Response::builder().status(status);
    if let Some(value) = cache_control {
        builder = builder.header(header::CACHE_CONTROL, value);
    }
    builder.body(Body::empty()).expect("response")
}

fn cache_control(response: &Response) -> &str {
    response
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .expect("Cache-Control set")
}

#[test]
fn public_success_is_publicly_cacheable() {
    let out = with_public_cache(response(StatusCode::OK, None), &PUBLIC_120);
    assert_eq!(cache_control(&out), "public, max-age=120");
}

#[test]
fn revalidation_repeats_the_public_policy() {
    let out = with_public_cache(response(StatusCode::NOT_MODIFIED, None), &PUBLIC_120);
    assert_eq!(cache_control(&out), "public, max-age=120");
}

#[test]
fn handler_private_choice_survives_the_public_layer() {
    let out = with_public_cache(
        response(StatusCode::OK, Some(&CACHE_PRIVATE_NO_STORE)),
        &PUBLIC_120,
    );
    assert_eq!(cache_control(&out), "private, no-store");
}

#[test]
fn errors_are_never_stored() {
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::NOT_FOUND,
        StatusCode::INTERNAL_SERVER_ERROR,
    ] {
        let out = with_public_cache(response(status, None), &PUBLIC_120);
        assert_eq!(cache_control(&out), "no-store", "{status}");
    }
}

#[test]
fn private_directive_forbids_shared_storage() {
    let value = CACHE_PRIVATE_NO_STORE.to_str().expect("ascii");
    assert!(
        value.contains("private") && value.contains("no-store"),
        "{value}"
    );
    assert!(!value.contains("public"), "{value}");
}
