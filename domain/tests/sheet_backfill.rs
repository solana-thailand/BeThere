//! `plan_backfill` — the D1 → sheet backfill for `.issues/151`. It fills empty
//! cells only, finds rows by api_id, and never guesses a row.

use event_checkin_domain::models::attendee::{
    CellWant, CellWrite, ColumnKey, ColumnMapping, plan_backfill,
};

fn row(cells: &[&str]) -> Vec<String> {
    cells.iter().map(|c| c.to_string()).collect()
}

fn want(api_id: &str, column: ColumnKey, value: &str) -> CellWant {
    CellWant {
        api_id: api_id.to_string(),
        column,
        value: value.to_string(),
    }
}

fn qr_col() -> usize {
    ColumnMapping::hardcoded().get_or_default(ColumnKey::QrCodeUrl)
}

/// A sheet whose rows are wide enough to hold the QR column.
fn sheet(rows: &[(&str, &str)]) -> Vec<Vec<String>> {
    let width = qr_col() + 1;
    let mut out = vec![row(&["api_id"])];
    for (id, qr) in rows {
        let mut cells = vec![String::new(); width];
        cells[0] = id.to_string();
        cells[qr_col()] = qr.to_string();
        out.push(cells);
    }
    out
}

#[test]
fn an_empty_cell_is_written_at_the_row_holding_the_api_id() {
    let s = sheet(&[("a", ""), ("b", "")]);
    let plan = plan_backfill(
        &s,
        &ColumnMapping::hardcoded(),
        &[want("b", ColumnKey::QrCodeUrl, "https://q/b")],
    );
    assert_eq!(
        plan.writes,
        vec![CellWrite {
            row: 3,
            column: qr_col(),
            value: "https://q/b".into()
        }]
    );
}

#[test]
fn a_filled_cell_is_kept_even_when_it_differs() {
    let s = sheet(&[("a", "typed by the organizer")]);
    let plan = plan_backfill(
        &s,
        &ColumnMapping::hardcoded(),
        &[want("a", ColumnKey::QrCodeUrl, "https://q/a")],
    );
    assert!(plan.writes.is_empty());
    assert_eq!(plan.already_filled, 1);
}

#[test]
fn a_short_row_counts_as_empty() {
    // The Sheets API drops trailing empty cells, so the QR cell is absent.
    let s = vec![row(&["api_id"]), row(&["a", "Name"])];
    let plan = plan_backfill(
        &s,
        &ColumnMapping::hardcoded(),
        &[want("a", ColumnKey::QrCodeUrl, "https://q/a")],
    );
    assert_eq!(plan.writes.len(), 1);
    assert_eq!(plan.writes[0].row, 2);
}

#[test]
fn missing_and_duplicated_ids_are_counted_and_skipped() {
    let s = sheet(&[("a", ""), ("dup", ""), ("dup", "")]);
    let plan = plan_backfill(
        &s,
        &ColumnMapping::hardcoded(),
        &[
            want("gone", ColumnKey::QrCodeUrl, "x"),
            want("dup", ColumnKey::QrCodeUrl, "y"),
        ],
    );
    assert!(plan.writes.is_empty());
    assert_eq!((plan.row_missing, plan.row_duplicated), (1, 1));
}

#[test]
fn an_empty_wanted_value_writes_nothing() {
    let s = sheet(&[("a", "")]);
    let plan = plan_backfill(
        &s,
        &ColumnMapping::hardcoded(),
        &[want("a", ColumnKey::QrCodeUrl, "  ")],
    );
    assert_eq!(plan, Default::default());
}
