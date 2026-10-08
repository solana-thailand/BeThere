//! The booking display code (`.issues/178`, migration 0058).
//!
//! Two halves:
//!
//! - the migration is executed against a real SQLite database built from the
//!   real migration chain (`sqlite3` as a subprocess, as in
//!   `claim_locks_backfill_migration.rs`; skipped when absent, while the static
//!   checks still run), including a forced-collision run that proves the
//!   unique index cannot fail to build;
//! - source guards pin the wiring: every D1 attendee writer assigns a code,
//!   the staff lookup is in the authed router and authorizes the event before
//!   touching D1, every query is event-scoped, and the code never reaches the
//!   claim or deposit handlers.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use event_checkin_domain::models::attendee::{DISPLAY_CODE_ALPHABET, DISPLAY_CODE_LEN};

const MIGRATION: &str = "0058_attendee_display_code.sql";

fn worker_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(worker_root().join(rel))
        .unwrap_or_else(|e| panic!("{rel} is unreadable: {e} — was it moved?"))
}

/// Source with `//` comments dropped, so prose is not read as code.
fn code(rel: &str) -> String {
    read(rel)
        .lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn migration_sql() -> String {
    read(&format!("migrations/{MIGRATION}"))
}

// ---------------------------------------------------------------------------
// SQLite harness
// ---------------------------------------------------------------------------

fn sqlite3_available() -> bool {
    Command::new("sqlite3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `sql` against `db`; `Ok(stdout)` or `Err(stderr)`.
fn sqlite_try(db: &Path, sql: &str) -> Result<String, String> {
    let mut child = Command::new("sqlite3")
        .arg(db)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn sqlite3");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(sql.as_bytes())
        .expect("write sql");
    let out = child.wait_with_output().expect("sqlite3 output");
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    match out.status.success() && stderr.is_empty() {
        true => Ok(String::from_utf8_lossy(&out.stdout).trim().to_string()),
        false => Err(stderr),
    }
}

fn sqlite(db: &Path, sql: &str) -> String {
    sqlite_try(db, sql).unwrap_or_else(|e| panic!("sqlite3 rejected:\n{e}\n--- sql ---\n{sql}"))
}

/// A database with every migration before 0058 applied.
fn pre_migration_db(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("bethere_display_code_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmpdir");
    let db = dir.join("test.db");
    let _ = std::fs::remove_file(&db);
    let mut files: Vec<_> = std::fs::read_dir(worker_root().join("migrations"))
        .expect("migrations dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().as_ref() < MIGRATION)
        })
        .collect();
    files.sort();
    assert!(
        files.len() > 50,
        "expected the full migration chain, found {} files",
        files.len()
    );
    for f in files {
        sqlite(&db, &std::fs::read_to_string(&f).expect("read migration"));
    }
    db
}

/// `n` attendees in each of two events.
fn seed_two_events(db: &Path, n: usize) {
    let mut sql = String::from("BEGIN;");
    for event in ["evt-a", "evt-b"] {
        for i in 0..n {
            sql.push_str(&format!(
                "INSERT INTO attendees (id, event_id, email) \
                 VALUES ('{event}-{i}', '{event}', '{event}-{i}@example.com');"
            ));
        }
    }
    sql.push_str("COMMIT;");
    sqlite(db, &sql);
}

fn assert_code_shape(code: &str) {
    assert_eq!(code.len(), DISPLAY_CODE_LEN, "{code:?}");
    assert!(
        code.bytes().all(|b| DISPLAY_CODE_ALPHABET.contains(&b)),
        "{code:?} uses a character outside the alphabet"
    );
}

/// The lookup statement, copied from the source so the test runs what ships.
fn lookup_sql() -> &'static str {
    "SELECT id FROM attendees WHERE event_id = ?1 AND display_code = ?2"
}

// ---------------------------------------------------------------------------
// Migration, executed
// ---------------------------------------------------------------------------

#[test]
fn the_backfill_gives_every_row_a_valid_code_unique_per_event() {
    if !sqlite3_available() {
        eprintln!("sqlite3 not on PATH — skipping the executed half of this test");
        return;
    }
    let db = pre_migration_db("backfill");
    seed_two_events(&db, 300);
    sqlite(&db, &migration_sql());

    let nulls = sqlite(
        &db,
        "SELECT COUNT(*) FROM attendees WHERE display_code IS NULL;",
    );
    // NULL is allowed by design (last-resort pass), but at 300 rows a
    // surviving collision after two redraws is ~1e-12; 0 is the expectation.
    assert_eq!(nulls, "0", "rows left without a code");

    let codes = sqlite(&db, "SELECT display_code FROM attendees;");
    for code in codes.lines() {
        assert_code_shape(code);
    }
    let dups = sqlite(
        &db,
        "SELECT COUNT(*) FROM (SELECT event_id, display_code FROM attendees \
         GROUP BY event_id, display_code HAVING COUNT(*) > 1);",
    );
    assert_eq!(dups, "0");

    // The helper index is gone; the partial unique index is there.
    let indexes = sqlite(
        &db,
        "SELECT name || '|' || \"unique\" || '|' || partial FROM pragma_index_list('attendees') \
         WHERE name LIKE '%display_code%';",
    );
    assert_eq!(indexes, "idx_attendees_event_display_code|1|1");
}

#[test]
fn the_unique_index_is_per_event_and_ignores_null() {
    if !sqlite3_available() {
        eprintln!("sqlite3 not on PATH — skipping");
        return;
    }
    let db = pre_migration_db("index");
    sqlite(&db, &migration_sql());
    sqlite(
        &db,
        "INSERT INTO attendees (id, event_id, email, display_code) VALUES \
           ('a1', 'evt-a', 'a1@x.com', '7KQ2XM'), \
           ('b1', 'evt-b', 'b1@x.com', '7KQ2XM'), \
           ('a2', 'evt-a', 'a2@x.com', NULL), \
           ('a3', 'evt-a', 'a3@x.com', NULL);",
    );
    let err = sqlite_try(
        &db,
        "INSERT INTO attendees (id, event_id, email, display_code) \
         VALUES ('a4', 'evt-a', 'a4@x.com', '7KQ2XM');",
    )
    .expect_err("a duplicate code in one event must be rejected");
    // The worker retries on exactly this text (`is_display_code_collision`).
    assert!(err.contains("UNIQUE constraint failed"), "{err}");
    assert!(err.contains("display_code"), "{err}");

    // The conditional assignment the worker runs never overwrites a code.
    sqlite(
        &db,
        "UPDATE attendees SET display_code = 'ZZZZZZ' \
         WHERE id = 'a1' AND event_id = 'evt-a' AND display_code IS NULL;",
    );
    assert_eq!(
        sqlite(&db, "SELECT display_code FROM attendees WHERE id = 'a1';"),
        "7KQ2XM"
    );
}

#[test]
fn a_code_from_one_event_does_not_resolve_in_another() {
    if !sqlite3_available() {
        eprintln!("sqlite3 not on PATH — skipping");
        return;
    }
    let db = pre_migration_db("scope");
    sqlite(&db, &migration_sql());
    sqlite(
        &db,
        "INSERT INTO attendees (id, event_id, email, display_code) VALUES \
           ('a1', 'evt-a', 'a1@x.com', 'ABC234'), \
           ('b1', 'evt-b', 'b1@x.com', 'XYZ789');",
    );
    let run = |event: &str, code: &str| {
        let sql = lookup_sql()
            .replace("?1", &format!("'{event}'"))
            .replace("?2", &format!("'{code}'"));
        sqlite(&db, &format!("{sql};"))
    };
    assert_eq!(run("evt-a", "ABC234"), "a1");
    assert_eq!(run("evt-b", "XYZ789"), "b1");
    assert_eq!(
        run("evt-b", "ABC234"),
        "",
        "event A's code resolved in event B"
    );
    assert_eq!(
        run("evt-a", "XYZ789"),
        "",
        "event B's code resolved in event A"
    );
}

/// Force every random draw to the same value: every row of an event collides
/// on every pass. The last pass must still leave the index buildable, keeping
/// exactly one holder per event and NULL for the rest.
#[test]
fn a_collision_storm_cannot_stop_the_index_from_building() {
    if !sqlite3_available() {
        eprintln!("sqlite3 not on PATH — skipping");
        return;
    }
    let sql = migration_sql();
    assert!(sql.contains("random()"));
    let forced = sql.replace("random()", "0");
    let db = pre_migration_db("storm");
    seed_two_events(&db, 5);
    sqlite(&db, &forced);
    let per_event = sqlite(
        &db,
        "SELECT event_id || '|' || COUNT(display_code) || '|' || COUNT(*) FROM attendees \
         GROUP BY event_id ORDER BY event_id;",
    );
    assert_eq!(per_event, "evt-a|1|5\nevt-b|1|5");
    assert_eq!(
        sqlite(
            &db,
            "SELECT DISTINCT display_code FROM attendees WHERE display_code IS NOT NULL;"
        ),
        "222222"
    );
}

// ---------------------------------------------------------------------------
// Static guards
// ---------------------------------------------------------------------------

#[test]
fn the_migration_alphabet_is_the_domain_alphabet() {
    let sql = migration_sql();
    let alphabet = std::str::from_utf8(DISPLAY_CODE_ALPHABET).expect("ascii");
    let quoted = format!("'{alphabet}'");
    let draws = sql.matches(&quoted).count();
    // 6 characters × 3 draw statements (backfill + two repair passes).
    assert_eq!(
        draws,
        DISPLAY_CODE_LEN * 3,
        "every draw uses the domain alphabet"
    );
    let modulo = format!("% {}", DISPLAY_CODE_ALPHABET.len());
    assert!(
        sql.contains(&modulo),
        "draws must be modulo the alphabet size"
    );
    // Never derived from a secret or a global id.
    let body: String = sql
        .lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    for banned in ["claim_token", "hex(id", "substr(id", "attendees.id"] {
        assert!(
            !body.contains(banned),
            "the backfill must not read {banned}"
        );
    }
}

/// Every file that INSERTs into D1 `attendees` must give the row a code.
/// The Durable Object's own SQLite is exempt: it has no such column, and its
/// D1 mirror (`event_do/sync.rs`) is covered.
#[test]
fn every_d1_attendee_writer_assigns_a_code() {
    let writers = [
        ("src/db/attendees/writes.rs", 2usize),
        ("src/db/attendees/walkin.rs", 1),
        ("src/db/attendees/management.rs", 1),
        ("src/durable_objects/event_do/sync.rs", 1),
    ];
    for (file, inserts) in writers {
        let src = code(file);
        assert_eq!(
            src.matches("INSERT INTO attendees").count(),
            inserts,
            "{file}: attendee INSERT count changed — does the new writer assign a display code?"
        );
        assert!(
            src.contains("assign_display_code") || src.contains("display_code = COALESCE("),
            "{file}: inserts attendees but never assigns a display code"
        );
    }
    // Each registration writer calls the assignment once per INSERT.
    let writes = code("src/db/attendees/writes.rs");
    assert_eq!(
        writes.matches("assign_display_code_best_effort(").count(),
        2
    );

    // No other file under src/ writes D1 attendees rows.
    let mut files = Vec::new();
    collect_rs(&worker_root().join("src"), &mut files);
    let known: Vec<&str> = writers.iter().map(|(f, _)| *f).collect();
    for path in files {
        let rel = path
            .strip_prefix(worker_root())
            .expect("under worker/")
            .to_string_lossy()
            .replace('\\', "/");
        let src = std::fs::read_to_string(&path).expect("read");
        let has_insert =
            src.contains("INSERT INTO attendees (") || src.contains("INSERT INTO attendees(");
        let exempt =
            rel == "src/durable_objects/event_do/checkin.rs" || rel == "src/db/nft_mint_jobs.rs";
        if has_insert && !exempt {
            assert!(
                known.contains(&rel.as_str()),
                "{rel}: a new attendee writer; give it a display code (.issues/178) and list it here"
            );
        }
    }
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir").flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => collect_rs(&path, out),
            false if path.extension().is_some_and(|e| e == "rs") => out.push(path),
            false => {}
        }
    }
}

#[test]
fn every_display_code_query_is_event_scoped() {
    let src = code("src/db/attendees/display_code.rs");
    assert!(
        src.contains(lookup_sql()),
        "lookup SQL drifted from this test"
    );
    for stmt in [
        "SELECT display_code FROM attendees",
        "UPDATE attendees SET display_code",
        "SELECT id FROM attendees",
    ] {
        let at = src
            .find(stmt)
            .unwrap_or_else(|| panic!("`{stmt}` is gone from display_code.rs"));
        let tail = &src[at..];
        let end = tail.find('"').unwrap_or(tail.len());
        assert!(
            tail[..end].contains("event_id = ?"),
            "`{stmt}` must filter on event_id (.issues/153)"
        );
    }
    // Retry is bounded.
    assert!(src.contains("DISPLAY_CODE_MAX_ATTEMPTS: usize = 5"));
}

/// The router block that a route string sits in, by the `let <name> =` that
/// opens it.
fn router_holding(router_src: &str, route: &str) -> String {
    let at = router_src
        .find(route)
        .unwrap_or_else(|| panic!("route {route} is not registered"));
    let head = &router_src[..at];
    let start = head
        .rfind("let ")
        .expect("a router binding precedes the route");
    let binding = &head[start + 4..];
    binding
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn the_lookup_is_staff_only_and_authorizes_the_event_first() {
    let router = code("src/handlers/mod.rs");
    assert_eq!(
        router_holding(&router, "\"/attendees/by-code/{code}\""),
        "protected",
        "the by-code lookup takes Extension<Claims>: it must sit in the staff router"
    );
    assert_eq!(router.matches("get_attendee_by_display_code").count(), 1);

    let handler = code("src/handlers/attendee/by_code.rs");
    let access = handler
        .find("resolve_event_with_access(")
        .expect("the handler must authorize the event like GET /api/attendee/{id}");
    let lookup = handler
        .find("find_attendee_id_by_display_code(")
        .expect("lookup call");
    assert!(access < lookup, "authorize the event before touching D1");
    assert!(handler.contains("Extension(claims): Extension<Claims>"));
    // Returns the id and nothing else: no claim token, no deposit data.
    assert!(handler.contains("json!({ \"attendee_id\": attendee_id })"));
    for leaked in ["claim_token", "deposit", "qr_url"] {
        assert!(
            !handler.contains(leaked),
            "by-code handler must not touch {leaked}"
        );
    }
}

#[test]
fn claim_and_deposit_handlers_never_accept_a_display_code() {
    let mut files = Vec::new();
    for dir in ["src/claim", "src/handlers/deposit"] {
        let path = worker_root().join(dir);
        assert!(path.is_dir(), "{dir} moved — update this guard");
        collect_rs(&path, &mut files);
    }
    files.push(worker_root().join("src/handlers/claim.rs"));
    assert!(
        !files.is_empty(),
        "claim/deposit handler dirs moved — update this guard"
    );
    for path in files {
        let src = std::fs::read_to_string(&path).expect("read");
        assert!(
            !src.contains("display_code") && !src.contains("DisplayCode"),
            "{}: the display code is not a credential (.issues/178)",
            path.display()
        );
    }
}

#[test]
fn the_public_ticket_returns_the_code_as_a_string() {
    let read = code("src/handlers/attendee/read.rs");
    assert!(read.contains("\"display_code\": display_code,"));
    assert!(
        read.contains("ensure_display_code(db, event_id, attendee_id)"),
        "the ticket read assigns a missing code lazily"
    );
    // A `null` would break the client (serde(default) does not cover null).
    assert!(read.contains("code.unwrap_or_default()"));
}
