//! Possible-duplicate hints for the roster (plan 025 §5, task 7.9).
//!
//! Linking emails is voluntary, so two rows for one person under different
//! emails stay two rows until someone notices. This flags rows in one event
//! that share a normalized name, a Solana wallet, or a contact handle while
//! their emails differ, so staff can link them, or turn one away, before
//! check-in. It is a hint and nothing more: it blocks nothing, and two real
//! people can share a common name.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::core::Attendee;

/// What two rows have in common.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateReason {
    SameName,
    SameWallet,
    SameHandle,
}

impl DuplicateReason {
    fn key(self, attendee: &Attendee) -> Option<String> {
        match self {
            DuplicateReason::SameName => name_key(attendee),
            DuplicateReason::SameWallet => wallet_key(attendee),
            DuplicateReason::SameHandle => handle_key(attendee),
        }
    }
}

/// Another row in the same event that this row may duplicate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateMatch {
    pub api_id: String,
    pub name: String,
    pub reason: DuplicateReason,
}

/// Lowercased, whitespace collapsed. Names shorter than two characters are
/// too weak a signal to flag on.
fn name_key(attendee: &Attendee) -> Option<String> {
    let key = attendee
        .name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    (key.chars().count() >= 2).then_some(key)
}

/// Exact string: base58 is case-sensitive.
fn wallet_key(attendee: &Attendee) -> Option<String> {
    let wallet = attendee.solana_address.as_deref()?.trim();
    (!wallet.is_empty()).then(|| wallet.to_string())
}

/// Channel plus handle, lowercased, leading `@` dropped: Telegram, GitHub and
/// X handles are case-insensitive.
fn handle_key(attendee: &Attendee) -> Option<String> {
    let handle = attendee
        .contact_handle
        .as_deref()?
        .trim()
        .trim_start_matches('@')
        .to_lowercase();
    if handle.is_empty() {
        return None;
    }
    let channel = attendee
        .contact_channel
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    Some(format!("{channel}:{handle}"))
}

/// For each row that may duplicate another, the rows it matches and why.
/// Rows with the same email are skipped: they are one person by definition,
/// and registration dedup already handles them.
pub fn possible_duplicates(attendees: &[Attendee]) -> HashMap<String, Vec<DuplicateMatch>> {
    const REASONS: [DuplicateReason; 3] = [
        DuplicateReason::SameName,
        DuplicateReason::SameWallet,
        DuplicateReason::SameHandle,
    ];

    let mut groups: HashMap<(DuplicateReason, String), Vec<usize>> = HashMap::new();
    for (index, attendee) in attendees.iter().enumerate() {
        for reason in REASONS {
            if let Some(key) = reason.key(attendee) {
                groups.entry((reason, key)).or_default().push(index);
            }
        }
    }

    let mut matches: HashMap<String, Vec<DuplicateMatch>> = HashMap::new();
    for ((reason, _), members) in groups.into_iter().filter(|(_, m)| m.len() > 1) {
        for &a in &members {
            for &b in &members {
                let (this, other) = (&attendees[a], &attendees[b]);
                if a == b || this.email.trim().eq_ignore_ascii_case(other.email.trim()) {
                    continue;
                }
                matches
                    .entry(this.api_id.clone())
                    .or_default()
                    .push(DuplicateMatch {
                        api_id: other.api_id.clone(),
                        name: other.display_name().to_string(),
                        reason,
                    });
            }
        }
    }
    // HashMap iteration order is random; keep the wire output stable.
    for list in matches.values_mut() {
        list.sort_by(|x, y| (x.reason, &x.api_id).cmp(&(y.reason, &y.api_id)));
    }
    matches
}
