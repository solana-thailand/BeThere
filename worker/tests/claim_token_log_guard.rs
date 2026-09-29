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
    let start = source
        .find("async fn exchange_github_code")
        .expect("exchange_github_code moved; update this guard");
    let end = source[start..]
        .find("\n}\n")
        .map(|offset| start + offset)
        .expect("end of exchange_github_code");
    let body = &source[start..end];
    assert!(
        !body.contains("{text}") && !body.contains("{text:"),
        "exchange_github_code interpolates the raw token response into an error"
    );
}
