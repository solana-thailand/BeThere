//! `sql/event_purge.sql` splits into one DELETE per table, each bound to the
//! event id only (.issues/187). The Python test runs the same file; this pins
//! the split the Rust batch uses.

use event_checkin_worker::db::event_purge::{EVENT_PURGE_KEEPS, purge_statements};

#[test]
fn one_event_scoped_delete_per_statement() {
    let statements = purge_statements();
    assert_eq!(statements.len(), 15, "{statements:#?}");
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
