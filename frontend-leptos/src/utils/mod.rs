//! Shared utility functions extracted from scanner and admin pages.

pub mod money;
pub mod promptpay;
pub mod qr_gen;

use std::cell::RefCell;

// ---------------------------------------------------------------------------
// SEC-005: Cluster-aware Solana explorer URLs
// ---------------------------------------------------------------------------

thread_local! {
    // Cached networks fetched from `/api/health`. None = not yet fetched.
    // Escrow and badge (NFT) clusters differ in prod: badges went mainnet while
    // escrow stayed on devnet (commit 8265d13), so each link picks its own.
    static CACHED_NETWORKS: RefCell<Option<SolanaNetworks>> = const { RefCell::new(None) };
}

/// Clusters reported by `/api/health`: `cluster` (escrow, where user
/// transactions are signed) and `solana.nft_cluster` (where badges are minted).
#[derive(Clone, Debug, PartialEq)]
pub struct SolanaNetworks {
    pub escrow: String,
    pub nft: String,
}

const FALLBACK_CLUSTER: &str = "devnet";

/// Parse the `/api/health` body. Missing fields fall back to devnet; a missing
/// `nft_cluster` (older worker) falls back to the escrow cluster.
pub fn parse_health_networks(body: &str) -> SolanaNetworks {
    let val: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let escrow = val
        .get("cluster")
        .and_then(|c| c.as_str())
        .unwrap_or(FALLBACK_CLUSTER)
        .to_string();
    let nft = val
        .pointer("/solana/nft_cluster")
        .and_then(|c| c.as_str())
        .filter(|c| *c != "unknown")
        .map_or_else(|| escrow.clone(), String::from);
    SolanaNetworks { escrow, nft }
}

/// Fetch the Solana networks from `/api/health` and cache them. Called once at
/// app boot (`App`), so every page's `get_cluster()` sees the real network.
/// Returns the escrow cluster ("devnet" if the fetch fails).
pub async fn fetch_cluster() -> String {
    let window = web_sys::window().expect("no window");
    let origin = window
        .location()
        .origin()
        .unwrap_or_else(|_| "http://localhost:8787".to_string());
    let url = format!("{origin}/api/health");

    let body = async {
        let resp = crate::api::fetch::get(&url, &[]).await.ok()?;
        crate::api::fetch::response_text(&resp).await.ok()
    }
    .await
    .unwrap_or_default();
    let networks = parse_health_networks(&body);
    let escrow = networks.escrow.clone();

    CACHED_NETWORKS.with(|c| *c.borrow_mut() = Some(networks));

    escrow
}

/// Escrow cluster (wallet signing, escrow tx links), or "devnet" before the
/// boot fetch resolves.
pub fn get_cluster() -> String {
    CACHED_NETWORKS.with(|c| {
        c.borrow()
            .as_ref()
            .map_or_else(|| FALLBACK_CLUSTER.to_string(), |n| n.escrow.clone())
    })
}

/// Badge (NFT) cluster, for links to where attendee badges live.
pub fn get_nft_cluster() -> String {
    CACHED_NETWORKS.with(|c| {
        c.borrow()
            .as_ref()
            .map_or_else(|| FALLBACK_CLUSTER.to_string(), |n| n.nft.clone())
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

/// Build an Orb Markets URL for viewing a compressed NFT asset.
/// Orb Markets displays cNFT images and metadata reliably across clusters.
pub fn orb_nft_url(asset_id: &str, cluster: &str) -> String {
    let cluster_param = if cluster == "mainnet-beta" {
        "?cluster=mainnet".to_string()
    } else {
        "?cluster=devnet".to_string()
    };
    format!("https://orbmarkets.io/token/{asset_id}/metadata{cluster_param}")
}

/// Build a Google Sheets editor URL from a spreadsheet ID.
/// Pattern: https://docs.google.com/spreadsheets/d/{SHEET_ID}/edit
/// Returns empty string for an empty/whitespace input (caller decides what to render).
pub fn google_sheet_url(sheet_id: &str) -> String {
    let trimmed = sheet_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    format!("https://docs.google.com/spreadsheets/d/{trimmed}/edit")
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

    if lower.contains("in-person") || lower.contains("in person") || lower.contains("in_person") {
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

    if lower == "retrospective" {
        return ParticipationBadge {
            label: "Retrospective".to_string(),
            css_class: "badge-neutral",
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

/// Title-case each word in a name string.
///
/// Handles "john doe" → "John Doe", "ozone" → "Ozone".
/// Preserves already-capitalized words.
pub fn capitalize_name(name: &str) -> String {
    name.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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
    lower.contains("in-person") || lower.contains("in person") || lower.contains("in_person")
}

/// Retrospective enrollment is a post-event learning lead, never a live
/// online registration. Keep this separate from `is_in_person` so callers do
/// not accidentally classify it as online by negation.
pub fn is_retrospective(participation_type: &str) -> bool {
    participation_type
        .trim()
        .eq_ignore_ascii_case("retrospective")
}

/// Format an epoch-millisecond instant as a short local date — `13 Sep 2026`.
///
/// The landing card formats its own date inline with `js_sys::Date` and also
/// handles `time_tba`; this is the date-only half, for surfaces that are
/// recalling an event that has already happened, where the start time is not
/// the useful part and "Time TBA" can no longer be true.
/// Format an epoch-millisecond instant as a short local date and 24-hour time —
/// `27 Sep 2026, 13:00`.
///
/// The landing card used to call `to_locale_string` with **no options**, which
/// renders the browser's default: `9/27/2026, 1:00:00 PM`. Seconds are noise on
/// an event date, and `9/27` is ambiguous to the Thai-majority audience this is
/// written for — `en-GB` puts the day first and names the month.
pub fn format_event_datetime(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let d = js_sys::Date::new_with_year_month_day(0, 0, 0);
    d.set_time(ms as f64);
    let opts = js_sys::Object::new();
    for (key, value) in [
        ("year", "numeric"),
        ("month", "short"),
        ("day", "numeric"),
        ("hour", "2-digit"),
        ("minute", "2-digit"),
    ] {
        let _ = js_sys::Reflect::set(&opts, &key.into(), &value.into());
    }
    let _ = js_sys::Reflect::set(&opts, &"hour12".into(), &false.into());
    d.to_locale_string("en-GB", &opts)
        .as_string()
        .unwrap_or_default()
}

/// Split an instant into the day number and a short uppercase month —
/// `("27", "SEPT")` — for the date chip on `/discover`.
///
/// Here rather than in the component for the reason `.issues/104` records: the
/// same `to_locale_string` call written per surface diverges silently, and had
/// already done so three times before this one was added.
pub fn format_event_day_parts(ms: i64) -> (String, String) {
    if ms <= 0 {
        return (String::new(), String::new());
    }
    let d = js_sys::Date::new_with_year_month_day(0, 0, 0);
    d.set_time(ms as f64);
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&opts, &"month".into(), &"short".into());
    let month = d
        .to_locale_string("en-GB", &opts)
        .as_string()
        .unwrap_or_default();
    (d.get_date().to_string(), month.to_uppercase())
}

pub fn format_event_day(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let d = js_sys::Date::new_with_year_month_day(0, 0, 0);
    d.set_time(ms as f64);
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&opts, &"year".into(), &"numeric".into());
    let _ = js_sys::Reflect::set(&opts, &"month".into(), &"short".into());
    let _ = js_sys::Reflect::set(&opts, &"day".into(), &"numeric".into());
    d.to_locale_string("en-GB", &opts)
        .as_string()
        .unwrap_or_default()
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
    fn retrospective_is_not_a_live_online_registration() {
        let badge = get_participation_badge("retrospective");
        assert_eq!(badge.label, "Retrospective");
        assert!(!is_in_person("retrospective"));
        assert!(is_retrospective("retrospective"));
        assert!(!is_retrospective("online"));
    }

    #[test]
    fn test_participation_badge_in_person_snake() {
        let badge = get_participation_badge("in_person");
        assert_eq!(badge.label, "In-Person");
        assert_eq!(badge.css_class, "badge-info");
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
        assert!(is_in_person("in_person"));
        assert!(is_in_person("In_Person"));
        assert!(is_in_person("IN_PERSON"));
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
