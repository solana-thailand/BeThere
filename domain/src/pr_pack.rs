//! PR Pack generator — Plan 008 Phase 4.
//!
//! Pure, deterministic functions that turn an `EventConfig` into copy-pasteable
//! marketing copy (headline, blurb, social post, calendar text, email snippet,
//! deposit terms, organizer list). No I/O, no external API calls, no randomness.
//!
//! Why pure functions: the spec (Plan 008 §3.4) requires "Generation is
//! deterministic — no external API calls." Co-locating the templating logic in
//! `domain` (not `worker`) keeps it unit-testable without a D1/KV harness and
//! lets the frontend reuse the same shaping if it ever needs to.
//!
//! Date formatting uses UTC for v1 (Plan 008 §3.4.1: "use UTC for v1, defer TZ
//! to frontend"). Time-zone-aware rendering is a frontend concern because the
//! viewer's TZ is not knowable server-side.

use chrono::{DateTime, TimeZone, Utc};

use crate::models::event::EventConfig;

/// Generated PR pack — one structured field per marketing surface.
///
/// Every field is a plain string so the frontend can drop it straight into a
/// copy-to-clipboard card. The `organizers` field is the one exception: it's a
/// list because rendering it as bullets vs. comma-joined is a UI choice.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PrPack {
    /// `{name} — {tagline}` (or just `{name}` when the tagline is empty).
    pub headline: String,
    /// 2-sentence elevator pitch using name, tagline, date, location.
    pub short_blurb: String,
    /// Twitter/X-shaped post, ≤280 chars when possible.
    pub social_post: String,
    /// "Add to calendar" one-liner + subscribe URL when set.
    pub calendar_text: String,
    /// 3-paragraph email template (intro / what+when / how to register).
    pub email_snippet: String,
    /// Human-readable summary of the deposit policy.
    pub deposit_terms: String,
    /// Parsed from `organizer_emails` (CSV → list). Empty list when unset.
    pub organizers: Vec<String>,
}

/// Format an epoch-millis timestamp as a readable UTC date+time string.
///
/// Returns `"TBA"` when the event has no fixed start time (`time_tba == true`
/// or `event_start_ms <= 0`), so callers don't have to special-case it.
fn format_event_date(event_start_ms: i64, time_tba: bool) -> String {
    if time_tba || event_start_ms <= 0 {
        return "TBA".to_string();
    }
    match Utc.timestamp_millis_opt(event_start_ms) {
        chrono::LocalResult::Single(dt) => dt.format("%a %b %e, %Y %H:%M UTC").to_string(),
        _ => "TBA".to_string(),
    }
}

/// Compute a human-readable duration from start/end epoch-millis.
///
/// Returns `""` when either bound is missing/invalid or the duration is
/// non-positive. Rounds down to the largest whole unit (days → hours → minutes)
/// so "2-hour event" reads cleaner than "120-minute event".
fn format_duration(event_start_ms: i64, event_end_ms: i64) -> String {
    if event_start_ms <= 0 || event_end_ms <= 0 || event_end_ms <= event_start_ms {
        return String::new();
    }
    let dur = match DateTime::<Utc>::from_timestamp_millis(event_end_ms)
        .zip(DateTime::<Utc>::from_timestamp_millis(event_start_ms))
    {
        Some((end, start)) => end.signed_duration_since(start),
        None => return String::new(),
    };
    let days = dur.num_days();
    if days >= 1 {
        return format!("{days}-day event");
    }
    let hours = dur.num_hours();
    if hours >= 1 {
        return format!("{hours}-hour event");
    }
    let mins = dur.num_minutes();
    if mins >= 1 {
        return format!("{mins}-minute event");
    }
    String::new()
}

/// Format a USDC amount stored as micro-USDC (6 decimals) as a dollar string.
///
/// `15_000_000` → `"$15"`, `15_500_000` → `"$15.50"`. Trailing zeros are
/// dropped so common round amounts render cleanly.
fn format_usdc(micro: u64) -> String {
    let dollars = micro / 1_000_000;
    let cents = (micro % 1_000_000) / 10_000; // 2-digit cents
    if cents == 0 {
        format!("${dollars}")
    } else {
        format!("${dollars}.{cents:02}")
    }
}

/// The public registration URL for an event.
///
/// Prefers `claim_base_url` (the organizer-configured base). Falls back to a
/// `/e/{slug}` path relative to the site root so the social post always has a
/// clickable link even when `claim_base_url` is empty.
fn registration_url(event: &EventConfig) -> String {
    if !event.claim_base_url.is_empty() {
        event.claim_base_url.clone()
    } else {
        format!("/e/{}", event.slug)
    }
}

/// Generate the full PR pack from an event config.
///
/// Deterministic: two calls with the same `EventConfig` return identical output.
/// No I/O, no allocation beyond the returned strings. Safe to call on every
/// request — the handler does no caching because generation is cheap (a handful
/// of `format!` calls).
pub fn generate(event: &EventConfig) -> PrPack {
    let date = format_event_date(event.event_start_ms, event.time_tba);
    let duration = format_duration(event.event_start_ms, event.event_end_ms);
    let reg_url = registration_url(event);
    let location = if event.location.is_empty() {
        "TBA"
    } else {
        event.location.as_str()
    };

    // ── headline ─────────────────────────────────────────────────────────
    let headline = if event.tagline.is_empty() {
        event.name.clone()
    } else {
        format!("{} — {}", event.name, event.tagline)
    };

    // ── short_blurb ──────────────────────────────────────────────────────
    let short_blurb = if event.tagline.is_empty() {
        format!(
            "Join us for {} on {} at {}. {}",
            event.name, date, location, reg_url
        )
    } else {
        format!(
            "{}: {}. Join us on {} at {}. Register: {}",
            event.name, event.tagline, date, location, reg_url
        )
    };

    // ── social_post (Twitter/X-shaped, ≤280 chars when possible) ─────────
    let mut social_post = format!(
        "🗓️ {} — {} @ {}\n{}\n{}",
        event.name, date, location, event.tagline, reg_url
    );
    if social_post.chars().count() > 280 {
        // Trim the tagline to fit. Recompute without the tagline line so the
        // essentials (name, date, location, link) always survive.
        social_post = format!("🗓️ {} — {} @ {}\n{}", event.name, date, location, reg_url);
    }
    if social_post.chars().count() > 280 {
        // Last resort: hard truncate at the char boundary. The link is at the
        // end so this preserves name+date; only acceptable if the location is
        // itself very long.
        social_post = social_post.chars().take(280).collect();
    }

    // ── calendar_text ────────────────────────────────────────────────────
    let mut calendar_text = if duration.is_empty() {
        format!(
            "Add to calendar: {} on {} at {}.",
            event.name, date, location
        )
    } else {
        format!(
            "Add to calendar: {} on {} at {}. {}.",
            event.name, date, location, duration
        )
    };
    if !event.calendar_subscribe_url.is_empty() {
        calendar_text.push_str(&format!(" Subscribe: {}", event.calendar_subscribe_url));
    }

    // ── email_snippet (3 paragraphs) ─────────────────────────────────────
    let p1 = format!("You're invited to {}!", event.name);
    let p2 = if event.tagline.is_empty() {
        format!("The event takes place on {} at {}.", date, location)
    } else {
        format!("{} — {} at {}.", event.tagline, date, location)
    };
    let p3 = format!("Register here: {}", reg_url);
    let email_snippet = format!("{p1}\n\n{p2}\n\n{p3}");

    // ── deposit_terms ────────────────────────────────────────────────────
    let deposit_terms = if !event.deposit_enabled {
        "No deposit required — free registration.".to_string()
    } else {
        let mut terms = format!(
            "A deposit is required to secure your spot: {}",
            format_usdc(event.deposit_amount_usdc)
        );
        if event.deposit_amount_thb > 0 {
            terms.push_str(&format!(
                " (or {} THB via PromptPay)",
                event.deposit_amount_thb
            ));
        }
        terms.push_str(&format!(
            ". Fully refunded within {} hours after the event",
            event.refund_deadline_hours
        ));
        if event.max_refundable_deposits > 0 {
            terms.push_str(&format!(
                " (up to {} refunds)",
                event.max_refundable_deposits
            ));
        }
        terms.push('.');
        terms
    };

    // ── organizers ───────────────────────────────────────────────────────
    // Already a Vec<String> on EventConfig; trim+dedupe defensively so a
    // stray whitespace entry doesn't show up as an empty bullet.
    let organizers = dedupe_emails(&event.organizer_emails);

    PrPack {
        headline,
        short_blurb,
        social_post,
        calendar_text,
        email_snippet,
        deposit_terms,
        organizers,
    }
}

/// Trim + lowercase + dedupe a list of emails. Preserves first-seen order.
fn dedupe_emails(emails: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(emails.len());
    for e in emails {
        let trimmed = e.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if seen.insert(lower.clone()) {
            out.push(lower);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::event::{EventConfig, EventFormat, EventStatus, EventVisibility};

    /// Minimal valid EventConfig for tests — only the fields `generate` reads.
    fn sample_event() -> EventConfig {
        EventConfig {
            id: "evt-1".into(),
            name: "Solana Bangkok 2025".into(),
            slug: "solana-bangkok-2025".into(),
            tagline: "The Road to Mainnet".into(),
            link: String::new(),
            status: EventStatus::Active,
            event_start_ms: 1_700_000_000_000, // 2023-11-14 22:13:20 UTC
            event_end_ms: 1_700_003_600_000,   // +1 hour
            time_tba: false,
            sheet_id: String::new(),
            sheet_name: String::new(),
            staff_sheet_name: String::new(),
            quiz_enabled: false,
            nft_collection_mint: String::new(),
            nft_metadata_uri: String::new(),
            nft_image_url: String::new(),
            poster_url: String::new(),
            recap_published: false,
            nft_name_template: String::new(),
            nft_symbol: String::new(),
            nft_description_template: String::new(),
            merkle_tree: String::new(),
            organization_id: String::new(),
            organizer_emails: vec!["AlIce@Org.com".into(), "bob@org.com".into()],
            staff_emails: vec![],
            claim_base_url: "https://bethere.example/claim".into(),
            deposit_enabled: true,
            deposit_amount_usdc: 15_000_000, // $15
            deposit_amount_thb: 500,
            promptpay_id: String::new(),
            escrow_address: String::new(),
            escrow_status: crate::models::event::EscrowStatus::None,
            organizer_wallet: String::new(),
            on_chain_event_id: 0,
            refund_deadline_hours: 168,
            max_refundable_deposits: 50,
            description: String::new(),
            location: "Bangkok, Thailand".into(),
            video_url: String::new(),
            calendar_subscribe_url: "https://cal.example/sub".into(),
            community_links: vec![],
            visibility: EventVisibility::Public,
            event_format: EventFormat::InPerson,
            require_contact_info: true,
            require_photo_consent: false,
            in_person_capacity: None,
            online_capacity: None,
            online_open_mode: crate::models::event::OnlineOpenMode::Always,
            online_registration_open: false,
            deposit_deadline_hours: None,
            created_at: "2023-01-01T00:00:00Z".into(),
            updated_at: "2023-01-01T00:00:00Z".into(),
            updated_by: String::new(),
            dev_profile_enabled: false,
        }
    }

    #[test]
    fn headline_combines_name_and_tagline() {
        let pack = generate(&sample_event());
        assert_eq!(pack.headline, "Solana Bangkok 2025 — The Road to Mainnet");
    }

    #[test]
    fn headline_falls_back_to_name_when_tagline_empty() {
        let mut e = sample_event();
        e.tagline.clear();
        let pack = generate(&e);
        assert_eq!(pack.headline, "Solana Bangkok 2025");
    }

    #[test]
    fn determinism_two_calls_identical() {
        let e = sample_event();
        let a = generate(&e);
        let b = generate(&e);
        assert_eq!(a, b, "generate() must be deterministic");
    }

    #[test]
    fn social_post_respects_280_char_budget() {
        let pack = generate(&sample_event());
        assert!(
            pack.social_post.chars().count() <= 280,
            "social_post must fit 280 chars, got {}: {}",
            pack.social_post.chars().count(),
            pack.social_post
        );
    }

    #[test]
    fn social_post_truncates_when_everything_is_long() {
        let mut e = sample_event();
        e.name = "A".repeat(300);
        e.tagline = String::new();
        e.location = "L".repeat(300);
        e.claim_base_url = String::new();
        let pack = generate(&e);
        assert!(
            pack.social_post.chars().count() <= 280,
            "even pathological input must be clamped to 280"
        );
    }

    #[test]
    fn deposit_terms_disabled_when_no_deposit() {
        let mut e = sample_event();
        e.deposit_enabled = false;
        let pack = generate(&e);
        assert_eq!(
            pack.deposit_terms,
            "No deposit required — free registration."
        );
    }

    #[test]
    fn deposit_terms_formats_usdc_and_thb_and_deadline_and_cap() {
        let pack = generate(&sample_event());
        assert!(
            pack.deposit_terms.contains("$15"),
            "expected $15 in: {}",
            pack.deposit_terms
        );
        assert!(
            pack.deposit_terms.contains("500 THB"),
            "expected THB in: {}",
            pack.deposit_terms
        );
        assert!(
            pack.deposit_terms.contains("168 hours"),
            "expected deadline in: {}",
            pack.deposit_terms
        );
        assert!(
            pack.deposit_terms.contains("up to 50 refunds"),
            "expected cap in: {}",
            pack.deposit_terms
        );
    }

    #[test]
    fn organizers_are_deduped_and_lowercased() {
        let pack = generate(&sample_event());
        assert_eq!(pack.organizers, vec!["alice@org.com", "bob@org.com"]);
    }

    #[test]
    fn organizers_empty_when_none_set() {
        let mut e = sample_event();
        e.organizer_emails = vec!["  ".into(), "".into()];
        let pack = generate(&e);
        assert!(pack.organizers.is_empty());
    }

    #[test]
    fn calendar_text_includes_subscribe_url_when_set() {
        let pack = generate(&sample_event());
        assert!(
            pack.calendar_text.contains("https://cal.example/sub"),
            "expected subscribe URL in: {}",
            pack.calendar_text
        );
    }

    #[test]
    fn registration_url_falls_back_to_slug_path() {
        let mut e = sample_event();
        e.claim_base_url.clear();
        let pack = generate(&e);
        assert!(
            pack.short_blurb.contains("/e/solana-bangkok-2025"),
            "expected slug fallback in blurb: {}",
            pack.short_blurb
        );
    }

    #[test]
    fn time_tba_renders_tba_everywhere() {
        let mut e = sample_event();
        e.time_tba = true;
        let pack = generate(&e);
        assert!(
            pack.short_blurb.contains("TBA"),
            "blurb: {}",
            pack.short_blurb
        );
        assert!(
            pack.calendar_text.contains("TBA"),
            "cal: {}",
            pack.calendar_text
        );
    }

    #[test]
    fn format_usdc_drops_trailing_zeros() {
        assert_eq!(format_usdc(15_000_000), "$15");
        assert_eq!(format_usdc(15_500_000), "$15.50");
        assert_eq!(format_usdc(0), "$0");
        assert_eq!(format_usdc(99), "$0");
    }

    #[test]
    fn duration_formats_largest_unit() {
        assert_eq!(format_duration(0, 3_600_000), "");
        assert_eq!(format_duration(1_000, 1_000), "");
        assert_eq!(format_duration(1_000, 3_601_000), "1-hour event");
        assert_eq!(format_duration(0, 86_400_000), "");
        assert_eq!(format_duration(1, 86_400_001), "1-day event");
    }
}
