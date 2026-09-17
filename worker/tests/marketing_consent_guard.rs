//! Answering the post-event survey must not withdraw marketing consent.
//!
//! Issue 115. The feedback page never asks about marketing, so its request
//! carries no `consent_marketing`. The worker read that absence as "no"
//! (`unwrap_or(false)`), and both attendee upserts overwrote the stored answer
//! *and re-dated it* (`consent_marketing = excluded.consent_marketing`). Every
//! survey submitted recorded a dated withdrawal nobody made, once per event
//! answered — 11 people in production by 2026-09-17, each withdrawn exactly as
//! many times as they answered, and `developer_profiles.consent_outreach` was
//! reset to `0` by the same path.
//!
//! The rule now: `consent_marketing` has three states end to end. A form that
//! shows the box sends `Some(checked)`; a form that does not ask sends `None`;
//! the worker binds `None` as SQL `NULL` and keeps what is stored. These checks
//! read the source because the failure is silent at runtime — a wrong row, not
//! an error — and the SQL's behaviour was verified against SQLite when written.

use std::{fs, path::Path};

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
}

/// The body of `fn name` up to the next top-level `fn`.
fn function<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("fn {name}("))
        .unwrap_or_else(|| panic!("fn {name} exists"));
    let rest = &source[start + 1..];
    let end = rest
        .find("\npub(crate) async fn ")
        .map_or(rest.len(), |i| i + 1);
    &source[start..start + end]
}

#[test]
fn both_upserts_keep_the_stored_answer_when_none_was_given() {
    let writes = read("src/db/attendees/writes.rs");
    for (name, slot) in [
        ("upsert_attendee", "?9"),
        ("upsert_post_event_attendee", "?8"),
    ] {
        let body = function(&writes, name);
        assert!(
            !body.contains("consent_marketing = excluded.consent_marketing"),
            "{name}: overwriting from `excluded` erases the stored answer when none was given"
        );
        assert!(
            !body.contains("unwrap_or(false)"),
            "{name}: absent is not a refusal"
        );
        for needle in [
            format!("COALESCE({slot}, 0)"),
            format!("CASE WHEN {slot} IS NULL THEN NULL ELSE datetime('now') END"),
            format!("consent_marketing = COALESCE({slot}, attendees.consent_marketing)"),
            format!(
                "consent_marketing_at = CASE WHEN {slot} IS NULL THEN attendees.consent_marketing_at"
            ),
        ] {
            assert!(body.contains(&needle), "{name}: missing `{needle}`");
        }
        assert!(
            body.contains("consent_bind(consent_marketing)"),
            "{name}: must bind None as NULL"
        );
    }
    assert!(
        writes.contains("None => D1Type::Null"),
        "consent_bind must map None to NULL"
    );
}

#[test]
fn handlers_pass_the_option_through() {
    for file in [
        "src/handlers/register/post_event.rs",
        "src/handlers/register/signup.rs",
    ] {
        let src = read(file);
        assert!(
            !src.contains("consent_marketing.unwrap_or"),
            "{file}: collapsing the Option before the write is the bug"
        );
    }
}

#[test]
fn the_developer_profile_is_written_only_when_asked() {
    let contact = read("src/handlers/register/contact.rs");
    let guard = contact
        .find("if let Some(consent) = consent_marketing")
        .expect("consent_outreach write is guarded on an answer");
    let write = contact
        .find("\"consent_outreach\"")
        .expect("consent_outreach is still written when asked");
    assert!(
        guard < write,
        "the consent_outreach upsert must sit inside the guard"
    );
}

#[test]
fn clients_state_what_they_asked_and_nothing_else() {
    let feedback = read("../frontend-leptos/src/pages/public/feedback.rs");
    assert!(
        feedback.contains("consent_marketing: None,"),
        "the survey asks nothing about marketing and must say so"
    );
    for file in [
        "../frontend-leptos/src/pages/public/post_event_register.rs",
        "../frontend-leptos/src/pages/public_event/registration_form.rs",
    ] {
        let src = read(file);
        let line = src
            .lines()
            .find(|l| l.trim_start().starts_with("consent_marketing:"))
            .unwrap_or_else(|| panic!("{file} sends consent_marketing"));
        assert!(
            line.contains("Some(") && !line.contains("None"),
            "{file}: an unticked box shown to the user is a stated no, not an absent answer"
        );
    }
    let post_event = read("../frontend-leptos/src/pages/public/post_event_register.rs");
    assert!(
        post_event.contains("let (consent_marketing, set_consent_marketing) = signal(false);"),
        "a pre-ticked marketing box is not consent (PDPA s.19)"
    );
}
