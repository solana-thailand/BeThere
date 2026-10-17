//! One-click post-event registration (.plans/047): the name comes from the
//! central profile, and a successful registration opens the ticket, which
//! carries the recording and the slides.

use event_checkin_frontend::api::DeveloperProfile;
use event_checkin_frontend::api::PostEventRegisterData;

fn profile(display_name: &str, contact_name: &str) -> DeveloperProfile {
    DeveloperProfile {
        display_name: display_name.to_string(),
        contact_name: contact_name.to_string(),
        ..Default::default()
    }
}

#[test]
fn the_profile_name_wins() {
    assert_eq!(profile("Nok", "Old Name").registration_name(), "Nok");
}

#[test]
fn the_last_registration_name_fills_an_empty_profile() {
    assert_eq!(profile("  ", "Nok").registration_name(), "Nok");
}

#[test]
fn nobody_on_file_means_the_form() {
    assert_eq!(profile("", " ").registration_name(), "");
}

#[test]
fn an_older_worker_without_contact_fields_still_parses() {
    let p: DeveloperProfile =
        serde_json::from_str(r#"{"email":"a@b.c","display_name":"Nok"}"#).expect("parse");
    assert_eq!(p.contact_name, "");
    assert_eq!(p.registration_name(), "Nok");
}

#[test]
fn the_response_carries_the_event_for_the_ticket() {
    let d: PostEventRegisterData =
        serde_json::from_str(r#"{"attendee_id":"att-1","message":"ok","event_id":"evt-9"}"#)
            .expect("parse");
    assert_eq!(d.event_id.as_deref(), Some("evt-9"));
    let old: PostEventRegisterData =
        serde_json::from_str(r#"{"attendee_id":"att-1","message":"ok"}"#).expect("parse");
    assert_eq!(
        old.event_id, None,
        "no event_id falls back to the done card"
    );
}

/// The quick path must send consent and nothing that rewrites the profile:
/// stack and interests are arrays on the profile, strings on this form.
#[test]
fn the_quick_submit_sends_only_name_contact_and_consent() {
    let src = include_str!("../src/pages/public/post_event_register.rs");
    let quick = src
        .split("RegState::Quick | RegState::Submitting if quick_mode")
        .nth(1)
        .and_then(|s| s.split("RegState::Quick | RegState::Form").next())
        .expect("the quick card arm");
    assert!(quick.contains("consent_given: true"));
    for field in [
        "experience_level:",
        "tech_stack:",
        "interests:",
        "profile_fields:",
    ] {
        assert!(!quick.contains(field), "quick path must not send {field}");
    }
}

/// Someone who registered to watch back can never check in, do the quest or
/// claim a badge, so their ticket must not promise any of it.
#[test]
fn the_watch_back_ticket_has_no_timeline_or_claim() {
    let src = include_str!("../src/pages/ticket/online_view.rs");
    assert!(src.contains("let retrospective = utils::is_retrospective(&participation);"));
    let timeline = src
        .split("// 3. Timeline")
        .nth(1)
        .and_then(|s| s.split("<Timeline").next())
        .expect("timeline block");
    assert!(
        timeline.contains("if retrospective {"),
        "timeline must skip retrospective"
    );
    let nft = src
        .split("// 4. NFT section")
        .nth(1)
        .and_then(|s| s.split("claimed {").next())
        .expect("nft block");
    assert!(
        nft.contains("if retrospective {"),
        "claim cards must skip retrospective"
    );
    assert!(src.contains("ticket.hero.retro_title"));
}
