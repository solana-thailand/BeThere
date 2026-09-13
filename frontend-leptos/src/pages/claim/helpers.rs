//! Small formatting and step-computation helpers.

use crate::utils::format_timestamp;

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Format seconds into "Xh Xm Xs" or "Xm Xs" or "Xs".
pub(super) fn format_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
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

/// Build the appropriate label for check-in status.
/// For online attendees without check-in: "Registered".
/// For checked-in attendees: "Checked in {timestamp}".
/// For others without check-in: "Not yet checked in".
pub(super) fn checked_in_label(checked_in_at: &str, participation_type: &str) -> String {
    if checked_in_at.is_empty() || checked_in_at == "N/A" {
        if is_online_participant(participation_type) {
            return "Registered".to_string();
        }
        return "Not yet checked in".to_string();
    }
    format!("Checked in {}", format_timestamp(checked_in_at))
}
