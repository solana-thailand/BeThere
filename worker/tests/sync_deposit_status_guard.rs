//! Regression guard for the Sheet → D1 sync demoting deposits
//! (`.issues/136` §6.6).
//!
//! `upsert_attendee_full` is the sync's only writer. It used to set
//! `deposit_status = excluded.deposit_status` — the sheet's value, always. The
//! sheet lags D1 by construction, so a sync turned a verified deposit back into
//! `none`, and a `refunded` one back into `verified`. Both were reproduced by
//! running the statement against the migrated schema in SQLite.
//!
//! Nothing errors when this regresses; the row just says something older.

use std::fs;
use std::path::Path;

fn upsert_sql() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/db/attendees/management.rs");
    let src = fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let start = src
        .find("pub(crate) async fn upsert_attendee_full")
        .expect("upsert_attendee_full must exist — it is the sync's writer");
    let body = &src[start..];
    let end = body
        .find("bind_refs")
        .expect("upsert_attendee_full binds its SQL");
    // Collapse whitespace so the guard does not depend on line wrapping.
    body[..end].split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn sync_never_takes_the_sheet_deposit_status_unconditionally() {
    let sql = upsert_sql();
    assert!(
        !sql.contains("deposit_status = excluded.deposit_status,"),
        "the sync must not overwrite deposit_status with the sheet's value — the \
         sheet lags D1 and this demotes verified deposits (.issues/136 §6.6)"
    );
}

#[test]
fn sync_deposit_status_is_ranked_and_preserves_off_ladder_states() {
    let sql = upsert_sql();
    for rung in [
        "WHEN 'none' THEN 0",
        "WHEN 'agreed' THEN 1",
        "WHEN 'pending' THEN 2",
        "WHEN 'verified' THEN 3",
    ] {
        assert!(
            sql.contains(rung),
            "deposit_status ladder is missing `{rung}`"
        );
    }
    // An unknown D1 value (refunded, manual_refund, …) must rank above anything
    // the sheet can derive, so it is never overwritten.
    assert!(
        sql.contains("ELSE 99 END"),
        "off-ladder D1 deposit states must outrank every sheet-derived value"
    );
    assert!(
        sql.contains("THEN excluded.deposit_status ELSE attendees.deposit_status END"),
        "deposit_status must only take the sheet value when it ranks higher"
    );
}
