//! Executes migration `0029_backfill_claim_locks_from_attendees.sql` against a
//! real SQLite database built from the real migration chain, and asserts the
//! five behaviours it promises.
//!
//! The backfill fills `claim_locks`' `asset_id` / `signature` / `claimed_at`
//! from the `attendees` row sharing its `claim_token`. Earlier notes assumed the
//! mint signature could only be recovered by joining against Solana; it is
//! already in the same database, so this is a pure SQL join.
//!
//! Runs `sqlite3` as a subprocess rather than linking a SQLite crate: the worker
//! compiles to `wasm32-unknown-unknown`, and a C-linking dependency for one
//! host-target test is a poor trade. If the binary is absent the SQL execution
//! is skipped, but the static assertions on the migration text still run, so the
//! test can never pass while reporting nothing.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

const BACKFILL: &str = "0029_backfill_claim_locks_from_attendees.sql";

fn sqlite3_available() -> bool {
    Command::new("sqlite3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `sql` against `db` and return stdout, panicking on any SQLite error.
///
/// The SQL goes in on stdin, not as an argument: a migration file starts with a
/// `--` comment, which `sqlite3` would parse as an unknown command-line option.
fn sqlite(db: &Path, sql: &str) -> String {
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
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stderr.trim().is_empty(),
        "sqlite3 rejected the statement:\n{stderr}\n--- sql ---\n{sql}"
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Build a database from every migration except the backfill itself.
fn seeded_db(dir: &Path) -> PathBuf {
    let db = dir.join("backfill_test.db");
    let mut files: Vec<_> = std::fs::read_dir(migrations_dir())
        .expect("migrations dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .filter(|p| p.file_name().is_some_and(|n| n != BACKFILL))
        .collect();
    files.sort();
    assert!(
        files.len() > 20,
        "expected the full migration chain, found {} files — the test is not \
         exercising the real schema",
        files.len()
    );
    for f in files {
        let sql = std::fs::read_to_string(&f).expect("read migration");
        sqlite(&db, &sql);
    }
    db
}

#[test]
fn the_backfill_repairs_only_the_rows_it_should() {
    if !sqlite3_available() {
        eprintln!("sqlite3 not on PATH — skipping the executed half of this test");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("bethere_backfill_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("tmpdir");
    let db = seeded_db(&tmp);

    // One fixture per branch the migration has to get right.
    sqlite(
        &db,
        "INSERT INTO attendees (id,event_id,email,claim_token,claimed_at,claim_asset_id,claim_signature) VALUES
           ('a1','evt1','a@x.com','tok-happy','2026-04-26T10:00:00.123456+00:00','ASSET1','SIG1'),
           ('a2','evt1','b@x.com','tok-done','2026-05-01T10:00:00+00:00','ASSET2','SIG2'),
           ('a3','evt1','c@x.com','tok-partial','2026-05-02T10:00:00+00:00',NULL,'SIG3'),
           ('a4','evt1','d@x.com','tok-unclaimed',NULL,NULL,NULL),
           ('a5','evt1','e@x.com','tok-badts','not-a-timestamp','ASSET5','SIG5');
         INSERT INTO claim_locks (event_id,token,lock_id,wallet,expires_at,asset_id,signature,claimed_at) VALUES
           ('evt1','tok-happy','L1','W1','2026-04-26T10:05:00Z',NULL,NULL,NULL),
           ('evt1','tok-done','L2','W2','2026-07-30T10:00:00Z','OLD_ASSET','OLD_SIG','2026-05-01T10:00:00+00:00'),
           ('evt1','tok-partial','L3','W3','2026-05-02T10:05:00Z',NULL,NULL,NULL),
           ('evt1','tok-unclaimed','L4','W4','2026-05-03T10:05:00Z',NULL,NULL,NULL),
           ('evt1','tok-badts','L5','W5','2026-05-04T10:05:00Z',NULL,NULL,NULL),
           ('evt1','tok-orphan','L6','W6','2026-05-05T10:05:00Z',NULL,NULL,NULL);",
    );

    let backfill = std::fs::read_to_string(migrations_dir().join(BACKFILL)).expect("read backfill");
    sqlite(&db, &backfill);

    let row = |token: &str| {
        sqlite(
            &db,
            &format!(
                "SELECT coalesce(asset_id,'~')||'|'||coalesce(signature,'~')||'|'|| \
                 coalesce(claimed_at,'~')||'|'||expires_at \
                 FROM claim_locks WHERE token='{token}';"
            ),
        )
    };

    // 1. Happy path: all three filled, and `expires_at` moved to the 90-day
    //    retention horizon measured from the *original* claim, exactly as
    //    `finalize_claim_lock` would have written it at the time.
    assert_eq!(
        row("tok-happy"),
        "ASSET1|SIG1|2026-04-26T10:00:00.123456+00:00|2026-07-25T10:00:00Z"
    );

    // 2. An already-finalized row is never overwritten.
    assert_eq!(
        row("tok-done"),
        "OLD_ASSET|OLD_SIG|2026-05-01T10:00:00+00:00|2026-07-30T10:00:00Z"
    );

    // 3. A half-written attendee row cannot produce a half-written lock row.
    assert_eq!(row("tok-partial"), "~|~|~|2026-05-02T10:05:00Z");

    // 4. An attendee who never claimed contributes nothing.
    assert_eq!(row("tok-unclaimed"), "~|~|~|2026-05-03T10:05:00Z");

    // 5. An unparseable `claimed_at` makes `strftime` return NULL. `expires_at`
    //    is NOT NULL, so without the COALESCE this aborts the whole statement —
    //    the exact failure mode `tests/not_null_column_guard.rs` exists for.
    assert_eq!(
        row("tok-badts"),
        "ASSET5|SIG5|not-a-timestamp|2026-05-04T10:05:00Z"
    );

    // 6. A lock with no matching attendee is left alone.
    assert_eq!(row("tok-orphan"), "~|~|~|2026-05-05T10:05:00Z");

    // Re-running must change nothing: D1 migrations are applied once, but a
    // hand-run during recovery must not be destructive.
    let snapshot = sqlite(
        &db,
        "SELECT group_concat(token||'|'||coalesce(asset_id,'~')||'|'|| \
         coalesce(signature,'~')||'|'||coalesce(claimed_at,'~')||'|'||expires_at) \
         FROM (SELECT * FROM claim_locks ORDER BY token);",
    );
    sqlite(&db, &backfill);
    let after = sqlite(
        &db,
        "SELECT group_concat(token||'|'||coalesce(asset_id,'~')||'|'|| \
         coalesce(signature,'~')||'|'||coalesce(claimed_at,'~')||'|'||expires_at) \
         FROM (SELECT * FROM claim_locks ORDER BY token);",
    );
    assert_eq!(snapshot, after, "the backfill is not idempotent");

    // No NOT NULL column was nulled anywhere.
    assert_eq!(
        sqlite(
            &db,
            "SELECT count(*) FROM claim_locks \
             WHERE expires_at IS NULL OR wallet IS NULL OR lock_id IS NULL \
                OR event_id IS NULL OR token IS NULL OR started_at IS NULL;"
        ),
        "0"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

/// Runs even without `sqlite3`, so the test file is never silently vacuous.
#[test]
fn the_backfill_keeps_its_scope_guards() {
    let sql = std::fs::read_to_string(migrations_dir().join(BACKFILL)).expect("read backfill");
    for guard in [
        "claim_locks.asset_id IS NULL",
        "claim_locks.signature IS NULL",
        "claim_locks.claimed_at IS NULL",
        "a.claimed_at IS NOT NULL",
        "a.claim_asset_id IS NOT NULL",
        "a.claim_signature IS NOT NULL",
        "COALESCE(",
    ] {
        assert!(
            sql.contains(guard),
            "migration {BACKFILL} lost its `{guard}` guard — it can now overwrite \
             finalized rows, copy half-written ones, or null a NOT NULL column"
        );
    }
}
