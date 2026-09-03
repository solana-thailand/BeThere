//! Plan 020 — SQL-injection regression guard for `worker/src`.
//!
//! Plan 020 converted the worker's D1 layer from hand-escaped string
//! interpolation to bound parameters (`bind_refs` + `D1Type`). Two escapers
//! were found broken along the way (§4.1, §4.2). Nothing stops a future change
//! from reintroducing the pattern — `format!("… WHERE id = '{id}'")` compiles,
//! passes clippy, and reads like every other query in the file.
//!
//! This test is the mechanical check. It lexes every `.rs` file under
//! `worker/src`, collects the string literals that are SQL, and fails if any of
//! them contains a `{…}` interpolation that is not on [`ALLOWED_INTERPOLATIONS`].
//!
//! ## Why an allowlist rather than a ban
//!
//! A handful of interpolations are unavoidable and safe:
//!
//! - SQLite cannot parameterise an **identifier** (a column name), so
//!   `SELECT * FROM events WHERE {column} = ?` has no bound equivalent.
//! - It cannot parameterise a **predicate fragment** either, so the shared
//!   `COUNT(*) … AND ({predicate})` helpers interpolate their condition.
//! - `D1Type::Integer` is `i32`-only, so a `u64` timestamp is interpolated
//!   rather than bound.
//!
//! Each of those is safe *because of the argument's type*, not because of the
//! call sites: `&'static str` admits only compile-time literals, and an integer
//! cannot carry SQL. Every allowlist row records which of those two arguments
//! applies. A new interpolation that does not fit either shape fails the test
//! and has to be justified here, in review, rather than slipping in unnoticed.
//!
//! ## What this guard does NOT prove
//!
//! It proves nothing about values that are bound — a bound parameter is safe by
//! construction. It also cannot see interpolation built up outside a SQL-shaped
//! literal (e.g. a `String` assembled from pieces and only later concatenated
//! into a query). That shape does not exist in the tree today; if it is ever
//! introduced, this guard will not catch it, and the reviewer is the backstop.
//!
//! ## Run
//!
//! ```sh
//! cargo test -p event-checkin-worker --test sql_interpolation_guard
//! ```

use std::fs;
use std::path::{Path, PathBuf};

/// Root of the worker crate (`CARGO_MANIFEST_DIR` is `worker/` for this test).
const WORKER_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// One sanctioned interpolation site: a file plus the placeholder names that
/// may appear inside a SQL literal there.
struct AllowedInterpolation {
    /// Path relative to `worker/`, using `/` separators.
    file: &'static str,
    /// Placeholder names, as they appear between the braces. `<positional>` is
    /// the marker for an unnamed `{}` / `{0}`.
    placeholders: &'static [&'static str],
    /// Why interpolation is safe for every placeholder listed. Must name the
    /// property that makes it safe (the argument's type), not merely assert
    /// that today's callers behave.
    reason: &'static str,
}

/// The complete set of SQL interpolations sanctioned in `worker/src`.
///
/// Adding a row is a deliberate act: it means "SQLite cannot bind this, and the
/// argument's *type* makes injection impossible". If neither half is true, bind
/// the value instead of extending this list.
const ALLOWED_INTERPOLATIONS: &[AllowedInterpolation] = &[
    // -- Identifiers: SQLite cannot parameterise a column name. -------------
    AllowedInterpolation {
        file: "src/db/events.rs",
        placeholders: &["column"],
        reason: "`get_event_raw(column: &'static str)` — an identifier cannot be \
                 bound; `&'static str` admits only compile-time literals.",
    },
    AllowedInterpolation {
        file: "src/db/attendees/reads.rs",
        placeholders: &["column"],
        reason: "`count_by_status(column: &'static str)` — identifier; the type \
                 admits only compile-time literals.",
    },
    AllowedInterpolation {
        file: "src/db/developers.rs",
        placeholders: &["field_name"],
        reason: "`upsert_developer_field` resolves the wire-supplied name through \
                 `resolve_profile_column`, which returns `Option<&'static str>` \
                 drawn from `UPSERTABLE_PROFILE_COLUMNS`. Only that `&'static str` \
                 is in scope at the `format!`.",
    },
    // -- Predicate / clause fragments: also unbindable. ----------------------
    AllowedInterpolation {
        file: "src/db/dashboard.rs",
        placeholders: &["predicate", "IN_PERSON_PREDICATE"],
        reason: "`count_attendees_by_predicate(predicate: &'static str)` plus the \
                 module `const IN_PERSON_PREDICATE: &'static str` — a predicate \
                 cannot be bound and neither value can originate off the wire.",
    },
    AllowedInterpolation {
        file: "src/db/event_summaries.rs",
        placeholders: &["predicate"],
        reason: "`thb_count_predicate` / `thb_count_and_sum_predicate` take \
                 `predicate: &'static str` — same shape as `dashboard.rs`.",
    },
    AllowedInterpolation {
        file: "src/db/campaigns/crud.rs",
        placeholders: &["<positional>", "where_clause"],
        reason: "`list_campaigns` joins `&'static str` clause fragments that are \
                 pushed alongside their `D1Type` args; every value is bound.",
    },
    AllowedInterpolation {
        file: "src/db/contacts.rs",
        placeholders: &["<positional>", "in_person_case", "where_clause"],
        reason: "`WHERE a.event_id IN (…)` is a join of generated `?N` markers \
                 whose values are all bound; `in_person_case` is a `&'static str` \
                 literal and `where_clause` is built only from those two.",
    },
    // -- Integers and bools: a number cannot carry SQL. ---------------------
    AllowedInterpolation {
        file: "src/db/events.rs",
        placeholders: &[
            "capacity",
            "deposit_amount_thb",
            "deposit_amount_usdc",
            "deposit_deadline_hours",
            "deposit_enabled",
            "dev_profile_enabled",
            "event_end_ms",
            "event_start_ms",
            "in_person_capacity",
            "max_refundable_deposits",
            "on_chain_event_id",
            "online_capacity",
            "online_registration_open",
            "open_val",
            "quiz_enabled",
            "recap_published",
            "refund_deadline_hours",
            "require_contact_info",
            "require_photo_consent",
            "time_tba",
            "total_attendees",
            "until_sql",
            "value",
        ],
        reason: "Every one is an integer or bool off a typed `EventConfig` field \
                 (`until_sql` is `format!(\"{ms}\")` over an `Option<i64>`, else the \
                 literal `NULL`). They are interpolated only because \
                 `D1Type::Integer` is `i32`-only and these are `u64`/`u32`/`bool`.",
    },
    AllowedInterpolation {
        file: "src/db/event_summaries.rs",
        placeholders: &[
            "checked_in",
            "claimed",
            "deposited",
            "end_ms",
            "in_person_chk",
            "in_person_reg",
            "no_show",
            "post_event_reg",
            "refunded",
            "registered",
            "start_ms",
            "thb_dep",
            "thb_ref",
            "usdc_dep",
            "usdc_ref",
        ],
        reason: "`u64` counts and millisecond timestamps off a typed summary \
                 struct; integers cannot inject and `D1Type::Integer` is `i32`-only.",
    },
    AllowedInterpolation {
        file: "src/db/campaigns/events.rs",
        placeholders: &["is_required", "sequence_order"],
        reason: "`i64` / bool arguments — an integer cannot carry SQL.",
    },
    AllowedInterpolation {
        file: "src/db/campaigns/progress.rs",
        placeholders: &[
            "completed_at_expr",
            "events_completed",
            "is_complete",
            "total_required",
        ],
        reason: "Three `i64` counters plus `completed_at_expr`, which is one of two \
                 `&'static str` literals (`datetime('now')` / `NULL`) chosen by a \
                 comparison on `is_complete`.",
    },
    AllowedInterpolation {
        file: "src/db/attendees/management.rs",
        placeholders: &["consent"],
        reason: "`set_marketing_consent(consent: bool)` renders as SQLite's \
                 `TRUE`/`FALSE` keyword; a bool cannot inject.",
    },
    AllowedInterpolation {
        file: "src/db/developers.rs",
        placeholders: &["limit", "offset"],
        reason: "`usize` pagination arguments — integers cannot inject, and SQLite \
                 will not bind inside `LIMIT`/`OFFSET` on this driver.",
    },
    AllowedInterpolation {
        file: "src/db/onchain_events.rs",
        placeholders: &["days"],
        reason: "`cleanup_old_dedup_entries(days: i64)` — an integer, and SQLite \
                 cannot bind inside a `datetime()` modifier string.",
    },
    AllowedInterpolation {
        file: "src/db/jwt_blacklist.rs",
        placeholders: &["expires_at"],
        reason: "`insert(expires_at: u64)` — a Unix timestamp, interpolated only \
                 because `D1Type::Integer` is `i32`-only. Integers cannot inject.",
    },
    AllowedInterpolation {
        file: "src/handlers/profile.rs",
        placeholders: &["consent_val"],
        reason: "`if body.consent_outreach { 1 } else { 0 }` — an integer literal \
                 derived from a bool.",
    },
];

/// Keywords that mark a string literal as SQL. Matched against the literal's
/// uppercased, left-trimmed start, so a comment or a message that merely
/// mentions "select" is not picked up.
///
/// `WHERE` and `AND` are included because the tree builds clause *fragments*
/// (`format!("WHERE {}", …)`) that are later concatenated into a statement —
/// a fragment is exactly as injectable as a whole statement.
const SQL_PREFIXES: &[&str] = &[
    "SELECT ",
    "INSERT ",
    "UPDATE ",
    "DELETE ",
    "REPLACE INTO ",
    "WITH ",
    "WHERE ",
];

#[test]
fn no_unsanctioned_sql_interpolation_in_worker_src() {
    let src = PathBuf::from(WORKER_ROOT).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    assert!(
        files.len() > 50,
        "expected to scan the whole worker source tree, found only {} files under {} — \
         the walker is broken and this guard would pass vacuously",
        files.len(),
        src.display()
    );

    let mut violations = Vec::new();
    let mut matched = vec![false; ALLOWED_INTERPOLATIONS.len()];

    for file in &files {
        let rel = relative_path(file);
        let source = fs::read_to_string(file).expect("read worker source file");
        for literal in string_literals(&source) {
            if !is_sql_literal(&literal) {
                continue;
            }
            for placeholder in placeholders(&literal) {
                match ALLOWED_INTERPOLATIONS
                    .iter()
                    .position(|a| a.file == rel && a.placeholders.contains(&placeholder.as_str()))
                {
                    Some(idx) => matched[idx] = true,
                    None => violations.push(format!(
                        "{rel}: SQL literal interpolates `{{{placeholder}}}`\n    literal: {}",
                        truncate(&literal)
                    )),
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "unsanctioned SQL interpolation ({} site(s)).\n\n{}\n\nBind the value with \
         `bind_refs(&[D1Type::…])` instead. If SQLite genuinely cannot bind it (an \
         identifier or a predicate fragment), make the argument `&'static str` or an \
         integer and add a row to `ALLOWED_INTERPOLATIONS` in {}.",
        violations.len(),
        violations.join("\n"),
        file!()
    );

    let stale: Vec<&str> = matched
        .iter()
        .zip(ALLOWED_INTERPOLATIONS)
        .filter(|(seen, _)| !**seen)
        .map(|(_, a)| a.file)
        .collect();
    assert!(
        stale.is_empty(),
        "ALLOWED_INTERPOLATIONS has {} row(s) that no longer match any SQL literal: {stale:?}.\n\
         The interpolation was removed (good — delete the row) or the file moved.\n\
         Stale rows are load-bearing: they silently widen the allowlist for whatever \
         lands at that path next.",
        stale.len()
    );
}

/// Every allowlist row must name a type-level reason, not a call-site habit.
///
/// Guards the failure mode where a row is added with a reason like "callers only
/// pass constants" — true today, unenforced tomorrow.
#[test]
fn allowlist_reasons_cite_a_type_level_guarantee() {
    for entry in ALLOWED_INTERPOLATIONS {
        assert!(
            !entry.placeholders.is_empty(),
            "{}: a row with no placeholders can never match and only clutters the \
             allowlist",
            entry.file
        );
        let cites_type = entry.reason.contains("&'static str")
            || entry.reason.contains("integer")
            || entry.reason.contains("Integer")
            || entry.reason.contains("i64")
            || entry.reason.contains("u64")
            || entry.reason.contains("bool");
        assert!(
            cites_type,
            "{} {:?} — reason must cite the argument type that makes injection \
             impossible (`&'static str`, or an integer/bool), not the current \
             callers. Got: {}",
            entry.file, entry.placeholders, entry.reason
        );
    }
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    let mut paths: Vec<PathBuf> = entries.map(|e| e.expect("dir entry").path()).collect();
    paths.sort();
    for path in paths {
        match path.is_dir() {
            true => collect_rs_files(&path, out),
            false => {
                if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
    }
}

fn relative_path(path: &Path) -> String {
    path.strip_prefix(WORKER_ROOT)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Extract the contents of every string literal in a Rust source file.
///
/// A hand-rolled lexer rather than a regex because the distinction that matters
/// is *string vs comment*: `//! format!("SELECT … {x}")` in a doc comment must
/// not be treated as code, and `"https://…"` must not be treated as a comment.
/// Handles line/block comments, escapes, char literals, and raw strings
/// (`r"…"`, `r#"…"#`). Byte strings are irrelevant here and fall out as
/// ordinary strings.
fn string_literals(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        // Line comment.
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Block comment (nesting is legal in Rust).
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let mut depth = 1usize;
            i += 2;
            while i < bytes.len() && depth > 0 {
                match (bytes[i], bytes.get(i + 1)) {
                    (b'/', Some(&b'*')) => {
                        depth += 1;
                        i += 2;
                    }
                    (b'*', Some(&b'/')) => {
                        depth -= 1;
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
            continue;
        }
        // Raw string: `r` / `br` followed by zero or more `#` then `"`.
        if bytes[i] == b'r' || (bytes[i] == b'b' && bytes.get(i + 1) == Some(&b'r')) {
            // Only a raw-string prefix if the preceding byte cannot continue an
            // identifier (otherwise `for` / `char` would trip it).
            let prev_is_ident =
                i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            let mut j = i + if bytes[i] == b'b' { 2 } else { 1 };
            let hash_start = j;
            while j < bytes.len() && bytes[j] == b'#' {
                j += 1;
            }
            if !prev_is_ident && bytes.get(j) == Some(&b'"') {
                let hashes = j - hash_start;
                let close = format!("\"{}", "#".repeat(hashes));
                let body_start = j + 1;
                let rest = &source[body_start..];
                let end = rest
                    .find(&close)
                    .map(|p| body_start + p)
                    .unwrap_or(bytes.len());
                out.push(source[body_start..end].to_string());
                i = end + close.len();
                continue;
            }
        }
        // Char literal — skipped so `'"'` does not open a string.
        if bytes[i] == b'\'' {
            let mut j = i + 1;
            if bytes.get(j) == Some(&b'\\') {
                j += 2;
            } else {
                j += 1;
            }
            if bytes.get(j) == Some(&b'\'') {
                i = j + 1;
                continue;
            }
            // A lifetime (`'a`), not a char literal.
            i += 1;
            continue;
        }
        // Ordinary string literal.
        if bytes[i] == b'"' {
            let mut body = String::new();
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'"' {
                if bytes[j] == b'\\' {
                    // `\` + newline is Rust's line continuation: the newline and
                    // the following indentation vanish, which is exactly how the
                    // multi-line SQL in this tree is written.
                    match bytes.get(j + 1) {
                        Some(b'\n') => {
                            j += 2;
                            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                                j += 1;
                            }
                            continue;
                        }
                        _ => {
                            j += 2;
                            continue;
                        }
                    }
                }
                body.push(source[j..].chars().next().expect("valid utf-8 boundary"));
                j += source[j..].chars().next().map(char::len_utf8).unwrap_or(1);
            }
            out.push(body);
            i = j + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// A literal is SQL if, ignoring leading whitespace, it starts with a SQL
/// keyword. Deliberately narrow: an error message like
/// `"D1 update_campaign_status bind: …"` mentions a statement but does not
/// start with one, so it is not scanned.
fn is_sql_literal(literal: &str) -> bool {
    let upper = literal.trim_start().to_ascii_uppercase();
    SQL_PREFIXES.iter().any(|kw| upper.starts_with(kw))
}

/// Placeholder names inside a format string. `{{` is an escaped brace and is
/// skipped; a positional `{}` / `{0}` yields the marker `<positional>` so it can
/// never accidentally match a named allowlist row.
fn placeholders(literal: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = literal.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '{' {
            i += 1;
            continue;
        }
        if chars.get(i + 1) == Some(&'{') {
            i += 2;
            continue;
        }
        let mut j = i + 1;
        let mut name = String::new();
        while j < chars.len() && chars[j] != '}' {
            name.push(chars[j]);
            j += 1;
        }
        // Strip a format spec (`{x:?}`) down to the bare name.
        let name = name.split(':').next().unwrap_or("").trim().to_string();
        let is_named = !name.is_empty() && name.chars().next().is_some_and(|c| c.is_alphabetic());
        out.push(match is_named {
            true => name,
            false => "<positional>".to_string(),
        });
        i = j + 1;
    }
    out
}

fn truncate(literal: &str) -> String {
    let one_line: String = literal.split_whitespace().collect::<Vec<_>>().join(" ");
    match one_line.len() > 120 {
        true => format!("{}…", &one_line[..120]),
        false => one_line,
    }
}
