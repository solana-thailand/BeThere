//! No SQL write may set a `NOT NULL` column to `NULL`.
//!
//! SQLite aborts the entire statement when it does, and every site in this
//! codebase that got it wrong swallowed the resulting error, so the write
//! simply never happened and nothing surfaced. That has now bitten four times:
//!
//! - `clear_attendee_pii` and `clear_developer_pii` (plan 020 §6.2) — a PDPA
//!   erasure reported `200 {"status":"completed"}` while the PII sat untouched.
//! - `clear_contact_pii` — the same abort, missed by that fix.
//! - `finalize_claim_lock` — `claim_locks.expires_at` is `TEXT NOT NULL` and
//!   both the D1 and the Durable Object copy set it to `NULL`, so no completed
//!   mint was ever recorded and the attendee lost their proof link.
//!
//! Reading the code cannot catch this: the statement and the schema live in
//! different files and different languages. So this guard reads both.
//!
//! ## Layer 1 — the real schema against the real source
//!
//! Parses every `CREATE TABLE` / `ALTER TABLE … ADD COLUMN` in
//! `worker/migrations/*.sql` *and* in the Durable Object's `schema.rs` into a
//! table → `NOT NULL` column map, then scans every `UPDATE … SET` in
//! `worker/src` for an assignment of `NULL` to one of those columns.
//!
//! ## Layer 2 — self-test
//!
//! Runs the same detector over a synthetic schema and statement, so a parser
//! that silently stops matching fails here rather than going quietly green.
//!
//! ## Known limits
//!
//! Only `UPDATE … SET` is checked — that is where all four real bugs lived. A
//! column-list `INSERT` that omits a `NOT NULL` column without a default is the
//! same class and is not covered.
//!
//! ## Run
//!
//! ```sh
//! cargo test -p event-checkin-worker --test not_null_column_guard
//! ```

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Schema sources: the D1 migrations and the DO's inline `CREATE TABLE`s.
const MIGRATIONS_DIR: &str = "migrations";
const DO_SCHEMA_RS: &str = "src/durable_objects/event_do/schema.rs";
/// Root of the source scanned for offending statements.
const SRC_DIR: &str = "src";

// ---------------------------------------------------------------------------
// Normalisation
// ---------------------------------------------------------------------------

/// Join Rust string-literal line continuations (`\` + newline + indent) and
/// collapse every whitespace run to a single space, so a statement split over
/// six source lines reads as one.
/// Drop `--` (SQL) and `//` (Rust) line comments. Both languages' comments here
/// discuss `NULL` and `NOT NULL` in prose, which would otherwise parse as code.
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| {
            let cut = [line.find("--"), line.find("//")]
                .into_iter()
                .flatten()
                .min();
            match cut {
                Some(i) => &line[..i],
                None => line,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize(src: &str) -> String {
    let src = strip_line_comments(src);
    let bytes: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            // A backslash immediately followed by a newline is a continuation.
            '\\' if bytes.get(i + 1).is_some_and(|c| *c == '\n') => {
                i += 2;
                while bytes.get(i).is_some_and(|c| *c == ' ' || *c == '\t') {
                    i += 1;
                }
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Case-insensitive search for `needle` in `hay` starting at `from`.
fn find_ci(hay: &str, needle: &str, from: usize) -> Option<usize> {
    let hay_lc = hay.to_lowercase();
    hay_lc[from..].find(needle).map(|p| p + from)
}

/// Read the identifier starting at `from`, skipping leading spaces, quotes and
/// backticks. Returns the identifier and the byte index just past it.
///
/// Byte-indexed, like `find_ci` — the migrations contain em dashes, so char
/// offsets and byte offsets are not interchangeable here.
fn read_ident(text: &str, from: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let mut i = from;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'"' | b'`' | b'\'') {
        i += 1;
    }
    let start = i;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    match i > start {
        true => Some((text[start..i].to_string(), i)),
        false => None,
    }
}

/// Extract the balanced-parenthesis body starting at the first `(` at or after
/// `from`. Returns the inner text and the byte index just past the closing `)`.
///
/// Scanning bytes is safe: every byte of a multi-byte UTF-8 sequence is >= 0x80,
/// so none can be mistaken for an ASCII parenthesis.
fn balanced_body(text: &str, from: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let open = (from..bytes.len()).find(|i| bytes[*i] == b'(')?;
    let mut depth = 0usize;
    for i in open..bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((text[open + 1..i].to_string(), i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Split on commas that sit at parenthesis depth zero.
fn split_top_level(body: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut cur = String::new();
    for c in body.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            ',' if depth == 0 => {
                parts.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// Keywords that begin a table-level constraint rather than a column.
const CONSTRAINT_KEYWORDS: &[&str] = &["primary", "unique", "foreign", "check", "constraint"];

/// table (lowercased) -> set of columns (lowercased) declared `NOT NULL`.
type NotNullMap = HashMap<String, HashSet<String>>;

/// Parse every `CREATE TABLE` and `ALTER TABLE … ADD COLUMN` in `text` into
/// `map`. Works on both `.sql` and normalized Rust because it keys off the SQL
/// keywords rather than the surrounding syntax.
fn collect_not_null(text: &str, map: &mut NotNullMap) {
    let norm = normalize(text);

    let mut at = 0;
    while let Some(pos) = find_ci(&norm, "create table", at) {
        at = pos + "create table".len();
        // Optional `IF NOT EXISTS`.
        // Only an `IF NOT EXISTS` sitting directly after `CREATE TABLE` belongs
        // to this statement; anything further along is a later one.
        let after = match find_ci(&norm, "if not exists", at) {
            Some(p) if norm[at..p].trim().is_empty() => p + "if not exists".len(),
            _ => at,
        };
        let Some((table, past_name)) = read_ident(&norm, after) else {
            continue;
        };
        let Some((body, past_body)) = balanced_body(&norm, past_name) else {
            continue;
        };
        let entry = map.entry(table.to_lowercase()).or_default();
        for part in split_top_level(&body) {
            let lc = part.to_lowercase();
            let Some((col, _)) = read_ident(&part, 0) else {
                continue;
            };
            if CONSTRAINT_KEYWORDS.contains(&col.to_lowercase().as_str()) {
                continue;
            }
            if lc.contains("not null") {
                entry.insert(col.to_lowercase());
            }
        }
        at = past_body;
    }

    let mut at = 0;
    while let Some(pos) = find_ci(&norm, "alter table", at) {
        at = pos + "alter table".len();
        let Some((table, past_name)) = read_ident(&norm, at) else {
            continue;
        };
        let Some(add) = find_ci(&norm, "add column", past_name) else {
            continue;
        };
        // Guard against running past this statement into the next one.
        if !norm[past_name..add].trim().is_empty() {
            continue;
        }
        let Some((col, past_col)) = read_ident(&norm, add + "add column".len()) else {
            continue;
        };
        let end = norm[past_col..]
            .find(';')
            .map(|p| p + past_col)
            .unwrap_or(norm.len());
        if norm[past_col..end].to_lowercase().contains("not null") {
            map.entry(table.to_lowercase())
                .or_default()
                .insert(col.to_lowercase());
        }
        at = end;
    }
}

// ---------------------------------------------------------------------------
// Statement scan
// ---------------------------------------------------------------------------

/// One `<column> = NULL` assignment found in an `UPDATE … SET`.
#[derive(Debug)]
struct NullAssignment {
    table: String,
    column: String,
}

/// Find every `UPDATE <table> SET … <col> = NULL` in `text`.
fn find_null_assignments(text: &str) -> Vec<NullAssignment> {
    let norm = normalize(text);
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(pos) = find_ci(&norm, "update ", at) {
        at = pos + "update ".len();
        let Some((table, past_name)) = read_ident(&norm, at) else {
            continue;
        };
        let Some(set) = find_ci(&norm, " set ", past_name) else {
            continue;
        };
        // `SET` must follow the table name directly, or this is prose rather
        // than a statement.
        if !norm[past_name..set].trim().is_empty() {
            continue;
        }
        let start = set + " set ".len();
        // The SET clause ends at WHERE, at the end of the string literal, or at
        // a statement separator — whichever comes first.
        let end = [
            find_ci(&norm, " where ", start),
            norm[start..].find('"').map(|p| p + start),
            norm[start..].find(';').map(|p| p + start),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(norm.len());

        let clause = &norm[start..end];
        for assign in clause.split(',') {
            let lc = assign.to_lowercase();
            let Some((_, eq)) = lc.split_once('=') else {
                continue;
            };
            if eq.trim() != "null" {
                continue;
            }
            if let Some((col, _)) = read_ident(assign.trim(), 0) {
                found.push(NullAssignment {
                    table: table.to_lowercase(),
                    column: col.to_lowercase(),
                });
            }
        }
        at = end;
    }
    found
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => rust_files(&path, out),
            false if path.extension().is_some_and(|e| e == "rs") => out.push(path),
            false => {}
        }
    }
}

fn load_schema() -> NotNullMap {
    let mut map = NotNullMap::new();

    let dir = Path::new(MIGRATIONS_DIR);
    assert!(
        dir.is_dir(),
        "{MIGRATIONS_DIR} not found — run from worker/"
    );
    let mut sql: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read migrations dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        .collect();
    sql.sort();
    for path in &sql {
        collect_not_null(&fs::read_to_string(path).expect("read migration"), &mut map);
    }

    let do_schema = fs::read_to_string(DO_SCHEMA_RS).expect("read DO schema");
    collect_not_null(&do_schema, &mut map);

    map
}

// ---------------------------------------------------------------------------
// Layer 1 — real schema, real source
// ---------------------------------------------------------------------------

#[test]
fn schema_parse_finds_the_known_not_null_columns() {
    let map = load_schema();

    // A sanity floor: if the parser breaks, these disappear and the scan below
    // goes vacuously green.
    let expected: &[(&str, &str)] = &[
        ("claim_locks", "expires_at"),
        ("claim_locks", "event_id"),
        ("claim_locks", "wallet"),
        ("contacts", "contact_channel"),
        ("contacts", "contact_handle"),
    ];
    for (table, column) in expected {
        let cols = map
            .get(*table)
            .unwrap_or_else(|| panic!("no schema parsed for table `{table}`"));
        assert!(
            cols.contains(*column),
            "`{table}.{column}` is NOT NULL in the schema but the parser missed it"
        );
    }

    assert!(
        map.len() > 10,
        "only {} tables parsed — the schema parser has stopped matching",
        map.len()
    );
}

#[test]
fn no_update_sets_a_not_null_column_to_null() {
    let map = load_schema();

    let mut files = Vec::new();
    rust_files(Path::new(SRC_DIR), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no sources found under {SRC_DIR}");

    let mut violations = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("read source");
        for found in find_null_assignments(&text) {
            let Some(cols) = map.get(&found.table) else {
                continue;
            };
            if cols.contains(&found.column) {
                violations.push(format!(
                    "{}: UPDATE {} SET {} = NULL — `{}.{}` is NOT NULL, so SQLite \
                     aborts the whole statement and the write silently does nothing",
                    path.display(),
                    found.table,
                    found.column,
                    found.table,
                    found.column
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "SQL writes null a NOT NULL column:\n  {}",
        violations.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Layer 2 — self-test
// ---------------------------------------------------------------------------

#[test]
fn detector_fires_on_a_synthetic_violation() {
    let schema = r#"
        CREATE TABLE IF NOT EXISTS widgets (
            id         TEXT NOT NULL,
            label      TEXT,
            expires_at TEXT NOT NULL,
            PRIMARY KEY (id)
        );
        ALTER TABLE widgets ADD COLUMN owner TEXT NOT NULL DEFAULT '';
    "#;
    let mut map = NotNullMap::new();
    collect_not_null(schema, &mut map);

    let cols = map.get("widgets").expect("widgets parsed");
    assert!(cols.contains("expires_at"), "column NOT NULL missed");
    assert!(cols.contains("owner"), "ADD COLUMN NOT NULL missed");
    assert!(!cols.contains("label"), "nullable column wrongly flagged");
    assert!(
        !cols.contains("primary"),
        "table constraint read as a column"
    );

    // The exact shape the real bug had, including the Rust line continuation.
    let source = "let stmt = db.prepare(\n    \"UPDATE widgets \\\n     \
                  SET label = ?1, expires_at = NULL \\\n     WHERE id = ?2\",\n);";
    let found = find_null_assignments(source);
    assert_eq!(found.len(), 1, "expected one hit, got {found:?}");
    assert_eq!(found[0].table, "widgets");
    assert_eq!(found[0].column, "expires_at");
    assert!(
        cols.contains(&found[0].column),
        "detector and schema must agree the assignment is a violation"
    );

    // A nullable target is not a violation.
    let benign = "\"UPDATE widgets SET label = NULL WHERE id = ?1\"";
    let found = find_null_assignments(benign);
    assert_eq!(found.len(), 1);
    assert!(
        !cols.contains(&found[0].column),
        "nulling a nullable column must not be flagged"
    );
}
