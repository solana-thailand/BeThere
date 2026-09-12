//! An upstream response must never reach an API error body.
//!
//! Issue 078, the response-body sibling of Issue 070. #070 cleaned the Worker's
//! *log* stream and explicitly could not reach anything outside it; the error
//! body is outside it. `AppError`'s `Display` renders operator detail — wrapped
//! upstream URLs, identifiers and verbatim response text — and that string used
//! to be serialized into the public JSON body of every failure, including the
//! unauthenticated `GET /api/claim/{token}`.
//!
//! `AppError::public_message` is now the only rendering allowed to leave the
//! Worker; `domain/tests/error_public_message.rs` pins what it returns. This
//! guard pins the *wiring*: there is exactly one place that builds an error
//! body, and it uses `public_message`.

use std::{fs, path::Path};

/// The single sanctioned error-body path.
const RESPONSE_MODULE: &str = "src/error.rs";

fn worker_src() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    let path = worker_src().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
}

fn rust_sources(path: &Path, output: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).expect("source directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        match path.is_dir() {
            true => rust_sources(&path, output),
            false => {
                if path.extension().is_some_and(|extension| extension == "rs") {
                    output.push(path);
                }
            }
        }
    }
}

#[test]
fn the_error_body_is_built_from_public_message() {
    let source = read(RESPONSE_MODULE);

    assert!(
        source.contains("self.0.public_message()"),
        "{RESPONSE_MODULE} must render the caller-facing body with \
         AppError::public_message"
    );
    assert!(
        source.contains("error: Some(public.into_owned())"),
        "the body's `error` field must carry the public rendering, not the \
         operator detail"
    );
}

#[test]
fn the_operator_detail_still_reaches_the_log() {
    let source = read(RESPONSE_MODULE);

    assert!(
        source.contains("error = %detail"),
        "the 5xx tracing event must keep the full Display rendering — a fix \
         that also drops the operator's detail has traded one problem for \
         another (Issue 078, Verification)"
    );
}

/// `ApiResponse` is the API envelope. Any hand-rolled construction that sets
/// `error:` is a second body path that will drift from `src/error.rs` — the
/// auth middleware had exactly that, and it bypassed the redaction entirely
/// until #078 routed it through `WorkerError`.
#[test]
fn no_handrolled_error_body_bypasses_the_central_path() {
    let mut sources = Vec::new();
    rust_sources(&worker_src().join("src"), &mut sources);

    let mut offenders = Vec::new();
    for path in &sources {
        let source = fs::read_to_string(path).expect("source file is readable");
        for (index, line) in source.lines().enumerate() {
            if !line.contains("ApiResponse") {
                continue;
            }
            // The envelope's own success constructors carry no error string.
            if !line.contains("ApiResponse::<()> {") && !line.contains("ApiResponse {") {
                continue;
            }
            let relative = path
                .strip_prefix(worker_src())
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            if relative == RESPONSE_MODULE {
                continue;
            }
            offenders.push(format!("{relative}:{}", index + 1));
        }
    }

    assert!(
        offenders.is_empty(),
        "error bodies must be produced by WorkerError in {RESPONSE_MODULE} so \
         they inherit the Issue 078 redaction; found hand-rolled envelopes at: \
         {offenders:?}"
    );
}

/// A regression net for the specific string that was observed leaking.
#[test]
fn no_source_interpolates_a_sheets_url_into_a_caller_facing_error() {
    let mut sources = Vec::new();
    rust_sources(&worker_src().join("src"), &mut sources);

    for path in &sources {
        let source = fs::read_to_string(path).expect("source file is readable");
        for (index, line) in source.lines().enumerate() {
            let interpolates_a_url = line.contains("AppError::")
                && line.contains("https://")
                && line.contains("format!");
            assert!(
                !interpolates_a_url,
                "{}:{} builds an AppError from a literal URL; the caller-facing \
                 half of that message is redacted, so the URL belongs on the \
                 tracing event instead",
                path.display(),
                index + 1
            );
        }
    }
}
