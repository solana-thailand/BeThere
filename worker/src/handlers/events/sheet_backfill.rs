//! D1 → sheet backfill (`.issues/151`, "Not done: Backfill").
//!
//! Before the 151 fix every row write from D1 addressed row 0 and was dropped,
//! so recent sheets lack statuses D1 holds: verified deposits, ticket QRs and
//! check-ins. This writes them back, filling **empty cells only**
//! (`domain::plan_backfill`), so nothing an organizer typed is overwritten.
//!
//! `POST /api/events/{id}/sheet-backfill` is a dry run: it reads the sheet
//! and reports what it would write. `?apply=true` writes. Super admin only,
//! because it touches every row of an organizer's sheet at once.

use axum::Extension;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use event_checkin_domain::models::attendee::{
    Attendee, CellWant, ColumnKey, column_letter, plan_backfill,
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::DepositSource;
use event_checkin_domain::models::error::AppError;

use crate::error::ApiOk;
use crate::http::{BatchUpdateRequest, ValueRange, batch_update_sheet, fetch_sheet_range};
use crate::state::AppState;

/// Cells per `values:batchUpdate` call, well under the API's request size.
const WRITE_CHUNK: usize = 400;

#[derive(Deserialize)]
pub struct BackfillQuery {
    #[serde(default)]
    apply: bool,
}

#[derive(Serialize)]
pub struct SheetBackfillResponse {
    pub event_id: String,
    pub applied: bool,
    pub attendees: usize,
    /// Cells D1 has a value for, by kind.
    pub wanted_deposit_cells: usize,
    pub wanted_qr_cells: usize,
    pub wanted_checkin_cells: usize,
    /// Empty cells that get (or would get) the D1 value.
    pub writes: usize,
    /// Cells the sheet already fills; kept as they are.
    pub already_filled: usize,
    pub row_missing: usize,
    pub row_duplicated: usize,
    /// Cells whose kind has no column on this sheet; skipped.
    pub column_missing: usize,
    /// Cells actually written (0 on a dry run).
    pub written: usize,
}

fn push(wants: &mut Vec<CellWant>, api_id: &str, column: ColumnKey, value: &str) {
    wants.push(CellWant {
        api_id: api_id.to_string(),
        column,
        value: value.to_string(),
    });
}

/// QR and check-in cells, as the live writers fill them.
fn attendee_wants(attendees: &[Attendee]) -> (Vec<CellWant>, usize, usize) {
    let mut wants = Vec::new();
    let (mut qr, mut checkin) = (0, 0);
    for a in attendees {
        if let Some(url) = a.qr_code_url.as_deref().filter(|u| !u.trim().is_empty()) {
            push(&mut wants, &a.api_id, ColumnKey::QrCodeUrl, url);
            qr += 1;
        }
        if let Some(at) = a.checked_in_at.as_deref().filter(|t| !t.trim().is_empty()) {
            push(&mut wants, &a.api_id, ColumnKey::CheckedInAt, at);
            checkin += 1;
            if let Some(by) = a.checked_in_by.as_deref() {
                push(&mut wants, &a.api_id, ColumnKey::CheckedInBy, by);
                checkin += 1;
            }
            if let Some(token) = a.claim_token.as_deref() {
                push(&mut wants, &a.api_id, ColumnKey::ClaimToken, token);
                checkin += 1;
            }
        }
    }
    (wants, qr, checkin)
}

#[worker::send]
pub async fn sheet_backfill(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<String>,
    Query(query): Query<BackfillQuery>,
) -> Result<ApiOk<SheetBackfillResponse>, crate::error::WorkerError> {
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can backfill a sheet".into()).into());
    }
    tracing::info!(
        event_id = %event_id,
        apply = query.apply,
        staff_fingerprint = %state.log_fingerprint(&claims.email),
        "sheet backfill requested"
    );

    let d1 = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not configured".into()))?;
    let config = crate::event_store::get_event_config_with_fallback(
        state.events_kv.as_ref(),
        Some(d1),
        &event_id,
    )
    .await
    .map_err(AppError::Internal)?
    .ok_or_else(|| AppError::NotFound(format!("event '{event_id}' not found")))?;
    if config.sheet_id.trim().is_empty() {
        return Err(AppError::Validation("this event has no sheet".into()).into());
    }

    let attendees = crate::db::attendees::get_attendees_by_event(d1, &config.id)
        .await
        .map_err(AppError::Internal)?;
    let deposits = crate::db::thb_deposits::list_thb_deposits(d1, &config.id)
        .await
        .map_err(AppError::Internal)?;

    let (mut wants, wanted_qr_cells, wanted_checkin_cells) = attendee_wants(&attendees);
    let mut wanted_deposit_cells = 0;
    // Cash deposits an organizer approved, written as the approve path does.
    // Credit and comp rows are left alone: the live flow never wrote "THB"
    // for them, and inventing a value here is how `.issues/136` happened.
    for d in deposits
        .iter()
        .filter(|d| d.verified && d.source() == DepositSource::Cash)
    {
        push(&mut wants, &d.attendee_id, ColumnKey::DepositMethod, "THB");
        push(
            &mut wants,
            &d.attendee_id,
            ColumnKey::DepositAmount,
            &d.amount_thb.to_string(),
        );
        push(
            &mut wants,
            &d.attendee_id,
            ColumnKey::DepositVerified,
            "Yes",
        );
        wanted_deposit_cells += 3;
    }

    let kv = state.events_kv.as_ref();
    let mapping =
        crate::sheets::get_column_mapping(&state, &config.sheet_id, &config.sheet_name, kv)
            .await
            .map_err(AppError::Internal)?;
    let token = crate::sheets::get_cached_access_token(&state, kv)
        .await
        .map_err(AppError::Internal)?;
    let sheet_ref = crate::sheets::a1::sheet_ref(&config.sheet_name);
    let range = format!("{sheet_ref}!A1:{}", mapping.last_column_letter());
    let read_url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}",
        config.sheet_id,
        urlencoding::encode(&range)
    );
    let sheet = fetch_sheet_range(&read_url, &token)
        .await
        .map_err(AppError::Internal)?;

    let plan = plan_backfill(&sheet.values, &mapping, &wants);

    let mut written = 0;
    if query.apply && !plan.writes.is_empty() {
        let write_url = format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{}/values:batchUpdate",
            config.sheet_id
        );
        for chunk in plan.writes.chunks(WRITE_CHUNK) {
            let body = BatchUpdateRequest {
                data: chunk
                    .iter()
                    .map(|w| ValueRange {
                        range: format!("{sheet_ref}!{}{}", column_letter(w.column), w.row),
                        values: vec![vec![w.value.clone()]],
                    })
                    .collect(),
                value_input_option: "USER_ENTERED".to_string(),
            };
            batch_update_sheet(&write_url, &body, &token)
                .await
                .map_err(AppError::Internal)?;
            written += chunk.len();
        }
    }

    tracing::info!(
        event_id = %event_id,
        writes = plan.writes.len(),
        written,
        already_filled = plan.already_filled,
        row_missing = plan.row_missing,
        row_duplicated = plan.row_duplicated,
        column_missing = plan.column_missing,
        "sheet backfill done"
    );

    Ok(ApiOk::new(SheetBackfillResponse {
        event_id: config.id,
        applied: query.apply,
        attendees: attendees.len(),
        wanted_deposit_cells,
        wanted_qr_cells,
        wanted_checkin_cells,
        writes: plan.writes.len(),
        already_filled: plan.already_filled,
        row_missing: plan.row_missing,
        row_duplicated: plan.row_duplicated,
        column_missing: plan.column_missing,
        written,
    }))
}
