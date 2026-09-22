//! The ticket names this system writes for attendees it creates itself.
//!
//! These were magic strings repeated across four writers before `.issues/136`,
//! and that is exactly how the two halves of the system came to disagree: the
//! Google Sheet appends wrote `"Self-Registered"` and `"Walk-in"`, D1 wrote
//! nothing at all because it had no column, and the D1 read path — which is the
//! one almost every screen actually uses ([[d1-first-not-sheets-first]]) —
//! filled the gap with the attendee's *name*. An organizer then saw anyone
//! called Vipada wearing a VIP badge.
//!
//! Naming them here does not by itself prevent that. What it enables is the
//! guard in `worker/tests/ticket_name_guards.rs`, which can now assert that the
//! sheet writer and the D1 writer for the same flow use the same constant,
//! because there is a single thing to point at.
//!
//! Anything NOT in this list came from the event's own ticketing — the sheet's
//! `ticket_name` column, filled by whoever imported the guest list. That is the
//! interesting case (a real "VIP", "Speaker", "Sponsor" tier) and this module
//! deliberately says nothing about it.

/// An attendee who signed themselves up through the public registration form.
///
/// Written by both halves of that flow: the Sheets append
/// (`sheets::write::append` and `sheets::bg_sync`) and the D1 upsert
/// (`db::attendees::upsert_attendee`).
pub const TICKET_NAME_SELF_REGISTERED: &str = "Self-Registered";

/// Someone admitted at the door with no prior registration.
///
/// Written by the Sheets append and by `db::attendees::try_insert_walkin`.
/// The admin list's Walk-in badge compares against this exact string with
/// `eq_ignore_ascii_case`, so it is not cosmetic.
pub const TICKET_NAME_WALK_IN: &str = "Walk-in";

/// Every ticket name this system mints for itself.
///
/// Useful for asking "did a human choose this, or did we?" — which is the
/// question a real ticket-tier check wants to start from.
pub const SYSTEM_TICKET_NAMES: [&str; 2] = [TICKET_NAME_SELF_REGISTERED, TICKET_NAME_WALK_IN];

/// Whether a ticket name is one this system generated rather than one the
/// organizer assigned.
pub fn is_system_ticket_name(ticket_name: &str) -> bool {
    SYSTEM_TICKET_NAMES
        .iter()
        .any(|known| known.eq_ignore_ascii_case(ticket_name.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Walk-in badge in `frontend-leptos/src/pages/admin.rs` compares with
    /// `eq_ignore_ascii_case("Walk-in")`. If this constant is ever renamed, the
    /// badge silently stops appearing — there is no type connecting them.
    #[test]
    fn the_walk_in_constant_is_what_the_admin_badge_compares_against() {
        assert!(TICKET_NAME_WALK_IN.eq_ignore_ascii_case("Walk-in"));
    }

    /// Neither system name may contain "vip", because the admin list flags a
    /// VIP with `ticket_name.to_lowercase().contains("vip")`. This is cheap
    /// insurance against someone renaming these to something like
    /// "Self-Registered (VIP eligible)" and turning every self-registration
    /// into a VIP overnight.
    #[test]
    fn no_system_ticket_name_reads_as_a_vip() {
        for name in SYSTEM_TICKET_NAMES {
            assert!(
                !name.to_lowercase().contains("vip"),
                "{name:?} would make every attendee in that flow a VIP — see .issues/136"
            );
        }
    }

    #[test]
    fn system_names_are_recognised_and_organizer_tiers_are_not() {
        assert!(is_system_ticket_name("Walk-in"));
        assert!(is_system_ticket_name(" self-registered "));
        assert!(!is_system_ticket_name("VIP"));
        assert!(!is_system_ticket_name("Speaker"));
        assert!(!is_system_ticket_name(""));
    }
}
