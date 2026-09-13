use axum::Extension;
use axum::extract::{Query, State};
use serde::Deserialize;
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 100;

#[derive(Debug, Default, Deserialize)]
pub struct ListEventsQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

fn page_range<T>(
    items: &[T],
    cursor: Option<&str>,
    limit: usize,
    id: impl Fn(&T) -> &str,
) -> Result<(std::ops::Range<usize>, Option<String>), AppError> {
    if !(1..=MAX_PAGE_SIZE).contains(&limit) {
        return Err(AppError::Validation(format!(
            "limit must be between 1 and {MAX_PAGE_SIZE}"
        )));
    }
    // Bind offsets to the exact ordered snapshot. Inserts, deletes, or reorders
    // expire the cursor instead of silently duplicating or skipping events.
    let mut snapshot = 0xcbf29ce484222325_u64;
    for item in items {
        for byte in id(item).as_bytes().iter().chain(std::iter::once(&0)) {
            snapshot ^= u64::from(*byte);
            snapshot = snapshot.wrapping_mul(0x100000001b3);
        }
    }
    let snapshot = format!("{snapshot:016x}");
    let start = match cursor {
        None => 0,
        Some(cursor) => {
            let cursor = cursor
                .strip_prefix("kv.")
                .ok_or_else(|| AppError::Validation("invalid event cursor".into()))?;
            let (offset, supplied_snapshot) = cursor
                .split_once('.')
                .ok_or_else(|| AppError::Validation("invalid event cursor".into()))?;
            if supplied_snapshot != snapshot {
                return Err(AppError::Validation("expired event cursor".into()));
            }
            let offset = offset
                .parse::<usize>()
                .map_err(|_| AppError::Validation("invalid event cursor".into()))?;
            if offset > items.len() {
                return Err(AppError::Validation("expired event cursor".into()));
            }
            offset
        }
    };
    let end = start.saturating_add(limit).min(items.len());
    let next_cursor = (end < items.len() && end > start).then(|| format!("kv.{end}.{snapshot}"));
    Ok((start..end, next_cursor))
}

/// GET /api/events
/// List events visible to the current user.
///
/// - **SuperAdmin**: sees all events
/// - **Organizer/Staff**: sees only events they are assigned to
///   (matched by `organizer_emails` or `staff_emails` in event config,
///   or by Google Sheet staff role)
///
/// Returns events sorted by creation date (newest first).
#[worker::send]
pub async fn list_events(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListEventsQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_fingerprint = %state.log_fingerprint(&claims.email), "list events requested");

    let limit = query.limit.unwrap_or(DEFAULT_PAGE_SIZE);
    if !(1..=MAX_PAGE_SIZE).contains(&limit) {
        return Err(
            AppError::Validation(format!("limit must be between 1 and {MAX_PAGE_SIZE}")).into(),
        );
    }
    let is_super_admin = state
        .config
        .super_admin_emails
        .iter()
        .any(|email| email.eq_ignore_ascii_case(&claims.email));

    // D1 is the primary event store and can apply authorization plus keyset
    // pagination in one bounded query. KV remains the availability fallback.
    let kv_cursor = query
        .cursor
        .as_deref()
        .is_some_and(|cursor| cursor.starts_with("kv."));
    if let Some(ref d1) = state.d1
        && !kv_cursor
    {
        let email_filter = (!is_super_admin).then_some(claims.email.as_str());
        match crate::db::events::list_event_meta_page(
            d1,
            email_filter,
            query.cursor.as_deref(),
            limit,
        )
        .await
        {
            Ok(page) => {
                return Ok(ApiOk::new(json!({
                    "events": page.events,
                    "next_cursor": page.next_cursor,
                })));
            }
            Err(crate::db::events::EventPageError::InvalidCursor) => {
                return Err(AppError::Validation("invalid or expired event cursor".into()).into());
            }
            Err(crate::db::events::EventPageError::Storage(error)) if query.cursor.is_none() => {
                tracing::warn!(error = %error, "D1 event page failed, falling back to KV");
            }
            Err(crate::db::events::EventPageError::Storage(error)) => {
                return Err(AppError::Internal(format!("failed to list events: {error}")).into());
            }
        }
    }

    let kv = state.events_kv.as_ref();
    let all_events = if let Some(kv_ref) = kv {
        crate::event_store::list_events(kv_ref).await.map_err(|e| {
            tracing::error!(error = %e, "failed to list events");
            AppError::Internal(format!("failed to list events: {e}"))
        })?
    } else {
        return Err(AppError::Internal("event storage unavailable".into()).into());
    };

    let (range, next_cursor) = page_range(&all_events, query.cursor.as_deref(), limit, |event| {
        event.id.as_str()
    })?;
    let page = &all_events[range];

    // SuperAdmin sees everything in this bounded page.
    if is_super_admin {
        return Ok(ApiOk::new(json!({
            "events": page,
            "next_cursor": next_cursor,
        })));
    }

    // Organizer/Staff: only see events they are assigned to.
    // EventMeta only has organizer_emails, not staff_emails.
    // We must load full configs to check both lists.
    let mut visible = Vec::new();
    for meta in page {
        // Quick check: organizer_emails is in meta (no need to load full config)
        let in_organizer_list = meta
            .organizer_emails
            .iter()
            .any(|e| e.eq_ignore_ascii_case(&claims.email));

        if in_organizer_list {
            visible.push(meta.clone());
            continue;
        }

        // Slower check: load full config to check staff_emails
        // KV first, then D1 fallback
        if let Some(kv_ref) = kv
            && let Ok(Some(config)) = crate::event_store::get_event_config(kv_ref, &meta.id).await
            && crate::event_store::has_event_access(&config, &claims.email)
        {
            visible.push(meta.clone());
        } else if let Some(ref d1) = state.d1 {
            // D1 fallback for staff check when event is not in KV
            if let Ok(Some(row)) = crate::db::events::get_event(d1, &meta.id).await {
                let config = row.to_event_config();
                if crate::event_store::has_event_access(&config, &claims.email) {
                    visible.push(meta.clone());
                }
            }
        }
    }

    Ok(ApiOk::new(json!({
        "events": visible,
        "next_cursor": next_cursor,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_pages_are_bounded_stable_and_non_overlapping() {
        let ids = vec!["new", "middle", "old"];
        let (first, next) = page_range(&ids, None, 2, |id| *id).unwrap();
        assert_eq!(&ids[first], &["new", "middle"]);
        assert!(next.as_deref().unwrap().starts_with("kv.2."));
        let (second, next) = page_range(&ids, next.as_deref(), 2, |id| *id).unwrap();
        assert_eq!(&ids[second], &["old"]);
        assert!(next.is_none());
    }

    #[test]
    fn cursor_and_limit_fail_closed() {
        let ids = vec!["event"];
        assert!(page_range(&ids, Some("missing"), 1, |id| *id).is_err());
        assert!(page_range(&ids, Some("kv.2.bad"), 1, |id| *id).is_err());
        assert!(page_range(&ids, None, 0, |id| *id).is_err());
        assert!(page_range(&ids, None, MAX_PAGE_SIZE + 1, |id| *id).is_err());
    }

    #[test]
    fn cursor_expires_when_the_ordered_snapshot_changes() {
        let ids = vec!["new", "old"];
        let (_, cursor) = page_range(&ids, None, 1, |id| *id).unwrap();
        let changed = vec!["newer", "new", "old"];
        assert!(page_range(&changed, cursor.as_deref(), 1, |id| *id).is_err());
    }
}
