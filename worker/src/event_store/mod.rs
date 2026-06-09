//! KV-based event storage for multi-event / organizer support (Issue 004).
//!
//! Events are stored in a Cloudflare KV namespace bound as `EVENTS`:
//!
//!   "events"                         → EventIndex (JSON) — list of EventMeta
//!   "event:{id}"                     → EventConfig (JSON) — full per-event config
//!
//! Per-event quiz data uses the same namespace with prefixed keys:
//!   "event:{id}:quiz:questions"      → QuizConfig (JSON)
//!   "event:{id}:quiz:progress:{tok}" → QuizProgress (JSON)

pub mod read;
pub mod schema;
pub mod write;

// ---------------------------------------------------------------------------
// Re-export all public items so existing `use crate::event_store::*` works
// ---------------------------------------------------------------------------

// Schema (key helpers — only export what's used externally)
pub use schema::{deposit_status_key, quiz_progress_key, quiz_questions_key, thb_deposit_key};

// Read operations
pub use read::{
    find_attendee_by_wallet, get_deposit_status, get_deposit_status_with_fallback, get_event,
    get_event_config, get_event_config_with_fallback, get_event_id_by_escrow, get_event_index,
    get_form_config, get_thb_deposit, has_event_access, increment_deposit_counter_with_fallback,
    is_event_organizer, is_event_staff, list_deposit_statuses, list_events, list_thb_deposits,
    resolve_event_by_slug, resolve_event_or_fallback, save_deposit_status_with_fallback,
};

// Write operations
pub use write::{
    apply_update, archive_event, create_event, hard_delete_event, migrate_quiz_to_event,
    restore_event, save_deposit_status, save_escrow_index, save_event_config, save_event_index,
    save_form_config, save_thb_deposit, seed_from_config, seed_kv_from_d1,
    sync_delete_event_from_d1, sync_event_to_d1, update_event,
};
