//! The public ticket page must never send a signed-out attendee to /login
//! (`.issues/142`, same class as `.issues/099`).
//!
//! The ticket page is "no auth required": every in-person attendee opens it at
//! the door, most of them signed out. Two components on it read the signed-in
//! attendee's own data on mount — the credit chip and the credit-refund card —
//! and both reached `api_get_json`, whose 401 path calls
//! `redirect_to_login_expired()`. The chip mounts the moment the attendee is
//! checked in, so a polling ticket bounced to the sign-in page at the door.
//!
//! The fix is `api_get_json_if_signed_in`, which maps signed-out and every
//! failure to `None`. Pin both the endpoints and the component files so a new
//! mount-time read on this page cannot quietly reach the redirecting helpers.

use std::path::PathBuf;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// The body of `async fn {name}` (any visibility, generic or not) up to its
/// closing brace at column 0.
fn fn_body<'a>(src: &'a str, name: &str) -> &'a str {
    let start = [format!("async fn {name}("), format!("async fn {name}<")]
        .iter()
        .find_map(|sig| src.find(sig.as_str()))
        .unwrap_or_else(|| panic!("`{name}` not found"));
    let rest = &src[start..];
    let end = rest.find("\n}").unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn ticket_page_reads_use_the_non_redirecting_helper() {
    let deposit = read("src/api/deposit.rs");
    for name in ["get_credit_balance", "get_credit_refund_request_status"] {
        let body = fn_body(&deposit, name);
        assert!(
            body.contains("api_get_json_if_signed_in("),
            "`{name}` is read on mount by the public ticket page; it must use \
             `api_get_json_if_signed_in`, not a helper that redirects on 401 \
             (.issues/142)"
        );
    }
}

#[test]
fn ticket_page_components_do_not_call_redirecting_get_helpers() {
    // `api_get*` / `get_me` all end a 401 in `redirect_to_login_expired()`.
    let forbidden = ["api_get(", "api_get_json(", "api_get_no_cache(", "get_me("];
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/pages/ticket");
    for entry in std::fs::read_dir(&dir)
        .expect("read src/pages/ticket")
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read source");
        for call in forbidden {
            assert!(
                !src.contains(call),
                "{} calls `{call}`, which redirects a signed-out attendee to \
                 /login; the ticket page is public (.issues/142)",
                path.display()
            );
        }
    }
}

#[test]
fn the_signed_in_helper_never_redirects() {
    let api = read("src/api/mod.rs");
    let body = fn_body(&api, "api_get_json_if_signed_in");
    assert!(
        !body.contains("redirect_to_login_expired"),
        "api_get_json_if_signed_in must never redirect"
    );
    assert!(
        body.contains("is_authenticated()"),
        "api_get_json_if_signed_in must skip the request when signed out"
    );
}
