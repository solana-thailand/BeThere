//! Shared utility functions extracted from scanner and admin pages.

/// Result of parsing a participation type string into a display badge.
#[derive(Debug, Clone)]
pub struct ParticipationBadge {
    /// Short display label (e.g. "In-Person", "Online").
    pub label: String,
    /// CSS class for the badge element.
    pub css_class: &'static str,
}

/// Parse a participation type string into a badge for display.
///
/// Handles long values like "In-Person (Bangkok): Attend at the venue..."
/// by matching on substrings, matching the legacy JS `getParticipationBadge()`.
pub fn get_participation_badge(participation_type: &str) -> ParticipationBadge {
    if participation_type.is_empty() {
        return ParticipationBadge {
            label: "Unknown".to_string(),
            css_class: "badge-warning",
        };
    }

    let lower = participation_type.to_lowercase();

    if lower.contains("in-person") || lower.contains("in person") {
        return ParticipationBadge {
            label: "In-Person".to_string(),
            css_class: "badge-info",
        };
    }

    if lower.contains("online") || lower.contains("virtual") {
        return ParticipationBadge {
            label: "Online".to_string(),
            css_class: "badge-warning",
        };
    }

    // Fallback: take text before colon or slash
    let label = participation_type
        .split(':')
        .next()
        .unwrap_or(participation_type)
        .split('/')
        .next()
        .unwrap_or(participation_type)
        .trim()
        .to_string();

    ParticipationBadge {
        label,
        css_class: "badge-warning",
    }
}

/// Format an ISO 8601 timestamp to a human-readable locale string.
///
/// Returns "N/A" for empty strings and the raw input if parsing fails.
pub fn format_timestamp(iso: &str) -> String {
    if iso.is_empty() {
        return "N/A".to_string();
    }

    let js_date = js_sys::Date::new_with_year_month_day_hr_min_sec(0, 0, 0, 0, 0, 0);
    js_date.set_time(js_sys::Date::parse(iso));
    if js_date.get_time().is_nan() {
        return iso.to_string();
    }

    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("year"),
        &wasm_bindgen::JsValue::from_str("numeric"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("month"),
        &wasm_bindgen::JsValue::from_str("short"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("day"),
        &wasm_bindgen::JsValue::from_str("numeric"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("hour"),
        &wasm_bindgen::JsValue::from_str("2-digit"),
    );
    let _ = js_sys::Reflect::set(
        &opts,
        &wasm_bindgen::JsValue::from_str("minute"),
        &wasm_bindgen::JsValue::from_str("2-digit"),
    );

    js_date
        .to_locale_string("en-US", &opts)
        .as_string()
        .unwrap_or_else(|| iso.to_string())
}

/// Format a relative time string (e.g. "5m ago", "2h ago").
///
/// Returns an empty string if the input is empty or unparseable.
pub fn time_ago(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }

    let js_date = js_sys::Date::new_with_year_month_day_hr_min_sec(0, 0, 0, 0, 0, 0);
    js_date.set_time(js_sys::Date::parse(iso));
    if js_date.get_time().is_nan() {
        return String::new();
    }

    let now_ms = js_sys::Date::now();
    let date_ms = js_date.get_time();
    let seconds = ((now_ms - date_ms) / 1000.0) as i64;

    if seconds < 60 {
        return "just now".to_string();
    }
    if seconds < 3600 {
        return format!("{}m ago", seconds / 60);
    }
    if seconds < 86400 {
        return format!("{}h ago", seconds / 3600);
    }
    format!("{}d ago", seconds / 86400)
}

/// Escape HTML special characters to prevent XSS in dynamic content.
pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Check if a participation type string indicates an in-person attendee.
///
/// Uses case-insensitive substring matching, matching the backend's
/// `is_in_person()` method.
pub fn is_in_person(participation_type: &str) -> bool {
    let lower = participation_type.to_lowercase();
    lower.contains("in-person") || lower.contains("in person")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_participation_badge_in_person() {
        let badge = get_participation_badge("In-Person");
        assert_eq!(badge.label, "In-Person");
        assert_eq!(badge.css_class, "badge-info");
    }

    #[test]
    fn test_participation_badge_in_person_case_insensitive() {
        let badge = get_participation_badge("in-person");
        assert_eq!(badge.label, "In-Person");
        assert_eq!(badge.css_class, "badge-info");
    }

    #[test]
    fn test_participation_badge_in_person_long() {
        let badge = get_participation_badge("In-Person (Bangkok): Attend at the venue");
        assert_eq!(badge.label, "In-Person");
        assert_eq!(badge.css_class, "badge-info");
    }

    #[test]
    fn test_participation_badge_online() {
        let badge = get_participation_badge("Online");
        assert_eq!(badge.label, "Online");
        assert_eq!(badge.css_class, "badge-warning");
    }

    #[test]
    fn test_participation_badge_virtual() {
        let badge = get_participation_badge("Virtual");
        assert_eq!(badge.label, "Online");
        assert_eq!(badge.css_class, "badge-warning");
    }

    #[test]
    fn test_participation_badge_empty() {
        let badge = get_participation_badge("");
        assert_eq!(badge.label, "Unknown");
        assert_eq!(badge.css_class, "badge-warning");
    }

    #[test]
    fn test_participation_badge_unknown() {
        let badge = get_participation_badge("Hybrid");
        assert_eq!(badge.label, "Hybrid");
        assert_eq!(badge.css_class, "badge-warning");
    }

    #[test]
    fn test_participation_badge_colon() {
        let badge = get_participation_badge("Hybrid: Some description");
        assert_eq!(badge.label, "Hybrid");
        assert_eq!(badge.css_class, "badge-warning");
    }

    #[test]
    fn test_is_in_person() {
        assert!(is_in_person("In-Person"));
        assert!(is_in_person("in-person"));
        assert!(is_in_person("In Person"));
        assert!(is_in_person("IN-PERSON"));
        assert!(is_in_person("In-Person (Physical)"));
        assert!(!is_in_person("Online"));
        assert!(!is_in_person("Virtual"));
        assert!(!is_in_person(""));
        assert!(!is_in_person("Hybrid"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(
            escape_html("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;"
        );
        assert_eq!(escape_html("a & b"), "a &amp; b");
        assert_eq!(escape_html("\"quoted\""), "&quot;quoted&quot;");
    }
}
