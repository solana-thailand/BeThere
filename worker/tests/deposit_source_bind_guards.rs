//! Guards for the `deposit_source` bind (`.issues/138`).
//!
//! On 2026-09-23 every attendee slip upload in production returned
//! `500 internal error`. Migration 0047 gave `thb_deposits.deposit_source` a
//! `CHECK (deposit_source IS NULL OR deposit_source IN ('cash','credit','comp'))`,
//! and the writer bound the module's usual "absent means empty string" value.
//! `''` satisfies neither branch of that CHECK, so SQLite aborted the INSERT
//! and the handler turned it into a 500 — for every upload, from the moment the
//! migration landed.
//!
//! The defect is invisible to the type system (`Option<DepositSource>` → `&str`
//! is a total function) and invisible to every test that does not run real SQL
//! against the real constraint. So this file asserts against the migration text
//! itself.

use std::fs;
use std::path::Path;

fn repo_file(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn strip_rs_comments(code: &str) -> String {
    code.lines()
        .map(|l| match l.trim_start().starts_with("//") {
            true => "",
            false => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The constraint that makes the empty string fatal. If it is ever relaxed, the
/// reasoning in `deposit_source_bind` stops applying and should be revisited
/// rather than silently kept.
#[test]
fn migration_0047_still_constrains_deposit_source() {
    let sql = repo_file("migrations/0047_thb_deposit_source.sql");
    assert!(
        sql.contains(
            "CHECK (deposit_source IS NULL OR deposit_source IN ('cash', 'credit', 'comp'))"
        ),
        "0047's CHECK is what makes an empty-string bind fatal. If it changed, \
         re-read .issues/138 before assuming the NULL bind is still required"
    );
}

/// The fix itself: `None` must bind SQL NULL, never `""`.
#[test]
fn an_absent_deposit_source_binds_null_not_an_empty_string() {
    let code = strip_rs_comments(&repo_file("src/db/thb_deposits.rs"));
    assert!(
        code.contains("None => D1Type::Null"),
        "deposit_source must bind D1Type::Null when absent. Binding \"\" — the \
         convention every other optional column in this module uses — fails \
         0047's CHECK and 500s every slip upload (.issues/138)"
    );
    assert!(
        !code.contains("None => \"\","),
        "the empty-string arm is back. That is the exact production incident of \
         2026-09-23"
    );
}

/// Both writers, not one. `insert_thb_deposit` and `update_thb_deposit` are the
/// pair; fixing one and leaving the sibling is a recurring shape in this repo.
#[test]
fn both_thb_writers_use_the_shared_bind() {
    let code = strip_rs_comments(&repo_file("src/db/thb_deposits.rs"));
    assert_eq!(
        code.matches("deposit_source_bind(deposit.deposit_source)")
            .count(),
        2,
        "both insert_thb_deposit and update_thb_deposit must bind deposit_source \
         through the shared helper — one raw bind is one more 500"
    );
}

/// The upload path really does pass `None`, which is why this matters at all.
/// If it ever started classifying at upload time, this test should be the thing
/// that makes someone re-read the reasoning rather than delete it.
#[test]
fn the_attendee_upload_path_writes_an_absent_source() {
    for handler in [
        "src/handlers/deposit/thb/handlers/slip_upload.rs",
        "src/handlers/deposit/thb/handlers/slip_admin_upload.rs",
    ] {
        let code = strip_rs_comments(&repo_file(handler));
        assert!(
            code.contains("deposit_source: None"),
            "{handler} is expected to leave deposit_source unset at upload time \
             — a slip's source is not known until it is verified. That is the \
             case .issues/138 is about"
        );
    }
}
