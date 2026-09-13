use super::*;

fn make_event() -> EventConfig {
    EventConfig {
        id: "test-event".to_string(),
        name: "Test Event".to_string(),
        slug: "test-event".to_string(),
        tagline: String::new(),
        link: String::new(),
        status: EventStatus::Active,
        event_start_ms: 2_000_000_000_000, // ~2033
        event_end_ms: 2_000_001_000_000,
        time_tba: false,
        sheet_id: String::new(),
        sheet_name: "Attendees".to_string(),
        staff_sheet_name: "staff".to_string(),
        quiz_enabled: false,
        nft_collection_mint: String::new(),
        nft_metadata_uri: String::new(),
        nft_image_url: String::new(),
        poster_url: String::new(),
        recap_published: false,
        post_event_registration_open: false,
        post_event_registration_until_ms: None,
        nft_name_template: String::new(),
        nft_symbol: String::new(),
        nft_description_template: String::new(),
        merkle_tree: String::new(),
        organization_id: String::new(),
        organizer_emails: vec![],
        staff_emails: vec![],
        claim_base_url: String::new(),
        deposit_enabled: false,
        deposit_amount_usdc: 0,
        deposit_amount_thb: 0,
        promptpay_id: String::new(),
        escrow_address: String::new(),
        escrow_status: EscrowStatus::None,
        organizer_wallet: String::new(),
        on_chain_event_id: 0,
        refund_deadline_hours: 168, // 7 days
        max_refundable_deposits: 0,
        description: String::new(),
        location: String::new(),
        video_url: String::new(),
        visibility: EventVisibility::Public,
        event_format: EventFormat::InPerson,
        require_contact_info: true,
        require_photo_consent: false,
        in_person_capacity: None,
        online_capacity: None,
        online_open_mode: OnlineOpenMode::Always,
        online_registration_open: false,
        deposit_deadline_hours: None,
        created_at: String::new(),
        updated_at: String::new(),
        updated_by: String::new(),
        dev_profile_enabled: false,
        community_links: vec![],
        calendar_subscribe_url: String::new(),
    }
}

// ── is_registration_open ────────────────────────────────────────

#[test]
fn test_registration_open_active_before_start() {
    let event = make_event();
    assert!(event.is_registration_open(1_000_000_000_000));
}

#[test]
fn test_registration_closed_after_start() {
    let event = make_event();
    assert!(!event.is_registration_open(2_500_000_000_000));
}

#[test]
fn test_registration_open_when_start_is_zero() {
    let mut event = make_event();
    event.event_start_ms = 0; // TBA
    assert!(event.is_registration_open(2_500_000_000_000));
}

#[test]
fn test_registration_closed_when_draft() {
    let mut event = make_event();
    event.status = EventStatus::Draft;
    assert!(!event.is_registration_open(1_000_000_000_000));
}

#[test]
fn test_registration_closed_when_completed() {
    let mut event = make_event();
    event.status = EventStatus::Completed;
    assert!(!event.is_registration_open(1_000_000_000_000));
}

// ── is_refund_eligible ──────────────────────────────────────────

#[test]
fn test_refund_eligible_before_deadline() {
    let event = make_event();
    // event_end + 168h * 3600_000 = 2_000_001_000_000 + 604_800_000 = 2_000_605_800_000
    assert!(event.is_refund_eligible(2_000_605_800_000)); // exactly at deadline
}

#[test]
fn test_refund_not_eligible_after_deadline() {
    let event = make_event();
    assert!(!event.is_refund_eligible(2_000_605_800_001));
}

#[test]
fn test_refund_eligible_well_before_deadline() {
    let event = make_event();
    assert!(event.is_refund_eligible(2_000_002_000_000));
}

// ── has_in_person_capacity ──────────────────────────────────────

#[test]
fn test_in_person_capacity_unlimited() {
    let event = make_event(); // in_person_capacity = None
    assert!(event.has_in_person_capacity(999));
}

#[test]
fn test_in_person_capacity_has_room() {
    let mut event = make_event();
    event.in_person_capacity = Some(100);
    assert!(event.has_in_person_capacity(50));
}

#[test]
fn test_in_person_capacity_at_limit() {
    let mut event = make_event();
    event.in_person_capacity = Some(100);
    assert!(!event.has_in_person_capacity(100));
}

#[test]
fn test_in_person_capacity_over_limit() {
    let mut event = make_event();
    event.in_person_capacity = Some(100);
    assert!(!event.has_in_person_capacity(101));
}

// ── has_online_capacity ─────────────────────────────────────────

#[test]
fn test_online_capacity_unlimited() {
    let event = make_event(); // online_capacity = None
    assert!(event.has_online_capacity(999));
}

#[test]
fn test_online_capacity_at_limit() {
    let mut event = make_event();
    event.online_capacity = Some(50);
    assert!(!event.has_online_capacity(50));
}

#[test]
fn test_online_capacity_has_room() {
    let mut event = make_event();
    event.online_capacity = Some(50);
    assert!(event.has_online_capacity(49));
}

// ── accepts_usdc_deposits ───────────────────────────────────────

#[test]
fn test_accepts_usdc_when_enabled_and_initialized() {
    let mut event = make_event();
    event.deposit_enabled = true;
    event.escrow_status = EscrowStatus::Initialized;
    assert!(event.accepts_usdc_deposits());
}

#[test]
fn test_rejects_usdc_when_deposit_disabled() {
    let mut event = make_event();
    event.deposit_enabled = false;
    event.escrow_status = EscrowStatus::Initialized;
    assert!(!event.accepts_usdc_deposits());
}

#[test]
fn test_rejects_usdc_when_escrow_not_initialized() {
    let mut event = make_event();
    event.deposit_enabled = true;
    event.escrow_status = EscrowStatus::None;
    assert!(!event.accepts_usdc_deposits());
}

#[test]
fn test_rejects_usdc_when_escrow_deactivated() {
    let mut event = make_event();
    event.deposit_enabled = true;
    event.escrow_status = EscrowStatus::Deactivated;
    assert!(!event.accepts_usdc_deposits());
}

// ── deposit_deadline_passed ─────────────────────────────────────

#[test]
fn test_deadline_not_passed_when_no_deadline_configured() {
    let event = make_event(); // deposit_deadline_hours = None
    assert!(!event.deposit_deadline_passed(1_000_000_000_000, 2_000_000_000_000));
}

#[test]
fn test_deadline_passed_when_overdue() {
    let mut event = make_event();
    event.deposit_deadline_hours = Some(24); // 24h after registration
    let reg_ms = 1_000_000_000_000_i64;
    let deadline_ms = reg_ms + (24_i64 * 3_600_000); // +24h
    assert!(!event.deposit_deadline_passed(reg_ms, deadline_ms)); // exactly at deadline
    assert!(event.deposit_deadline_passed(reg_ms, deadline_ms + 1)); // just past
}

#[test]
fn test_deadline_not_passed_when_within_window() {
    let mut event = make_event();
    event.deposit_deadline_hours = Some(48);
    let reg_ms = 1_000_000_000_000_i64;
    let within_ms = reg_ms + (12_i64 * 3_600_000); // +12h
    assert!(!event.deposit_deadline_passed(reg_ms, within_ms));
}

// ── post-event registration fields (Plan 008 — Phase 3) ──────────

#[test]
fn test_post_event_registration_defaults_closed() {
    // New events default to closed lead capture, no deadline.
    let event = make_event();
    assert!(!event.post_event_registration_open);
    assert_eq!(event.post_event_registration_until_ms, None);
}

#[test]
fn test_post_event_registration_propagates_to_meta() {
    // The flag + deadline must survive EventConfig → EventMeta so the
    // public recap page (which reads EventMeta via the API) can render the
    // CTA when the organizer opens lead capture.
    let mut event = make_event();
    event.post_event_registration_open = true;
    event.post_event_registration_until_ms = Some(1_700_000_000_000);
    let meta = event.to_meta();
    assert!(meta.post_event_registration_open);
    assert_eq!(
        meta.post_event_registration_until_ms,
        Some(1_700_000_000_000)
    );
}

#[test]
fn test_post_event_registration_serde_round_trip() {
    // KV stores EventConfig as JSON; the fields must round-trip losslessly
    // (open flag + deadline), including the "no deadline" None case.
    let mut event = make_event();
    event.post_event_registration_open = true;
    event.post_event_registration_until_ms = Some(1_700_000_000_000);

    let json = serde_json::to_string(&event).expect("serialize");
    let back: EventConfig = serde_json::from_str(&json).expect("deserialize");
    assert!(back.post_event_registration_open);
    assert_eq!(
        back.post_event_registration_until_ms,
        Some(1_700_000_000_000)
    );

    // None deadline survives + is skipped in serialization (no null key).
    event.post_event_registration_until_ms = None;
    let json_none = serde_json::to_string(&event).expect("serialize none");
    assert!(!json_none.contains("post_event_registration_until_ms"));
    let back_none: EventConfig = serde_json::from_str(&json_none).expect("deserialize none");
    assert_eq!(back_none.post_event_registration_until_ms, None);
}

#[test]
fn test_post_event_registration_absent_in_json_defaults_closed() {
    // Older KV entries (pre-Phase-3) lack these fields entirely — serde's
    // #[serde(default)] must deserialize them as closed / no-deadline so a
    // reseed or read doesn't panic or leak a false "open" state.
    let legacy = make_event();
    let mut value = serde_json::to_value(&legacy).expect("serialize");
    let obj = value.as_object_mut().expect("object");
    obj.remove("post_event_registration_open");
    obj.remove("post_event_registration_until_ms");
    let back: EventConfig = serde_json::from_value(value).expect("deserialize stripped");
    assert!(!back.post_event_registration_open);
    assert_eq!(back.post_event_registration_until_ms, None);
}

// ── NFT name / description template resolution ──────────────────────
// Guards the badge title/description drift: a template WITHOUT the
// {event_name} placeholder is a fixed literal and does NOT follow the event
// name — the exact behavior that put a "#3" title on a "#4" event.

#[test]
fn nft_name_empty_template_uses_bethere_prefix() {
    let mut e = make_event();
    e.name = "Rustacean Day".to_string();
    e.nft_name_template = String::new();
    assert_eq!(e.nft_name(), "BeThere - Rustacean Day");
}

#[test]
fn nft_name_placeholder_template_follows_event_name() {
    let mut e = make_event();
    e.name = "Rust Day".to_string();
    e.nft_name_template = "{event_name} Badge".to_string();
    assert_eq!(e.nft_name(), "Rust Day Badge");
}

#[test]
fn nft_name_fixed_literal_ignores_event_name() {
    // Fixed literal (no placeholder) — stays put regardless of the name.
    let mut e = make_event();
    e.name = "Solana x AI Builders #4 (Bangkok)".to_string();
    e.nft_name_template = "Solana x AI Builders #3".to_string();
    assert_eq!(e.nft_name(), "Solana x AI Builders #3");
}

#[test]
fn nft_name_truncates_to_32_chars() {
    let mut e = make_event();
    e.name = "A Very Long Event Name That Exceeds The Metaplex Limit".to_string();
    e.nft_name_template = String::new();
    let n = e.nft_name();
    assert!(n.len() <= 32, "name '{n}' is {} chars", n.len());
    assert!(n.starts_with("BeThere - "));
    assert!(n.ends_with("..."));
}

#[test]
fn nft_description_empty_template_default() {
    let mut e = make_event();
    e.name = "Rust Day".to_string();
    e.nft_description_template = String::new();
    assert_eq!(e.nft_description(), "Proof of attendance at Rust Day");
}

#[test]
fn nft_description_placeholder_substituted() {
    let mut e = make_event();
    e.name = "Rust Day".to_string();
    e.nft_description_template = "You attended {event_name}!".to_string();
    assert_eq!(e.nft_description(), "You attended Rust Day!");
}

// ── Post-event registration deadline (Plan 008 — Phase 3) ──────────────

#[test]
fn post_event_registration_no_deadline_never_lapses() {
    let mut e = make_event();
    e.post_event_registration_open = true;
    e.post_event_registration_until_ms = None;
    assert!(!e.post_event_registration_deadline_passed(i64::MAX));
    assert!(e.post_event_registration_accepting(i64::MAX));
}

#[test]
fn post_event_registration_deadline_is_exclusive_at_the_boundary() {
    let mut e = make_event();
    e.post_event_registration_open = true;
    e.post_event_registration_until_ms = Some(1_000);
    // Matches `register::post_event`'s `now_ms >= until` → the deadline
    // instant itself is already closed.
    assert!(!e.post_event_registration_deadline_passed(999));
    assert!(e.post_event_registration_deadline_passed(1_000));
    assert!(e.post_event_registration_deadline_passed(1_001));
}

#[test]
fn post_event_registration_accepting_is_false_once_the_deadline_lapses() {
    let mut e = make_event();
    e.post_event_registration_open = true;
    e.post_event_registration_until_ms = Some(1_000);
    assert!(e.post_event_registration_accepting(999));
    assert!(!e.post_event_registration_accepting(1_000));
}

#[test]
fn post_event_registration_closed_toggle_beats_a_future_deadline() {
    let mut e = make_event();
    e.post_event_registration_open = false;
    e.post_event_registration_until_ms = Some(i64::MAX);
    assert!(!e.post_event_registration_accepting(0));
    // The deadline itself has still not passed — the two are independent.
    assert!(!e.post_event_registration_deadline_passed(0));
}
