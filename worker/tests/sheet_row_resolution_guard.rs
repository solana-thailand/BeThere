//! Sheet writes find their row by `api_id` at write time, and appends start in
//! column A (`.issues/151`).
//!
//! On the RTM#6 sheet (2026-09-25) every status write from D1 had silently
//! failed: writers took `row_index` from D1's `sheet_row_index`, which is empty
//! for every attendee D1 created, so each range addressed row 0. And a row
//! appended after hand-edited rows landed one column right. The lookup and the
//! append-position parser are unit-tested in `domain/tests/sheet_row_lookup.rs`;
//! this guard keeps every writer on them.

use std::{fs, path::Path};

fn source(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel);
    let body = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} is unreadable: {e} — was the module moved?"));
    body.lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn function_body<'a>(rel: &str, code: &'a str, name: &str) -> &'a str {
    let signature = format!("pub async fn {name}(");
    let start = code
        .find(&signature)
        .unwrap_or_else(|| panic!("{rel}: `{name}` is gone — was it renamed?"));
    let rest = &code[start..];
    let end = rest.find("\n}\n").unwrap_or(rest.len());
    &rest[..end]
}

/// Writers that address one attendee's row. Each must take a `SheetRow` and
/// resolve it before building any range.
const ROW_WRITERS: [(&str, &[&str]); 4] = [
    ("sheets/bg_sync.rs", &["update_qr_urls"]),
    (
        "sheets/write/checkin.rs",
        &[
            "mark_checked_in",
            "mark_virtual_checked_in",
            "clear_checked_in",
            "mark_claimed",
            "update_qr_urls",
        ],
    ),
    (
        "sheets/write/deposit.rs",
        &[
            "write_bank_info",
            "write_deposit_verification",
            "update_deposit_method",
            "write_refund_status",
            "write_refund_link",
        ],
    ),
    ("sheets/write/append.rs", &["update_participation_type"]),
];

#[test]
fn row_writers_resolve_the_row_before_writing() {
    for (rel, names) in ROW_WRITERS {
        let code = source(rel);
        for name in names {
            let body = function_body(rel, &code, name);
            assert!(
                body.contains("SheetRow") && !body.contains("row_index: usize"),
                "{rel}: `{name}` must take a SheetRow, not a remembered row number"
            );
            let resolved = body
                .find("resolve_row(")
                .or_else(|| body.find("resolve_rows("))
                .unwrap_or_else(|| panic!("{rel}: `{name}` never resolves its row"));
            let first_range = body.find("ValueRange {").unwrap_or(body.len());
            assert!(
                resolved < first_range,
                "{rel}: `{name}` must resolve the row before building a range"
            );
        }
    }
}

/// `bg_sync`'s one-row writers share `write_row`, which resolves the row
/// before it builds any cell. Each writer must go through it and must not
/// build a range of its own.
const BG_SYNC_ROW_WRITERS: [&str; 9] = [
    "mark_checked_in",
    "mark_virtual_checked_in",
    "clear_checked_in",
    "mark_claimed",
    "update_participation_type",
    "write_bank_info",
    "write_deposit_verification",
    "write_refund_status",
    "write_refund_link",
];

#[test]
fn bg_sync_row_writers_go_through_the_resolving_helper() {
    let rel = "sheets/bg_sync.rs";
    let code = source(rel);

    let start = code
        .find("async fn write_row(")
        .unwrap_or_else(|| panic!("{rel}: `write_row` is gone — was it renamed?"));
    let helper = &code[start..];
    let helper = &helper[..helper.find("\n}\n").unwrap_or(helper.len())];
    assert!(
        helper.contains("row: &SheetRow"),
        "{rel}: `write_row` must take a SheetRow, not a remembered row number"
    );
    let resolved = helper
        .find("resolve_row(")
        .unwrap_or_else(|| panic!("{rel}: `write_row` never resolves its row"));
    let first_cell = helper
        .find("row_cells(")
        .unwrap_or_else(|| panic!("{rel}: `write_row` no longer builds its cells"));
    assert!(
        resolved < first_cell,
        "{rel}: `write_row` must resolve the row before building a range"
    );

    for name in BG_SYNC_ROW_WRITERS {
        let body = function_body(rel, &code, name);
        assert!(
            body.contains("SheetRow") && !body.contains("row_index: usize"),
            "{rel}: `{name}` must take a SheetRow, not a remembered row number"
        );
        assert!(
            body.contains("write_row("),
            "{rel}: `{name}` must write through `write_row`"
        );
        assert!(
            !body.contains("a1::cell(") && !body.contains("ValueRange {"),
            "{rel}: `{name}` builds its own range instead of going through `write_row`"
        );
    }
}

/// The blocking one-row writers resolve their row, then hand it to
/// `values::write_cells`, which builds the cells and sends one batch. None of
/// them builds a range or a request body of its own.
#[test]
fn blocking_row_writers_write_through_the_shared_cells_helper() {
    for (rel, names) in ROW_WRITERS {
        if rel == "sheets/bg_sync.rs" {
            continue;
        }
        let code = source(rel);
        for name in names.iter().filter(|name| **name != "update_qr_urls") {
            let body = function_body(rel, &code, name);
            let resolved = body
                .find("resolve_row(")
                .unwrap_or_else(|| panic!("{rel}: `{name}` never resolves its row"));
            let written = body
                .find("write_cells(")
                .unwrap_or_else(|| panic!("{rel}: `{name}` must write through `write_cells`"));
            assert!(
                resolved < written,
                "{rel}: `{name}` must resolve the row before writing it"
            );
            assert!(
                !body.contains("a1::cell(") && !body.contains("BatchUpdateRequest"),
                "{rel}: `{name}` builds its own range instead of going through `write_cells`"
            );
        }
    }

    let rel = "sheets/values.rs";
    let code = source(rel);
    let start = code
        .find("pub(crate) async fn write_cells(")
        .unwrap_or_else(|| panic!("{rel}: `write_cells` is gone — was it renamed?"));
    let helper = &code[start..];
    let helper = &helper[..helper.find("\n}\n").unwrap_or(helper.len())];
    assert!(
        helper.contains("row_cells(") && helper.contains("send("),
        "{rel}: `write_cells` must build through `row_cells` and send one batch"
    );
    assert!(
        code.contains("a1::cell("),
        "{rel}: `row_cells` must skip missing columns through `a1::cell`"
    );
}

#[test]
fn attendee_appends_are_pinned_to_column_a() {
    for rel in ["sheets/write/append.rs", "sheets/bg_sync.rs"] {
        let code = source(rel);
        assert!(
            !code.contains(":append?"),
            "{rel}: attendee rows must be appended through locate::append_row, not a raw :append"
        );
        assert!(
            code.contains("locate::append_row("),
            "{rel}: attendee rows must be appended through locate::append_row"
        );
    }
    let locate = source("sheets/locate.rs");
    let body = function_body("sheets/locate.rs", &locate, "append_row");
    assert!(
        body.contains("range_start(") && body.contains("put_json("),
        "sheets/locate.rs: append_row must read where the row landed and move it back to column A"
    );
}

#[test]
fn the_pdpa_sheet_clear_finds_its_row_by_api_id() {
    let code = source("handlers/privacy.rs");
    assert!(
        !code.contains("row_index > 0"),
        "handlers/privacy.rs: the row-number guard skipped every D1-created attendee"
    );
    assert!(
        code.contains("async fn clear_sheet_pii(state: &AppState, event_id: &str, row: SheetRow)")
            && code.contains("locate::resolve_row("),
        "handlers/privacy.rs: clear_sheet_pii must resolve the attendee's row by api_id"
    );
}

#[test]
fn qr_generation_is_keyed_by_api_id() {
    let code = source("handlers/qr.rs");
    assert!(
        !code.contains("a.row_index == "),
        "handlers/qr.rs: matching generated QRs back to attendees by row number sent \
         every URL to the first attendee when row numbers were all 0"
    );
}
