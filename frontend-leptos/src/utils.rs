//! Shared utility functions extracted from scanner and admin pages.

use std::cell::RefCell;

// ---------------------------------------------------------------------------
// SEC-005: Cluster-aware Solana explorer URLs
// ---------------------------------------------------------------------------

thread_local! {
    // Cached cluster string fetched from `/api/health`.
    // None = not yet fetched; Some("devnet") or Some("mainnet-beta").
    static CACHED_CLUSTER: RefCell<Option<String>> = RefCell::new(None);
}

/// Fetch the Solana cluster from `/api/health` and cache it.
/// Returns "devnet" as fallback if the fetch fails.
pub async fn fetch_cluster() -> String {
    let window = web_sys::window().expect("no window");
    let origin = window.location().origin().unwrap_or_else(|_| "http://localhost:8787".to_string());
    let url = format!("{origin}/api/health");

    let cluster = async {
        let resp = gloo::net::http::Request::get(&url).send().await.ok()?;
        let body = resp.text().await.ok()?;
        let val: serde_json::Value = serde_json::from_str(&body).ok()?;
        val.get("cluster")?.as_str().map(String::from)
    }
    .await
    .unwrap_or_else(|| "devnet".to_string());

    CACHED_CLUSTER.with(|c| *c.borrow_mut() = Some(cluster.clone()));

    cluster
}

/// Get the cached cluster, or "devnet" as fallback.
/// Call `fetch_cluster()` first (in `spawn_local`) before using this.
pub fn get_cluster() -> String {
    CACHED_CLUSTER.with(|c| {
        c.borrow()
            .clone()
            .unwrap_or_else(|| "devnet".to_string())
    })
}

/// Build a cluster-aware Solscan transaction URL.
pub fn solscan_tx_url(signature: &str, cluster: &str) -> String {
    let cluster_param = if cluster == "mainnet-beta" {
        String::new()
    } else {
        format!("?cluster={cluster}")
    };
    format!("https://solscan.io/tx/{signature}{cluster_param}")
}

/// Build a cluster-aware Solscan account/address URL.
pub fn solscan_address_url(address: &str, cluster: &str) -> String {
    let cluster_param = if cluster == "mainnet-beta" {
        String::new()
    } else {
        format!("?cluster={cluster}")
    };
    format!("https://solscan.io/address/{address}{cluster_param}")
}

/// Build a Metaplex Core Explorer URL for verifying compressed NFT assets.
/// Pattern: https://core.metaplex.com/explorer/{asset_id}?env={cluster}
pub fn metaplex_explorer_url(asset_id: &str, cluster: &str) -> String {
    let env = if cluster == "mainnet-beta" {
        "mainnet-beta"
    } else {
        "devnet"
    };
    format!("https://core.metaplex.com/explorer/{asset_id}?env={env}")
}

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

/// Build a JS object from key-value string pairs.
///
/// Helper to avoid repeated `Reflect::set` calls when constructing
/// JS options objects for `toLocaleString` etc.
fn js_object(pairs: &[(&str, &str)]) -> js_sys::Object {
    let obj = js_sys::Object::new();
    for (key, val) in pairs {
        let _ = js_sys::Reflect::set(
            &obj,
            &wasm_bindgen::JsValue::from_str(key),
            &wasm_bindgen::JsValue::from_str(val),
        );
    }
    obj
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

    let opts = js_object(&[
        ("year", "numeric"),
        ("month", "short"),
        ("day", "numeric"),
        ("hour", "2-digit"),
        ("minute", "2-digit"),
    ]);

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
///
/// Defaults to `true` when empty — legacy events predate this field
/// and were all in-person.
pub fn is_in_person(participation_type: &str) -> bool {
    let lower = participation_type.to_lowercase();
    if lower.is_empty() {
        return true;
    }
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
        assert!(is_in_person(""));
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
