//! `sql/event_purge.sql` splits into one DELETE per table, each bound to the
//! event id only (.issues/187). The Python test runs the same file; this pins
//! the split the Rust batch uses.

use event_checkin_worker::db::event_purge::{EVENT_PURGE_KEEPS, purge_statements};

#[test]
fn one_event_scoped_delete_per_statement() {
    let statements = purge_statements();
    assert_eq!(statements.len(), 20, "{statements:#?}");
    for sql in &statements {
        assert!(
            sql.starts_with("DELETE FROM ") && sql.ends_with(" WHERE event_id = ?1"),
            "{sql}"
        );
        assert!(!sql.contains("--") && !sql.contains(';'), "{sql}");
    }
}

#[test]
fn attendees_go_last_and_money_is_never_purged() {
    let statements = purge_statements();
    assert!(
        statements
            .last()
            .unwrap()
            .starts_with("DELETE FROM attendees ")
    );
    for kept in EVENT_PURGE_KEEPS {
        assert!(
            !statements
                .iter()
                .any(|s| s.starts_with(&format!("DELETE FROM {kept} "))),
            "{kept} is kept on purpose"
        );
    }
}

/// .issues/191: the purge batch returns the credit applied to the event
/// before anything is deleted, and the nightly repair names the audit
/// actions exactly as the hard delete stores them.
#[test]
fn credit_is_returned_in_the_purge_batch_and_the_repair_matches_the_audit() {
    let purge = include_str!("../src/db/event_purge.rs");
    let batch = purge
        .find("pub async fn purge_event_rows")
        .expect("purge fn");
    let ret = purge[batch..]
        .find("RETURN_DELETED_EVENT_APPLIES_SQL")
        .expect("return in the batch");
    let deletes = purge[batch..]
        .find(".chain(purge_statements())")
        .expect("deletes");
    assert!(ret < deletes, "the return comes first in the batch");

    let ledger = include_str!("../src/db/credit_ledger.rs");
    let audit = include_str!("../src/audit_store.rs");
    let enum_at = audit.find("pub enum AuditAction").expect("enum");
    assert!(
        audit[..enum_at].ends_with("#[serde(rename_all = \"snake_case\")]\n"),
        "the stored action is the snake_case variant name"
    );
    for variant in ["EventHardDeleted", "ForceDeleteUsed"] {
        assert!(
            audit[enum_at..].contains(&format!("    {variant},")),
            "{variant}"
        );
        let stored: String = variant
            .chars()
            .enumerate()
            .flat_map(|(i, c)| match (i, c.is_ascii_uppercase()) {
                (0, _) | (_, false) => vec![c.to_ascii_lowercase()],
                (_, true) => vec!['_', c.to_ascii_lowercase()],
            })
            .collect();
        assert!(
            ledger.contains(&format!("'{stored}'")),
            "repair SQL lacks the stored action '{stored}'"
        );
    }
}
