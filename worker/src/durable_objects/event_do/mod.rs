//! EventDurableObject — per-event Durable Object with SQLite storage.
//!
//! Each event gets its own DO instance (sharded by `event_id` as DO name).
//! SQLite inside the DO provides single-threaded, ACID-guaranteed writes.
//! After each write, the changed row is synced to D1 for read availability.

mod checkin;
mod claim_lock;
mod schema;
mod sync;
mod tests;
mod types;

use worker::{DurableObject, Env, Request, Response, Result, State, durable_object};

pub(crate) use types::DoRequest;
pub(crate) use types::UpsertAttendeeParams;

#[durable_object]
pub struct EventDurableObject {
    sql: worker::SqlStorage,
    env: Env,
}

impl DurableObject for EventDurableObject {
    fn new(state: State, env: Env) -> Self {
        let sql = state.storage().sql();
        schema::init_schema(&sql);
        Self { sql, env }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let body: DoRequest = req.json().await?;
        let result = match body {
            // Phase 1: Claim lock operations
            DoRequest::AcquireClaimLock {
                lock_id,
                event_id,
                token,
                wallet,
                expires_at,
            } => self.handle_acquire_claim_lock(&lock_id, &event_id, &token, &wallet, &expires_at),

            DoRequest::FinalizeClaimLock {
                event_id,
                token,
                asset_id,
                signature,
                claimed_at,
            } => self.handle_finalize_claim_lock(
                &event_id,
                &token,
                &asset_id,
                &signature,
                &claimed_at,
            ),

            DoRequest::ReleaseClaimLock { event_id, token } => {
                self.handle_release_claim_lock(&event_id, &token)
            }

            // Phase 2: Check-in & claim operations
            DoRequest::CheckIn {
                attendee_id,
                event_id,
                checked_in_at,
                checked_in_by,
                claim_token,
            } => self.handle_check_in(
                &attendee_id,
                &event_id,
                &checked_in_at,
                &checked_in_by,
                &claim_token,
            ),

            DoRequest::UndoCheckIn {
                attendee_id,
                event_id,
            } => self.handle_undo_check_in(&attendee_id, &event_id),

            DoRequest::ClaimAttendee {
                event_id,
                claim_token,
                claimed_at,
                claim_asset_id,
                claim_signature,
            } => self.handle_claim_attendee(
                &event_id,
                &claim_token,
                &claimed_at,
                &claim_asset_id,
                &claim_signature,
            ),

            DoRequest::UpsertAttendee {
                id,
                event_id,
                email,
                name,
                approval_status,
                participation_type,
                contact_channel,
                contact_handle,
            } => self.handle_upsert_attendee(UpsertAttendeeParams {
                id: &id,
                event_id: &event_id,
                email: &email,
                name: &name,
                approval_status: &approval_status,
                participation_type: &participation_type,
                contact_channel: &contact_channel,
                contact_handle: &contact_handle,
            }),
        };
        Response::from_json(&result)
    }
}
