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
    find_attendee_by_wallet, get_deposit_status, get_event, get_event_config,
    get_event_id_by_escrow, get_event_index, get_thb_deposit, has_event_access, is_event_organizer,
    is_event_staff, list_deposit_statuses, list_events, list_thb_deposits,
    resolve_event_or_fallback,
};

// Write operations
pub use write::{
    archive_event, create_event, hard_delete_event, increment_deposit_counter,
    migrate_quiz_to_event, restore_event, save_deposit_status, save_escrow_index, save_event_index,
    save_thb_deposit, seed_from_config, update_event,
};
