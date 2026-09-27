//! Roster "Possible duplicate" badge (plan 025 §7.9). The worker flags rows
//! in this event that share a name, wallet or contact handle under another
//! email; the badge says with whom and why, so staff can link the emails or
//! turn one row away before check-in. It blocks nothing.

use event_checkin_domain::models::attendee::{DuplicateMatch, DuplicateReason};
use leptos::prelude::*;

fn reason_label(reason: DuplicateReason) -> &'static str {
    match reason {
        DuplicateReason::SameName => "same name as",
        DuplicateReason::SameWallet => "same wallet as",
        DuplicateReason::SameHandle => "same contact handle as",
    }
}

/// One line per match, e.g. "same wallet as Somchai J.".
pub fn duplicate_hint_title(matches: &[DuplicateMatch]) -> String {
    matches
        .iter()
        .map(|m| format!("{} {}", reason_label(m.reason), m.name))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The badge, or nothing when the row matches no other row.
pub fn duplicate_hint(matches: &[DuplicateMatch]) -> Option<AnyView> {
    if matches.is_empty() {
        return None;
    }
    let title = format!(
        "Possible duplicate — check before check-in; link the emails if it is one person.\n{}",
        duplicate_hint_title(matches)
    );
    Some(
        view! {
            <span class="badge badge-warning" title=title>
                "⚠ Possible duplicate"
            </span>
        }
        .into_any(),
    )
}
