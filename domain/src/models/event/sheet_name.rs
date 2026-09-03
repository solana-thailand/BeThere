//! Validation for the organiser-supplied Google Sheets tab names.
//!
//! `sheet_name` / `staff_sheet_name` are free text on the event form. Quoting
//! them correctly in an A1 range (worker `sheets::a1::sheet_ref`) makes any
//! *legal* name work, but it cannot make an *illegal* one exist: Google refuses
//! to create a tab whose name is over 100 characters, contains `[ ] * ? / \`,
//! or starts or ends with an apostrophe. A name like that is accepted by the
//! form, stored, and then fails on every Sheets call — silently, because nearly
//! all of those calls are detached best-effort work whose errors are logged and
//! dropped. The organiser just sees a tab that never fills in.
//!
//! Rejecting at the point of entry turns that into a 400 on the form, next to
//! the field that caused it. This module is the single definition of the rule
//! so the worker and the Leptos form cannot drift apart.

/// Default tab holding attendee rows, when the organiser leaves the field blank.
pub const DEFAULT_ATTENDEE_SHEET_NAME: &str = "Attendees";

/// Default tab holding staff rows, when the organiser leaves the field blank.
pub const DEFAULT_STAFF_SHEET_NAME: &str = "staff";

/// Google's cap on a tab name, in characters (not bytes — Thai and emoji tab
/// names are common here and count as one each).
pub const MAX_SHEET_NAME_CHARS: usize = 100;

/// Characters Google refuses inside a tab name.
const FORBIDDEN_CHARS: [char; 6] = ['[', ']', '*', '?', '/', '\\'];

/// Trim `raw`, fall back to `default` when it is blank, and reject anything
/// Google would refuse to name a tab.
///
/// The returned name is what should be persisted: callers must not keep the
/// untrimmed original, or the stored name will not match the tab title the API
/// reports back.
///
/// # Errors
///
/// Returns a message written for the organiser looking at the form field.
pub fn normalize_sheet_name(raw: &str, default: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Ok(default.to_string());
    }

    let chars = name.chars().count();
    if chars > MAX_SHEET_NAME_CHARS {
        return Err(format!(
            "sheet name is {chars} characters; Google allows at most {MAX_SHEET_NAME_CHARS}"
        ));
    }

    if let Some(bad) = name.chars().find(|c| FORBIDDEN_CHARS.contains(c)) {
        return Err(format!(
            "sheet name cannot contain '{bad}' — Google rejects [ ] * ? / \\ in tab names"
        ));
    }

    // Control characters cannot be typed into a tab title at all; they arrive
    // from a paste or a scripted request and would otherwise reach a URL path.
    if name.chars().any(char::is_control) {
        return Err("sheet name cannot contain control characters".to_string());
    }

    // A leading or trailing `'` is Google's own rule, and it is the shape an
    // organiser produces by pasting the already-quoted form (`'Day 1'`) out of
    // a formula. Left alone it would address a tab literally named `'Day 1'`,
    // which cannot exist.
    if name.starts_with('\'') || name.ends_with('\'') {
        return Err(
            "sheet name cannot start or end with an apostrophe — enter the tab name without quotes"
                .to_string(),
        );
    }

    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_falls_back_to_the_default() {
        for raw in ["", "   ", "\t\n"] {
            assert_eq!(
                normalize_sheet_name(raw, DEFAULT_ATTENDEE_SHEET_NAME).unwrap(),
                DEFAULT_ATTENDEE_SHEET_NAME
            );
        }
    }

    #[test]
    fn the_shipped_defaults_pass() {
        for name in [DEFAULT_ATTENDEE_SHEET_NAME, DEFAULT_STAFF_SHEET_NAME] {
            assert_eq!(normalize_sheet_name(name, "x").unwrap(), name);
        }
    }

    #[test]
    fn legal_but_unusual_names_pass_and_are_trimmed() {
        // These all need A1 quoting, which is a separate concern; the tabs
        // themselves are legal, so validation must not reject them.
        for (raw, want) in [
            ("Attendee List", "Attendee List"),
            ("  Day 1  ", "Day 1"),
            ("walk-ins", "walk-ins"),
            ("Bob's tab", "Bob's tab"),
            ("ผู้เข้าร่วม", "ผู้เข้าร่วม"),
            ("2026", "2026"),
            ("a:b", "a:b"),
        ] {
            assert_eq!(normalize_sheet_name(raw, "x").unwrap(), want);
        }
    }

    #[test]
    fn names_google_would_refuse_are_rejected() {
        for raw in ["day[1]", "a/b", "a\\b", "why?", "star*", "'Day 1'", "x'"] {
            assert!(
                normalize_sheet_name(raw, "x").is_err(),
                "{raw} should be rejected"
            );
        }
    }

    #[test]
    fn the_length_cap_counts_characters_not_bytes() {
        let thai = "ก".repeat(MAX_SHEET_NAME_CHARS);
        assert_eq!(normalize_sheet_name(&thai, "x").unwrap(), thai);

        let too_long = "ก".repeat(MAX_SHEET_NAME_CHARS + 1);
        assert!(normalize_sheet_name(&too_long, "x").is_err());
    }

    #[test]
    fn control_characters_are_rejected() {
        // Would otherwise reach a Sheets URL path via the `:append` endpoints.
        assert!(normalize_sheet_name("Attendees\nstaff", "x").is_err());
        assert!(normalize_sheet_name("Attendees\u{0}", "x").is_err());
    }
}
