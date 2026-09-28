//! Small formatting and step-computation helpers.

use leptos::prelude::*;

use crate::i18n::{Locale, t, td_string, use_i18n};
use crate::pages::ticket::view_data::format_check_in_time;

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Format seconds into "Xh Xm Xs" or "Xm Xs" or "Xs", with the unit words of
/// `locale`.
pub(super) fn format_duration(secs: i64, locale: Locale) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let uh = td_string!(locale, claim.unit.h);
    let um = td_string!(locale, claim.unit.m);
    let us = td_string!(locale, claim.unit.s);
    if h > 0 {
        format!("{h}{uh} {m}{um} {s}{us}")
    } else if m > 0 {
        format!("{m}{um} {s}{us}")
    } else {
        format!("{s}{us}")
    }
}

/// Simple deterministic hash for generating avatar colors from name.
pub(super) fn simple_hash(s: &str) -> u32 {
    let mut hash: u32 = 0;
    for b in s.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u32);
    }
    hash
}

/// Check if participation type indicates an online attendee.
pub(super) fn is_online_participant(participation_type: &str) -> bool {
    let lower = participation_type.trim().to_lowercase();
    lower.contains("online")
}

/// The check-in status line, in the attendee's language.
/// For online attendees without check-in: "Registered".
/// For checked-in attendees: "Checked in {timestamp}".
/// For others without check-in: "Not yet checked in".
pub(super) fn checked_in_label(checked_in_at: &str, participation_type: &str) -> AnyView {
    let i18n = use_i18n();
    if checked_in_at.is_empty() || checked_in_at == "N/A" {
        if is_online_participant(participation_type) {
            return t!(i18n, claim.registered).into_any();
        }
        return t!(i18n, claim.not_checked_in).into_any();
    }
    let iso = checked_in_at.to_string();
    let time = move || format_check_in_time(&iso);
    t!(i18n, claim.checked_in_at, time).into_any()
}
