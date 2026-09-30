//! View-state enums and small pure helpers for the campaigns page.

// ===== Promote-from-event payload =====

/// Payload produced by `EventsPage` when an organizer chooses to promote an
/// existing event into a new campaign. Consumed by `CampaignsPage` on mount
/// to pre-fill the create form and auto-link the source event.
#[derive(Debug, Clone, Default)]
pub struct PromoteEventPayload {
    pub event_id: String,
    pub event_name: String,
}

// ===== View State =====

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum CampaignView {
    List,
    Create,
    Edit,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum DetailTab {
    Events,
    Progress,
    Stats,
}

/// Availability of the campaign id (slug) currently in the create form.
///
/// Answers are advisory except for [`SlugStatus::Taken`], which is a certain
/// save failure and so blocks submission.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum SlugStatus {
    /// Never checked, or the slug changed since the last answer arrived.
    Unchecked,
    Checking,
    Available,
    Taken,
    /// Rejected by the server as malformed (not `[A-Za-z0-9_-]`, or too long).
    Malformed,
    /// The probe itself failed (offline, 5xx). Never blocks saving.
    CheckFailed,
}

// ===== Helper =====

pub(super) fn status_badge_class(status: &str) -> &'static str {
    match status {
        "active" => "badge badge-success",
        "completed" => "badge badge-info",
        _ => "badge badge-warning",
    }
}

/// Convert a free-form title into a URL-safe kebab-case slug.
/// Lowercases; replaces runs of non-[a-z0-9] with '-'; trims leading/trailing
/// dashes; caps length at 60 chars. Returns empty string for empty/whitespace input.
pub(super) fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > 60 {
        out.truncate(60);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- slugify (plan 016 P0.1) -------------------------------------------

    #[test]
    fn slugify_produces_kebab_case() {
        assert_eq!(
            slugify("Solana Hacker Series 2025"),
            "solana-hacker-series-2025"
        );
    }

    #[test]
    fn slugify_collapses_runs_and_trims() {
        assert_eq!(slugify("  Hello --- World!!  "), "hello-world");
    }

    #[test]
    fn slugify_returns_empty_for_blank_input() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("   "), "");
        assert_eq!(slugify("!!!"), "");
    }

    #[test]
    fn slugify_caps_length_without_trailing_dash() {
        let long = "a".repeat(80);
        assert_eq!(slugify(&long).len(), 60);
        // A cut landing on a separator must not leave a dangling dash.
        let awkward = format!("{} x", "a".repeat(59));
        let out = slugify(&awkward);
        assert!(!out.ends_with('-'), "slug ended with a dash: {out}");
    }
}
