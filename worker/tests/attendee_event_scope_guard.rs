//! Issue 153 — every by-id attendee write is scoped to its event.
//!
//! `attendees.id` is a global key. `resolve_event_with_access` authorizes the
//! *event*, not the id, so an `UPDATE attendees … WHERE id = ?` (or a `DELETE`)
//! with no `event_id` filter lets staff of one event write another event's row.
//! The by-id lookup is scoped too, but a write that relies on "the lookup ran
//! first" breaks the moment a new caller skips the lookup.
//!
//! This guard lexes every `.rs` string literal and every `.sql` file under
//! `worker/src`, keeps the ones that write `attendees`, and fails if a bare
//! `id = ?` predicate appears without `event_id = ?` beside it. Sanctioned
//! exceptions are listed in [`ALLOWED`] with the reason each is safe; an entry
//! that no longer matches anything fails too, so the list cannot rot.
//!
//! ```sh
//! cargo test -p event-checkin-worker --test attendee_event_scope_guard
//! ```

use std::fs;
use std::path::{Path, PathBuf};

const WORKER_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// A write that matches on `id` alone on purpose.
struct Allowed {
    /// Path relative to `worker/`, `/` separators.
    file: &'static str,
    /// Substring that identifies the statement within that file.
    marker: &'static str,
}

const ALLOWED: &[Allowed] = &[
    // PDPA erasure: the ids come from `get_attendees_by_email`, which is
    // cross-event by design — the data subject's rows in every event go.
    Allowed {
        file: "src/db/attendees/management.rs",
        marker: "name = '[DELETED]'",
    },
    // Claim-token repair: the ids are read from `attendees` in the same
    // function, so each one is the row it was read from.
    Allowed {
        file: "src/db/attendees/management.rs",
        marker: "SET claim_token = ?1, updated_at = datetime('now') WHERE id = ?2",
    },
    // Durable Object storage: one SQLite database per event instance, so an
    // id there is already event-scoped.
    Allowed {
        file: "src/durable_objects/event_do/checkin.rs",
        marker: "SET checked_in_at = ?1, checked_in_by = ?2, claim_token = ?3",
    },
    Allowed {
        file: "src/durable_objects/event_do/checkin.rs",
        marker: "SET checked_in_at = NULL, checked_in_by = NULL, claim_token = NULL",
    },
];

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => collect_files(&path, out),
            false => match path.extension().and_then(|e| e.to_str()) {
                Some("rs") | Some("sql") => out.push(path),
                _ => {}
            },
        }
    }
}

/// Plain `"…"` string literals in Rust source, with `\`-newline continuations
/// folded and `\n` escapes turned into spaces. Raw strings and comments are
/// not SQL in this crate, so a small lexer is enough.
fn rust_string_literals(src: &str) -> Vec<String> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'\'' if bytes.get(i + 2) == Some(&b'\'') => i += 3,
            b'"' => {
                let mut lit = String::new();
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    match bytes[i] {
                        b'\\' => {
                            match bytes.get(i + 1) {
                                Some(b'\n') => {
                                    i += 2;
                                    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                                        i += 1;
                                    }
                                    continue;
                                }
                                Some(b'n') | Some(b't') => lit.push(' '),
                                Some(&c) => lit.push(c as char),
                                None => {}
                            }
                            i += 2;
                        }
                        c => {
                            lit.push(c as char);
                            i += 1;
                        }
                    }
                }
                out.push(lit);
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

fn writes_attendees(sql: &str) -> bool {
    let upper = sql.to_ascii_uppercase();
    upper.contains("UPDATE ATTENDEES") || upper.contains("DELETE FROM ATTENDEES")
}

/// `id = ?` with no identifier character before `id` (so `event_id = ?` and
/// `claim_asset_id = ?` do not count).
fn has_bare_id_predicate(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    sql.match_indices("id = ?")
        .any(|(at, _)| at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_'))
}

fn is_unscoped_attendee_write(sql: &str) -> bool {
    writes_attendees(sql) && has_bare_id_predicate(sql) && !sql.contains("event_id = ?")
}

#[test]
fn every_by_id_attendee_write_is_event_scoped() {
    let root = Path::new(WORKER_ROOT);
    let mut files = Vec::new();
    collect_files(&root.join("src"), &mut files);
    files.sort();
    assert!(files.len() > 50, "walked too few files: {}", files.len());

    let mut violations = Vec::new();
    let mut allowed_hits = vec![0usize; ALLOWED.len()];
    let mut writes_seen = 0usize;

    for path in &files {
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let src = fs::read_to_string(path).unwrap();
        let statements = match path.extension().and_then(|e| e.to_str()) {
            Some("sql") => vec![src.split_whitespace().collect::<Vec<_>>().join(" ")],
            _ => rust_string_literals(&src),
        };
        for sql in statements.iter().filter(|s| writes_attendees(s)) {
            writes_seen += 1;
            if !is_unscoped_attendee_write(sql) {
                continue;
            }
            match ALLOWED
                .iter()
                .position(|a| a.file == rel && sql.contains(a.marker))
            {
                Some(idx) => allowed_hits[idx] += 1,
                None => violations.push(format!("{rel}: {sql}")),
            }
        }
    }

    assert!(
        writes_seen >= 10,
        "found only {writes_seen} attendee writes; the lexer is probably broken"
    );
    assert!(
        violations.is_empty(),
        "attendee writes matched on `id` alone (Issue 153) — add `AND event_id = ?`:\n{}",
        violations.join("\n")
    );
    let stale: Vec<_> = ALLOWED
        .iter()
        .zip(&allowed_hits)
        .filter(|(_, hits)| **hits == 0)
        .map(|(a, _)| format!("{}: {}", a.file, a.marker))
        .collect();
    assert!(
        stale.is_empty(),
        "allowlist entries match nothing; remove them:\n{}",
        stale.join("\n")
    );
}

#[test]
fn detector_flags_an_id_only_write() {
    assert!(is_unscoped_attendee_write(
        "UPDATE attendees SET qr_url = ?1 WHERE id = ?2"
    ));
    assert!(is_unscoped_attendee_write(
        "DELETE FROM attendees WHERE id = ?1"
    ));
}

#[test]
fn detector_accepts_scoped_and_unrelated_statements() {
    assert!(!is_unscoped_attendee_write(
        "UPDATE attendees SET qr_url = ?1 WHERE id = ?2 AND event_id = ?3"
    ));
    assert!(!is_unscoped_attendee_write(
        "UPDATE attendees SET claimed_at = ?1 WHERE claim_token = ?4"
    ));
    assert!(!is_unscoped_attendee_write(
        "UPDATE campaigns SET status = ? WHERE id = ?"
    ));
    assert!(!is_unscoped_attendee_write(
        "SELECT * FROM attendees WHERE id = ?1"
    ));
}

#[test]
fn lexer_folds_line_continuations() {
    let src = "let s = \"UPDATE attendees \\\n         WHERE id = ?1\";";
    assert_eq!(
        rust_string_literals(src),
        vec!["UPDATE attendees WHERE id = ?1".to_string()]
    );
}
