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
//! table → column map recording, per column, whether it is `NOT NULL`, whether
//! it has a `DEFAULT`, and whether it is the `INTEGER PRIMARY KEY` rowid alias.
//! It then scans every statement under `worker/src` for the three ways a write
//! can fall foul of that schema:
//!
//! 1. `UPDATE … SET <col> = NULL` on a `NOT NULL` column — the four real bugs.
//! 2. `INSERT … VALUES` putting a literal `NULL` in a `NOT NULL` column.
//! 3. A column-list `INSERT` that never names a `NOT NULL` column which has no
//!    `DEFAULT` to fall back on.
//!
//! `ON CONFLICT … DO UPDATE SET` is read as part of its `INSERT`, so the upsert
//! clause is covered by (1) as well — it is invisible to the `UPDATE` scan,
//! which needs an `UPDATE <table> SET` to find a table name.
//!
//! ## Layer 2 — self-test
//!
//! Runs the same detectors over a synthetic schema and statements, so a parser
//! that silently stops matching fails here rather than going quietly green. The
//! Layer 1 tests carry sanity floors for the same reason: a minimum table
//! count, a minimum number of `NOT NULL`-without-`DEFAULT` columns, and a
//! minimum number of parsed column-list `INSERT`s.
//!
//! ## Known limits
//!
//! - Only the first statement in a Rust string literal is read; the scan stops
//!   at the closing quote or the first `;`.
//! - `INSERT … SELECT` contributes no `VALUES` findings. Its column list is
//!   still checked for omissions.
//! - A `NOT NULL` column reachable only through a `format!` placeholder in the
//!   column list is invisible — the guard reads the literal, not the value.
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

/// What one column declaration says about whether a write may leave it unset.
#[derive(Debug, Default, Clone, Copy)]
struct ColumnFacts {
    /// Declared `NOT NULL`, so an explicit `NULL` write aborts the statement.
    not_null: bool,
    /// Has a `DEFAULT`, so omitting it from an `INSERT` column list is fine.
    has_default: bool,
    /// `INTEGER PRIMARY KEY` — the rowid alias, which SQLite fills in itself
    /// even though it is implicitly `NOT NULL`.
    integer_pk: bool,
}

impl ColumnFacts {
    /// Merge two declarations of the same column. The two schema sources (the
    /// D1 migrations and the Durable Object's inline `CREATE TABLE`s) can
    /// disagree; take `NOT NULL` from either, since a write has to satisfy
    /// whichever backend it lands on, but take an escape hatch from either too,
    /// so a disagreement can only ever weaken a claim, never invent one.
    fn merge(self, other: Self) -> Self {
        Self {
            not_null: self.not_null || other.not_null,
            has_default: self.has_default || other.has_default,
            integer_pk: self.integer_pk || other.integer_pk,
        }
    }

    /// An `INSERT` with a column list must name this column explicitly.
    fn required_on_insert(self) -> bool {
        self.not_null && !self.has_default && !self.integer_pk
    }
}

/// table (lowercased) -> column (lowercased) -> what its declaration says.
type SchemaMap = HashMap<String, HashMap<String, ColumnFacts>>;

/// The `NOT NULL` columns of `table`, or an empty set if it is unknown.
fn not_null_columns(map: &SchemaMap, table: &str) -> HashSet<String> {
    map.get(table)
        .map(|cols| {
            cols.iter()
                .filter(|(_, facts)| facts.not_null)
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Parse every `CREATE TABLE` and `ALTER TABLE … ADD COLUMN` in `text` into
/// `map`. Works on both `.sql` and normalized Rust because it keys off the SQL
/// keywords rather than the surrounding syntax.
fn collect_schema(text: &str, map: &mut SchemaMap) {
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
            let Some((col, past_col)) = read_ident(&part, 0) else {
                continue;
            };
            let col = col.to_lowercase();
            if CONSTRAINT_KEYWORDS.contains(&col.as_str()) {
                continue;
            }
            // Everything after the column name is its type and constraints.
            // Pad so ` default ` cannot match a bare suffix of a longer word.
            let rest = format!(" {} ", part[past_col..].to_lowercase());
            let facts = ColumnFacts {
                not_null: rest.contains(" not null"),
                has_default: rest.contains(" default "),
                integer_pk: rest.contains(" integer ") && rest.contains(" primary key"),
            };
            let merged = entry.get(&col).copied().unwrap_or_default().merge(facts);
            entry.insert(col, merged);
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
        let rest = format!(" {} ", norm[past_col..end].to_lowercase());
        let facts = ColumnFacts {
            not_null: rest.contains(" not null"),
            // SQLite refuses `ADD COLUMN … NOT NULL` without one, so this is
            // all but implied; read it anyway rather than assume.
            has_default: rest.contains(" default "),
            integer_pk: rest.contains(" integer ") && rest.contains(" primary key"),
        };
        let entry = map.entry(table.to_lowercase()).or_default();
        let col = col.to_lowercase();
        let merged = entry.get(&col).copied().unwrap_or_default().merge(facts);
        entry.insert(col, merged);
        at = end;
    }
}

// ---------------------------------------------------------------------------
// Statement scan
// ---------------------------------------------------------------------------

/// One way a statement can fall foul of a `NOT NULL` column.
#[derive(Debug, PartialEq, Eq)]
enum Offence {
    /// `<col> = NULL` where `<col>` is `NOT NULL`. SQLite aborts the statement.
    Nulled {
        table: String,
        column: String,
        /// Which clause it sits in, for the failure message.
        clause: &'static str,
    },
    /// A column-list `INSERT` that never names a `NOT NULL` column which has no
    /// `DEFAULT` to fall back on. SQLite aborts the statement just the same.
    Omitted { table: String, column: String },
}

impl Offence {
    fn table(&self) -> &str {
        match self {
            Self::Nulled { table, .. } | Self::Omitted { table, .. } => table,
        }
    }

    fn column(&self) -> &str {
        match self {
            Self::Nulled { column, .. } | Self::Omitted { column, .. } => column,
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Nulled {
                table,
                column,
                clause,
            } => format!("{clause} on `{table}` sets `{column}` = NULL"),
            Self::Omitted { table, column } => {
                format!("INSERT INTO `{table}` omits `{column}`, which has no DEFAULT")
            }
        }
    }
}

/// Columns a `SET` clause assigns a literal `NULL`.
///
/// Shared by `UPDATE … SET` and `ON CONFLICT … DO UPDATE SET`, which are the
/// same grammar and the same hazard.
fn null_targets_in_set_clause(clause: &str) -> Vec<String> {
    let mut cols = Vec::new();
    for assign in clause.split(',') {
        let Some((lhs, rhs)) = assign.split_once('=') else {
            continue;
        };
        if !rhs.trim().eq_ignore_ascii_case("null") {
            continue;
        }
        if let Some((col, _)) = read_ident(lhs.trim(), 0) {
            cols.push(col.to_lowercase());
        }
    }
    cols
}

/// Where a statement embedded in a Rust string literal stops: the closing
/// quote, or a `;`, whichever comes first.
fn statement_end(norm: &str, from: usize) -> usize {
    [norm[from..].find('"'), norm[from..].find(';')]
        .into_iter()
        .flatten()
        .min()
        .map(|p| p + from)
        .unwrap_or(norm.len())
}

/// Find every `UPDATE <table> SET … <col> = NULL` in `text`.
fn find_null_assignments(text: &str) -> Vec<Offence> {
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
        let end = find_ci(&norm, " where ", start)
            .into_iter()
            .chain([statement_end(&norm, start)])
            .min()
            .unwrap_or(norm.len());

        for column in null_targets_in_set_clause(&norm[start..end]) {
            found.push(Offence::Nulled {
                table: table.to_lowercase(),
                column,
                clause: "UPDATE … SET",
            });
        }
        at = end;
    }
    found
}

/// Skip whitespace and one optional leading comma, then read the parenthesised
/// tuple that must follow. Returns `None` if anything else is in the way, so a
/// `VALUES` scan stops at the end of the tuple list instead of running on into
/// the rest of the statement.
fn next_tuple(text: &str, from: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let mut i = from;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b',' {
        i += 1;
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
    }
    match bytes.get(i) {
        Some(b'(') => balanced_body(text, i),
        _ => None,
    }
}

/// One parsed `INSERT`, reduced to what bears on `NOT NULL`.
#[derive(Debug)]
struct InsertStatement {
    table: String,
    /// The explicit column list. `None` when the statement has none, in which
    /// case it names every column by position and nothing can be omitted.
    columns: Option<Vec<String>>,
    /// Columns a `VALUES` tuple assigns a literal `NULL`.
    null_values: Vec<String>,
    /// Columns an `ON CONFLICT … DO UPDATE SET` clause assigns `NULL`.
    null_upserts: Vec<String>,
}

/// Find every `INSERT [OR …] INTO <table> …` in `text`.
///
/// `INSERT … SELECT` parses fine — it simply contributes no `VALUES` findings,
/// while its column list is still checked for omissions.
fn find_inserts(text: &str) -> Vec<InsertStatement> {
    let norm = normalize(text);
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(pos) = find_ci(&norm, "insert", at) {
        at = pos + "insert".len();
        // `INSERT OR IGNORE INTO`, `INSERT OR REPLACE INTO`, plain `INSERT INTO`.
        let mut cursor = at;
        if let Some((word, past_or)) = read_ident(&norm, cursor)
            && word.eq_ignore_ascii_case("or")
            && let Some((_verb, past_verb)) = read_ident(&norm, past_or)
        {
            cursor = past_verb;
        }
        let Some(into) = find_ci(&norm, "into", cursor) else {
            continue;
        };
        // `INTO` must follow directly, or the word `insert` was prose.
        if !norm[cursor..into].trim().is_empty() {
            continue;
        }
        let Some((table, past_name)) = read_ident(&norm, into + "into".len()) else {
            continue;
        };
        let end = statement_end(&norm, past_name);
        at = end;

        let mut cursor = past_name;
        let mut columns = None;
        if norm[cursor..end].trim_start().starts_with('(')
            && let Some((body, past_cols)) = balanced_body(&norm[..end], cursor)
        {
            columns = Some(
                split_top_level(&body)
                    .iter()
                    .filter_map(|part| read_ident(part, 0).map(|(c, _)| c.to_lowercase()))
                    .collect::<Vec<_>>(),
            );
            cursor = past_cols;
        }

        // A literal `NULL` in a `VALUES` tuple is only attributable to a column
        // when the statement lists its columns.
        let mut null_values = Vec::new();
        if let Some(values) = find_ci(&norm[..end], "values", cursor)
            && norm[cursor..values].trim().is_empty()
        {
            let mut tuple_at = values + "values".len();
            while let Some((body, past_tuple)) = next_tuple(&norm[..end], tuple_at) {
                for (i, expr) in split_top_level(&body).iter().enumerate() {
                    if expr.trim().eq_ignore_ascii_case("null")
                        && let Some(column) = columns.as_ref().and_then(|c| c.get(i))
                    {
                        null_values.push(column.clone());
                    }
                }
                tuple_at = past_tuple;
            }
            cursor = tuple_at;
        }

        let mut null_upserts = Vec::new();
        if let Some(update) = find_ci(&norm[..end], "do update set ", cursor) {
            let start = update + "do update set ".len();
            let clause_end = find_ci(&norm[..end], " where ", start).unwrap_or(end);
            null_upserts = null_targets_in_set_clause(&norm[start..clause_end]);
        }

        found.push(InsertStatement {
            table: table.to_lowercase(),
            columns,
            null_values,
            null_upserts,
        });
    }
    found
}

/// Judge one parsed `INSERT` against the schema.
fn insert_offences(stmt: &InsertStatement, map: &SchemaMap) -> Vec<Offence> {
    let Some(declared) = map.get(&stmt.table) else {
        return Vec::new();
    };
    let mut offences = Vec::new();

    for column in stmt.null_values.iter().chain(&stmt.null_upserts) {
        let clause = match stmt.null_values.contains(column) {
            true => "INSERT … VALUES",
            false => "ON CONFLICT … DO UPDATE SET",
        };
        if declared.get(column).is_some_and(|f| f.not_null) {
            offences.push(Offence::Nulled {
                table: stmt.table.clone(),
                column: column.clone(),
                clause,
            });
        }
    }

    if let Some(listed) = &stmt.columns {
        let mut missing: Vec<&String> = declared
            .iter()
            .filter(|(name, facts)| facts.required_on_insert() && !listed.contains(name))
            .map(|(name, _)| name)
            .collect();
        missing.sort();
        for column in missing {
            offences.push(Offence::Omitted {
                table: stmt.table.clone(),
                column: column.clone(),
            });
        }
    }

    offences
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

fn load_schema() -> SchemaMap {
    let mut map = SchemaMap::new();

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
        collect_schema(&fs::read_to_string(path).expect("read migration"), &mut map);
    }

    let do_schema = fs::read_to_string(DO_SCHEMA_RS).expect("read DO schema");
    collect_schema(&do_schema, &mut map);

    map
}

// ---------------------------------------------------------------------------
// Layer 1 — real schema, real source
// ---------------------------------------------------------------------------

#[test]
fn schema_parse_finds_the_known_not_null_columns() {
    let map = load_schema();

    // A sanity floor: if the parser breaks, these disappear and the scans below
    // go vacuously green.
    let expected: &[(&str, &str)] = &[
        ("claim_locks", "expires_at"),
        ("claim_locks", "event_id"),
        ("claim_locks", "wallet"),
        ("contacts", "contact_channel"),
        ("contacts", "contact_handle"),
    ];
    for (table, column) in expected {
        assert!(
            not_null_columns(&map, table).contains(*column),
            "`{table}.{column}` is NOT NULL in the schema but the parser missed it"
        );
    }

    assert!(
        map.len() > 10,
        "only {} tables parsed — the schema parser has stopped matching",
        map.len()
    );

    // The omission check only bites on columns that are `NOT NULL` *and* have
    // no `DEFAULT`. If `has_default` detection ever over-matches, that set
    // empties out and `no_insert_leaves_a_not_null_column_unset` goes quiet.
    let required: usize = map
        .values()
        .flat_map(|cols| cols.values())
        .filter(|facts| facts.required_on_insert())
        .count();
    assert!(
        required > 50,
        "only {required} columns are NOT NULL without a DEFAULT — the DEFAULT \
         or INTEGER PRIMARY KEY detection has started over-matching"
    );
}

/// Scan every Rust source under `src` with `detect`, and report what it finds.
fn scan_sources(detect: impl Fn(&str) -> Vec<Offence>) -> Vec<String> {
    let mut files = Vec::new();
    rust_files(Path::new(SRC_DIR), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no sources found under {SRC_DIR}");

    let mut violations = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("read source");
        for offence in detect(&text) {
            violations.push(format!("{}: {}", path.display(), offence.describe()));
        }
    }
    violations
}

#[test]
fn no_update_sets_a_not_null_column_to_null() {
    let map = load_schema();
    let violations = scan_sources(|text| {
        find_null_assignments(text)
            .into_iter()
            .filter(|o| not_null_columns(&map, o.table()).contains(o.column()))
            .collect()
    });

    assert!(
        violations.is_empty(),
        "SQL writes null a NOT NULL column — SQLite aborts the whole statement, \
         so the write silently does nothing:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn no_insert_leaves_a_not_null_column_unset() {
    let map = load_schema();

    // Without this floor a broken `INSERT` parser passes by finding nothing.
    let mut with_columns = 0usize;
    let mut files = Vec::new();
    rust_files(Path::new(SRC_DIR), &mut files);
    for path in &files {
        let text = fs::read_to_string(path).expect("read source");
        with_columns += find_inserts(&text)
            .iter()
            .filter(|s| s.columns.is_some() && map.contains_key(&s.table))
            .count();
    }
    assert!(
        with_columns > 20,
        "only {with_columns} column-list INSERTs against known tables parsed — \
         the INSERT parser has stopped matching"
    );

    let violations = scan_sources(|text| {
        find_inserts(text)
            .iter()
            .flat_map(|stmt| insert_offences(stmt, &map))
            .collect()
    });

    assert!(
        violations.is_empty(),
        "INSERTs leave a NOT NULL column unset — SQLite aborts the whole \
         statement, so the row is never written:\n  {}",
        violations.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Layer 2 — self-test
// ---------------------------------------------------------------------------

/// The synthetic schema both self-tests run against.
const SYNTHETIC_SCHEMA: &str = r#"
    CREATE TABLE IF NOT EXISTS widgets (
        rowid      INTEGER PRIMARY KEY AUTOINCREMENT,
        id         TEXT NOT NULL,
        label      TEXT,
        state      TEXT NOT NULL DEFAULT 'new',
        expires_at TEXT NOT NULL,
        PRIMARY KEY (id)
    );
    ALTER TABLE widgets ADD COLUMN owner TEXT NOT NULL DEFAULT '';
"#;

#[test]
fn detector_fires_on_a_synthetic_violation() {
    let mut map = SchemaMap::new();
    collect_schema(SYNTHETIC_SCHEMA, &mut map);

    let cols = not_null_columns(&map, "widgets");
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
    assert_eq!(found[0].table(), "widgets");
    assert_eq!(found[0].column(), "expires_at");
    assert!(
        cols.contains(found[0].column()),
        "detector and schema must agree the assignment is a violation"
    );

    // A nullable target is not a violation.
    let benign = "\"UPDATE widgets SET label = NULL WHERE id = ?1\"";
    let found = find_null_assignments(benign);
    assert_eq!(found.len(), 1);
    assert!(
        !cols.contains(found[0].column()),
        "nulling a nullable column must not be flagged"
    );
}

#[test]
fn insert_detector_fires_on_a_synthetic_violation() {
    let mut map = SchemaMap::new();
    collect_schema(SYNTHETIC_SCHEMA, &mut map);

    let facts = &map["widgets"];
    assert!(
        facts["expires_at"].required_on_insert(),
        "NOT NULL without a DEFAULT must be required"
    );
    assert!(
        !facts["state"].required_on_insert(),
        "NOT NULL with a DEFAULT must not be required"
    );
    assert!(
        !facts["rowid"].required_on_insert(),
        "INTEGER PRIMARY KEY is filled in by SQLite"
    );
    assert!(
        !facts["label"].required_on_insert(),
        "nullable is not required"
    );

    // Omits `expires_at` entirely, and nulls it in the upsert clause too.
    let offending = "\"INSERT OR IGNORE INTO widgets (id, label) VALUES (?1, NULL) \\\n         \
                     ON CONFLICT (id) DO UPDATE SET label = excluded.label\"";
    let stmts = find_inserts(offending);
    assert_eq!(stmts.len(), 1, "expected one INSERT, got {stmts:?}");
    assert_eq!(stmts[0].table, "widgets");
    assert_eq!(
        stmts[0].columns.as_deref(),
        Some(["id".to_string(), "label".to_string()].as_slice())
    );
    // `label` is nullable, so the literal NULL is recorded but not an offence.
    assert_eq!(stmts[0].null_values, ["label"]);
    let offences = insert_offences(&stmts[0], &map);
    assert_eq!(
        offences,
        [Offence::Omitted {
            table: "widgets".to_string(),
            column: "expires_at".to_string(),
        }],
        "expected only the omitted NOT NULL column"
    );

    // Naming every required column, with a real value, is clean.
    let clean = "\"INSERT INTO widgets (id, expires_at, label) VALUES (?1, ?2, NULL)\"";
    let stmts = find_inserts(clean);
    assert_eq!(stmts.len(), 1);
    assert!(
        insert_offences(&stmts[0], &map).is_empty(),
        "a complete INSERT must not be flagged"
    );

    // Nulling a NOT NULL column positionally is caught even though it is named.
    let nulled = "\"INSERT INTO widgets (id, expires_at) VALUES (?1, NULL)\"";
    let stmts = find_inserts(nulled);
    assert_eq!(
        insert_offences(&stmts[0], &map),
        [Offence::Nulled {
            table: "widgets".to_string(),
            column: "expires_at".to_string(),
            clause: "INSERT … VALUES",
        }]
    );

    // So is nulling one in the upsert clause.
    let upsert = "\"INSERT INTO widgets (id, expires_at) VALUES (?1, ?2) \
                  ON CONFLICT (id) DO UPDATE SET expires_at = NULL\"";
    let stmts = find_inserts(upsert);
    assert_eq!(stmts[0].null_upserts, ["expires_at"]);
    assert_eq!(
        insert_offences(&stmts[0], &map),
        [Offence::Nulled {
            table: "widgets".to_string(),
            column: "expires_at".to_string(),
            clause: "ON CONFLICT … DO UPDATE SET",
        }]
    );

    // Prose containing the word must not parse as a statement.
    assert!(
        find_inserts("// we insert the widget into the queue here").is_empty(),
        "prose parsed as an INSERT"
    );
}
