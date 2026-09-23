//! Pins the auth fixes from the ISO 27001 gap assessment (`.issues/143`).
//!
//! Each was a one-line pattern that is easy to write again:
//! - a post-login redirect taken verbatim from the OAuth `state`;
//! - a SIWS nonce derived from the clock;
//! - a bearer secret compared with `==`/`!=`.

use std::path::PathBuf;

fn src(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

#[test]
fn oauth_state_redirect_goes_through_safe_redirect_path() {
    let auth = src("src/handlers/auth.rs");
    assert!(
        auth.contains("safe_redirect_path"),
        "auth_callback must filter the OAuth `state` through \
         validation::safe_redirect_path before redirecting (open redirect)"
    );
    assert!(
        !auth.contains("if let Some(ref state_url) = query.state"),
        "the raw OAuth `state` is being used as a redirect target again"
    );
}

#[test]
fn siws_nonce_is_not_derived_from_the_clock() {
    let auth = src("src/handlers/auth.rs");
    let start = auth
        .find("pub async fn wallet_nonce(")
        .expect("wallet_nonce");
    let body = &auth[start..start + auth[start..].find("\n}").expect("end")];
    let nonce_line = body
        .lines()
        .find(|l| l.trim_start().starts_with("let nonce ="))
        .expect("`let nonce =` in wallet_nonce");
    assert!(
        !nonce_line.contains("now"),
        "the SIWS nonce must be random, not computed from the clock: {nonce_line}"
    );
}

#[test]
fn webhook_bearer_secrets_are_compared_in_constant_time() {
    for rel in [
        "src/handlers/escrow_index.rs",
        "src/handlers/deposit/usdc/handlers/webhook.rs",
    ] {
        let s = src(rel);
        assert!(
            s.contains("constant_time_eq("),
            "{rel} must compare the webhook bearer with crypto::constant_time_eq"
        );
        for bad in ["auth_header == expected", "auth_header != expected"] {
            assert!(!s.contains(bad), "{rel}: `{bad}` is not constant-time");
        }
    }
}
