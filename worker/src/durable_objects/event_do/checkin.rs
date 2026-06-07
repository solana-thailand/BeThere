//! Check-in and claim handlers for the EventDurableObject.

use super::types::*;
use crate::durable_objects::event_do::EventDurableObject;

impl EventDurableObject {
    /// Check in an attendee — sets checked_in_at, checked_in_by, claim_token.
    /// Idempotent: if already checked in with the same claim_token, returns success.
    /// If checked in with a different claim_token, returns error.
    pub(super) fn handle_check_in(
        &self,
        attendee_id: &str,
        _event_id: &str,
        checked_in_at: &str,
        checked_in_by: &str,
        claim_token: &str,
    ) -> DoResponse {
        // Check existing state
        let existing: Option<ExistingCheckIn> = self
            .sql
            .exec(
                "SELECT checked_in_at, claim_token FROM attendees WHERE id = ?1",
                Some(vec![attendee_id.into()]),
            )
            .ok()
            .and_then(|cursor| cursor.one::<ExistingCheckIn>().ok());

        match existing {
            Some(row) if row.checked_in_at.is_some() => {
                // Already checked in — idempotent if same token
                if row.claim_token.as_deref() == Some(claim_token) {
                    tracing::info!(
                        attendee_id = %attendee_id,
                        "DO: check-in idempotent (already checked in)"
                    );
                    return DoResponse::ok();
                }
                return DoResponse::err("attendee is already checked in");
            }
            None => {
                return DoResponse::err("attendee not found");
            }
            _ => {} // Not checked in yet, proceed
        }

        let result = self.sql.exec(
            "UPDATE attendees \
             SET checked_in_at = ?1, checked_in_by = ?2, claim_token = ?3, \
                 updated_at = datetime('now') \
             WHERE id = ?4",
            Some(vec![
                checked_in_at.into(),
                checked_in_by.into(),
                claim_token.into(),
                attendee_id.into(),
            ]),
        );

        match result {
            Ok(cursor) => {
                let written = cursor.rows_written();
                if written > 0 {
                    tracing::info!(
                        attendee_id = %attendee_id,
                        rows_written = written,
                        "DO: attendee checked in"
                    );
                    self.sync_attendee_to_d1(attendee_id);
                    DoResponse::ok()
                } else {
                    DoResponse::err("check-in update matched 0 rows")
                }
            }
            Err(e) => DoResponse::err(format!("DO check_in: {e:?}")),
        }
    }

    /// Undo a check-in — clears checked_in_at, checked_in_by, claim_token.
    pub(super) fn handle_undo_check_in(&self, attendee_id: &str, _event_id: &str) -> DoResponse {
        let result = self.sql.exec(
            "UPDATE attendees \
             SET checked_in_at = NULL, checked_in_by = NULL, claim_token = NULL, \
                 updated_at = datetime('now') \
             WHERE id = ?1",
            Some(vec![attendee_id.into()]),
        );

        match result {
            Ok(cursor) => {
                tracing::info!(
                    attendee_id = %attendee_id,
                    rows_written = cursor.rows_written(),
                    "DO: check-in undone"
                );
                self.sync_attendee_to_d1(attendee_id);
                DoResponse::ok()
            }
            Err(e) => DoResponse::err(format!("DO undo_check_in: {e:?}")),
        }
    }

    /// Mark an attendee as claimed after NFT mint.
    pub(super) fn handle_claim_attendee(
        &self,
        event_id: &str,
        claim_token: &str,
        claimed_at: &str,
        claim_asset_id: &str,
        claim_signature: &str,
    ) -> DoResponse {
        let result = self.sql.exec(
            "UPDATE attendees \
             SET claimed_at = ?1, claim_asset_id = ?2, claim_signature = ?3, \
                 updated_at = datetime('now') \
             WHERE event_id = ?4 AND claim_token = ?5",
            Some(vec![
                claimed_at.into(),
                claim_asset_id.into(),
                claim_signature.into(),
                event_id.into(),
                claim_token.into(),
            ]),
        );

        match result {
            Ok(cursor) => {
                let written = cursor.rows_written();
                if written > 0 {
                    tracing::info!(rows_written = written, "DO: attendee claimed");
                    // Look up attendee_id for D1 sync
                    let id = self.resolve_attendee_id_by_claim_token(event_id, claim_token);
                    if let Some(id) = id {
                        self.sync_attendee_to_d1(&id);
                    }
                    DoResponse::ok()
                } else {
                    DoResponse::err("claim update matched 0 rows — attendee not found")
                }
            }
            Err(e) => DoResponse::err(format!("DO claim_attendee: {e:?}")),
        }
    }

    /// Insert or update an attendee. Idempotent upsert by `id`.
    pub(super) fn handle_upsert_attendee(&self, p: UpsertAttendeeParams<'_>) -> DoResponse {
        let result = self.sql.exec(
            "INSERT INTO attendees (id, event_id, email, name, approval_status, \
                 participation_type, contact_channel, contact_handle, \
                 created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now')) \
             ON CONFLICT (id) DO UPDATE SET \
                 name = excluded.name, \
                 approval_status = excluded.approval_status, \
                 participation_type = excluded.participation_type, \
                 contact_channel = excluded.contact_channel, \
                 contact_handle = excluded.contact_handle, \
                 updated_at = datetime('now')",
            Some(vec![
                p.id.into(),
                p.event_id.into(),
                p.email.into(),
                p.name.into(),
                p.approval_status.into(),
                p.participation_type.into(),
                p.contact_channel.into(),
                p.contact_handle.into(),
            ]),
        );

        match result {
            Ok(cursor) => {
                tracing::info!(
                    id = %p.id,
                    rows_written = cursor.rows_written(),
                    "DO: attendee upserted"
                );
                self.sync_attendee_to_d1(p.id);
                DoResponse::ok()
            }
            Err(e) => DoResponse::err(format!("DO upsert_attendee: {e:?}")),
        }
    }

    /// Resolve attendee id by (event_id, claim_token).
    fn resolve_attendee_id_by_claim_token(
        &self,
        event_id: &str,
        claim_token: &str,
    ) -> Option<String> {
        self.sql
            .exec(
                "SELECT id FROM attendees WHERE event_id = ?1 AND claim_token = ?2",
                Some(vec![event_id.into(), claim_token.into()]),
            )
            .ok()
            .and_then(|cursor| cursor.one::<IdRow>().ok())
            .map(|r| r.id)
    }
}
