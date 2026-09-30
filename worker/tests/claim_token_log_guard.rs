//! Capability-bearing claim tokens must never cross into application logs.

use std::{fs, path::Path};

fn rust_sources(path: &Path, output: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).expect("source directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            rust_sources(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

#[test]
fn claim_tokens_are_fingerprinted_before_logging() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_root, &mut sources);

    for path in sources {
        let source = fs::read_to_string(&path).expect("Rust source is UTF-8");
        assert!(
            !source.contains("claim_token = %") && !source.contains("token = %"),
            "raw claim-token tracing field in {}",
            path.display()
        );
        assert!(
            !source.contains("claim token {token}"),
            "raw claim token interpolated into log message in {}",
            path.display()
        );
    }
}

/// The GitHub token exchange must not echo the response body into its error:
/// the callback logs that error, and a form-encoded answer carries
/// `access_token=…`.
#[test]
fn github_token_response_body_is_not_echoed() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers/social_link.rs");
    let source = fs::read_to_string(&path).expect("social_link.rs is readable");
    let body = fn_body(&source, "async fn exchange_github_code");
    assert!(
        !body.contains("{text}") && !body.contains("{text:"),
        "exchange_github_code interpolates the raw token response into an error"
    );
    assert!(
        body.contains("http::post_json_quiet("),
        "exchange_github_code must use the quiet helper; the others quote the body"
    );
}

fn fn_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("{signature} moved; update this guard"));
    let end = source[start..]
        .find("\n}\n")
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("end of {signature}"));
    &source[start..end]
}

/// The GitHub user lookup returns profile data; its errors are logged too.
#[test]
fn github_user_lookup_uses_the_quiet_helper() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers/social_link.rs");
    let source = fs::read_to_string(&path).expect("social_link.rs is readable");
    let body = fn_body(&source, "async fn github_get_user");
    assert!(
        body.contains("http::get_json_quiet("),
        "github_get_user must use the quiet helper; the others quote the body"
    );
}

/// The quiet helpers must omit the body on a bad status, and no helper may
/// parse with `Response::json`: V8's `JSON.parse` error quotes the text.
#[test]
fn quiet_http_helpers_never_quote_the_body() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http.rs");
    let source = fs::read_to_string(&path).expect("http.rs is readable");
    assert!(
        !source.contains(".json()"),
        "http.rs parses with Response::json; use read_json"
    );
    let read = fn_body(&source, "async fn read_json");
    assert!(
        read.contains("parse_quiet("),
        "read_json must parse with parse_quiet"
    );
    for signature in [
        "pub async fn get_json_quiet",
        "pub async fn post_json_quiet",
    ] {
        let body = fn_body(&source, signature);
        assert!(
            body.contains("ErrorBody::Omit") && body.contains("read_json("),
            "{signature} must send with ErrorBody::Omit and parse with read_json"
        );
    }
    let parse = fn_body(&source, "fn parse_quiet");
    assert!(
        !parse.contains("{text}") && !parse.contains("{text:"),
        "parse_quiet quotes the body"
    );
    let check = fn_body(&source, "async fn check_status");
    let omit = check
        .find("ErrorBody::Omit")
        .expect("check_status handles ErrorBody::Omit");
    let read = check.find(".text()").expect("check_status reads the body");
    assert!(
        omit < read,
        "check_status reads the body before the Omit return"
    );
}

/// Google's userinfo body carries the user's email, and the login path logs
/// the error; a bad status must not quote it.
#[test]
fn google_user_info_uses_the_quiet_helper() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http.rs");
    let source = fs::read_to_string(&path).expect("http.rs is readable");
    let body = fn_body(&source, "pub async fn fetch_user_info");
    assert!(
        body.contains("get_json_quiet("),
        "fetch_user_info must use get_json_quiet; get_json quotes error bodies"
    );
}
