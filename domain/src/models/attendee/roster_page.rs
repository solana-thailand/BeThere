//! Paging the approved roster behind `GET /api/attendees` (`.issues/151` C).
//!
//! The cursor used to be the `row_index` of the last item. `row_index` comes
//! from D1's `sheet_row_index`, which is 0 for every attendee D1 created, so
//! every row had the same key: page 2 (`row_index > 0`) came back empty and
//! everyone after the 200th approved attendee vanished. The cursor is now an
//! offset into a total order, `(row_index, api_id)`, which no tie can break.

use super::core::Attendee;

/// Largest page the roster endpoint serves, and its default.
pub const ROSTER_PAGE_MAX: usize = 200;

/// One page of approved attendees.
#[derive(Debug)]
pub struct RosterPage<'a> {
    pub items: Vec<&'a Attendee>,
    /// Offset to pass as `cursor` for the next page; `None` on the last one.
    pub next_cursor: Option<usize>,
}

/// The approved attendees at `[cursor, cursor + limit)` of the roster order.
///
/// `limit` is clamped to `1..=ROSTER_PAGE_MAX`, so a `limit=0` request can
/// never hand back a cursor that does not advance.
pub fn roster_page(attendees: &[Attendee], cursor: Option<usize>, limit: usize) -> RosterPage<'_> {
    let limit = limit.clamp(1, ROSTER_PAGE_MAX);
    let mut approved: Vec<&Attendee> = attendees.iter().filter(|a| a.is_approved()).collect();
    approved.sort_unstable_by(|a, b| {
        (a.row_index, a.api_id.as_str()).cmp(&(b.row_index, b.api_id.as_str()))
    });

    let start = cursor.unwrap_or(0).min(approved.len());
    let end = start.saturating_add(limit).min(approved.len());
    let next_cursor = match end < approved.len() {
        true => Some(end),
        false => None,
    };
    approved.truncate(end);
    approved.drain(..start);
    RosterPage {
        items: approved,
        next_cursor,
    }
}
