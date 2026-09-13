//! The capability-token replay window must not be opted out of silently.
//!
//! Issue 071. Claim, quiz and adventure tokens travel in the URL path, so
//! Cloudflare's platform request log records them; `ClaimTokenPolicy` bounds how
//! long a token lifted from that log stays replayable. Passing the policy is a
//! required parameter, so the compiler already stops a new resolution path from
//! *forgetting* it — but nothing stops one from passing
//! `ClaimTokenPolicy::unrestricted()`, which disables the window entirely.
//!
//! This guard pins the set of files allowed to do that. Adding one is a
//! deliberate act that has to be argued for in review, not a default.
//!
//! Companion to `claim_token_log_guard.rs` (tokens in the log stream) and
//! `log_pii_guard.rs` (identifiers in the log stream).

use std::{fs, path::Path};

/// Files permitted to bypass the replay window, and why.
///
/// - `claim/ttl.rs` defines `unrestricted()`, and its tests exercise it.
/// - `handlers/attendee/delete.rs` deletes an attendee *by* claim token; admin
///   deletion must find the row whatever its age. It grants no capability.
const ALLOWED_BYPASS_FILES: &[&str] = &["claim/ttl.rs", "handlers/attendee/delete.rs"];

fn rust_sources(path: &Path, output: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).expect("source directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        match path.is_dir() {
            true => rust_sources(&path, output),
            false => {
                if path.extension().is_some_and(|ext| ext == "rs") {
                    output.push(path);
                }
            }
        }
    }
}

fn worker_sources() -> Vec<std::path::PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&root, &mut sources);
    assert!(
        sources.len() > 50,
        "expected to scan the whole Worker source tree, found {} files — the \
         scan root is probably wrong, which would make this guard blind",
        sources.len()
    );
    sources
}

/// Strip `//`-style comments so a doc comment that merely *names*
/// `ClaimTokenPolicy::unrestricted` is not mistaken for a call to it. Without
/// this the guard fires on its own documentation, which trains people to widen
/// the allow-list for non-reasons.
fn without_line_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn relative(path: &Path) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    path.strip_prefix(&root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn replay_window_is_only_bypassed_where_allowed() {
    let offenders: Vec<String> = worker_sources()
        .into_iter()
        .filter(|path| {
            let rel = relative(path);
            !ALLOWED_BYPASS_FILES.iter().any(|a| rel == *a)
        })
        .filter(|path| {
            let body = fs::read_to_string(path).expect("source file is readable");
            without_line_comments(&body).contains("ClaimTokenPolicy::unrestricted")
        })
        .map(|path| relative(&path))
        .collect();

    assert!(
        offenders.is_empty(),
        "these files bypass the Issue 071 claim-token replay window with \
         ClaimTokenPolicy::unrestricted(): {offenders:?}. A resolution path that \
         grants a capability must use AppState::claim_token_policy() instead. If \
         a bypass is genuinely correct, add the file to ALLOWED_BYPASS_FILES with \
         a reason."
    );
}

/// Guard the guard: if `unrestricted` is ever renamed, the scan above silently
/// matches nothing and passes forever. Pin that the allow-list entries really
/// do contain what we claim, so a rename breaks this test loudly.
#[test]
fn the_allow_list_entries_actually_contain_a_bypass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for allowed in ALLOWED_BYPASS_FILES {
        let path = root.join(allowed);
        let body = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("allow-listed file {allowed} is unreadable: {e}"));
        let code = without_line_comments(&body);
        assert!(
            code.contains("ClaimTokenPolicy::unrestricted") || code.contains("fn unrestricted"),
            "{allowed} is allow-listed as a replay-window bypass but contains \
             none — either the bypass was removed (drop it from the list) or \
             `unrestricted` was renamed, which would make the scan above blind."
        );
    }
}
