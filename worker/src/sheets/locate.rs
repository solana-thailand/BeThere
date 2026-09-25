//! Where an attendee's row is, and where an appended row went.
//!
//! Two failures seen on the RTM#6 sheet (2026-09-25):
//!
//! - **Row writes addressed row 0.** Writers took `row_index` from D1's
//!   `sheet_row_index`, which is empty for every attendee D1 created, so each
//!   check-in, deposit, bank, refund and QR write built a range like
//!   `Attendees!R0` that the API rejects, and the error was only logged. Rows
//!   are also inserted by hand, so a remembered number goes stale anyway.
//!   [`resolve_row`] finds the row by `api_id` in column A just before writing.
//! - **An append landed one column right.** Rows added by hand with column A
//!   blank made Google's `:append` treat the table as starting at column B, so
//!   a new registration was written from B. [`append_row`] reads where the row
//!   actually landed and moves it back to column A.

use event_checkin_domain::models::attendee::{
    RowMatch, SheetRow, column_letter, find_row, range_start,
};

use crate::http::{ValueRange, fetch_sheet_range, post_json, put_json};

fn values_url(sheet_id: &str, range: &str) -> String {
    format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(range)
    )
}

/// The current 1-based row of `row`'s attendee, read from column A now.
///
/// Errors (logged by the caller, which then skips its write) when the
/// attendee has no row, or when more than one row carries its `api_id`:
/// writing to a guessed row would put one person's status on another's.
pub async fn resolve_row(
    row: &SheetRow,
    sheet_id: &str,
    sheet_ref: &str,
    access_token: &str,
) -> Result<usize, String> {
    let column = fetch_sheet_range(
        &values_url(sheet_id, &format!("{sheet_ref}!A:A")),
        access_token,
    )
    .await?;
    match find_row(&column.values, row.api_id()) {
        RowMatch::Found(n) => Ok(n),
        RowMatch::NotFound => {
            Err("attendee has no row in the sheet (no api_id match in column A)".into())
        }
        RowMatch::Duplicate => {
            Err("more than one sheet row has this attendee's api_id; not guessing".into())
        }
    }
}

/// [`resolve_row`] for a batch: column A is read once. Attendees with no
/// single matching row are dropped (and logged), the rest keep their payload.
pub async fn resolve_rows<T>(
    rows: Vec<(SheetRow, T)>,
    sheet_id: &str,
    sheet_ref: &str,
    access_token: &str,
) -> Result<Vec<(usize, T)>, String> {
    let column = fetch_sheet_range(
        &values_url(sheet_id, &format!("{sheet_ref}!A:A")),
        access_token,
    )
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(row, payload)| match find_row(&column.values, row.api_id()) {
            RowMatch::Found(n) => Some((n, payload)),
            miss => {
                tracing::warn!(lookup = ?miss, "sheet row lookup: one write in the batch skipped");
                None
            }
        })
        .collect())
}

/// Where an `:append` wrote, from its response.
#[derive(serde::Deserialize, Default)]
struct AppendResponse {
    #[serde(default)]
    updates: AppendUpdates,
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppendUpdates {
    #[serde(default)]
    updated_range: String,
}

/// Append one attendee row (column A first) and make sure it starts in
/// column A. Returns the 1-based row it ended up on, when Google said.
///
/// When the append lands `k` columns to the right, the row is rewritten from
/// column A in the same sheet row, with `k` empty cells after it to clear the
/// shifted tail. Logged as an error either way, so a hand-edited sheet that
/// provokes it is visible.
pub async fn append_row(
    sheet_id: &str,
    sheet_ref: &str,
    mut row: Vec<String>,
    access_token: &str,
) -> Result<Option<usize>, String> {
    let last_idx = row.iter().rposition(|v| !v.is_empty()).unwrap_or(0);
    row.truncate(last_idx + 1);
    let range = format!("{sheet_ref}!A:{}", column_letter(last_idx));
    let url = format!(
        "{}:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS",
        values_url(sheet_id, &range)
    );
    let body = ValueRange {
        range,
        values: vec![row.clone()],
    };
    let response: AppendResponse = post_json(&url, &body, Some(access_token)).await?;

    let Some((start_col, landed_row)) = range_start(&response.updates.updated_range) else {
        tracing::warn!("sheet append: no updatedRange in the response; cannot check its column");
        return Ok(None);
    };
    if start_col == 0 {
        return Ok(Some(landed_row));
    }

    tracing::error!(
        shifted_columns = start_col,
        row = landed_row,
        "sheet append landed off column A (rows edited by hand?); moving it back"
    );
    row.extend(std::iter::repeat_n(String::new(), start_col));
    let fix_range = format!(
        "{sheet_ref}!A{landed_row}:{}{landed_row}",
        column_letter(row.len() - 1)
    );
    let fix = ValueRange {
        range: fix_range.clone(),
        values: vec![row],
    };
    put_json(
        &format!(
            "{}?valueInputOption=USER_ENTERED",
            values_url(sheet_id, &fix_range)
        ),
        &fix,
        access_token,
    )
    .await
    .map_err(|e| format!("appended row landed off column A and moving it back failed: {e}"))?;
    Ok(Some(landed_row))
}
