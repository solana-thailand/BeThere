//! Event management API handlers (Issue 004 — Multi-event / Organizer support).
//!
//! Protected endpoints (require admin auth):
//!   GET    /api/events               — list all events
//!   POST   /api/events               — create a new event
//!   POST   /api/events/seed          — seed first event from env vars (super admin only)
//!   POST   /api/events/migrate       — migrate quiz data from QUIZ to EVENTS namespace (super admin only)
//!   GET    /api/events/{id}          — get event details
//!   PUT    /api/events/{id}          — update event config
//!   DELETE /api/events/{id}          — archive (soft-delete) event
//!   POST   /api/events/{id}/duplicate — copy event settings into a new Draft (Issue #055)
//!   POST   /api/events/reseed-kv     — reseed KV index from D1 (super admin only)

pub mod audit;
pub mod create;
pub mod duplicate;
pub mod lifecycle;
pub mod list;
pub mod read;
pub mod seed;
pub mod sync;
pub mod update;

pub use audit::{get_event_audit, get_form_config, get_global_audit, put_form_config};
pub use create::create_event;
pub use duplicate::duplicate_event;
pub use lifecycle::{archive_event, hard_delete_event, restore_event};
pub use list::list_events;
pub use read::get_event;
pub use seed::{migrate_quiz, reseed_kv_from_d1, seed_event};
pub use sync::sync_sheet_to_d1;
pub use update::update_event;
