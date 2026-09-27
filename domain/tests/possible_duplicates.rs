//! `possible_duplicates` — the roster's possible-duplicate hint (plan 025 §7.9).
//! Rows in one event that share a name, wallet or contact handle under
//! different emails are flagged both ways; the same email never is.

use event_checkin_domain::models::attendee::{
    Attendee, CheckInStatus, DuplicateReason, possible_duplicates,
};

fn attendee(id: &str, name: &str, email: &str) -> Attendee {
    Attendee {
        api_id: id.to_string(),
        first_name: String::new(),
        last_name: String::new(),
        name: name.to_string(),
        email: email.to_string(),
        ticket_name: String::new(),
        approval_status: CheckInStatus::Approved,
        participation_type: "general".to_string(),
        registration_date: None,
        phone: None,
        contact_channel: None,
        contact_handle: None,
        deposit_agreed: None,
        deposit_method: None,
        deposit_amount: None,
        deposit_tx_signature: None,
        deposit_verified: None,
        checked_in_at: None,
        checked_in_by: None,
        solana_address: None,
        qr_code_url: None,
        claim_token: None,
        claimed_at: None,
        nft_proof_url: None,
        bank_account: None,
        bank_name: None,
        account_name: None,
        refund_status: None,
        refund_link: None,
        send_email_status: None,
        row_index: 0,
    }
}

fn reasons(
    found: &std::collections::HashMap<
        String,
        Vec<event_checkin_domain::models::attendee::DuplicateMatch>,
    >,
    id: &str,
) -> Vec<(String, DuplicateReason)> {
    found
        .get(id)
        .map(|list| list.iter().map(|m| (m.api_id.clone(), m.reason)).collect())
        .unwrap_or_default()
}

#[test]
fn same_name_under_two_emails_is_flagged_both_ways() {
    let rows = [
        attendee("a", "Somchai  Jaidee", "a@x.com"),
        attendee("b", "somchai jaidee", "b@y.com"),
    ];
    let found = possible_duplicates(&rows);
    assert_eq!(
        reasons(&found, "a"),
        vec![("b".into(), DuplicateReason::SameName)]
    );
    assert_eq!(
        reasons(&found, "b"),
        vec![("a".into(), DuplicateReason::SameName)]
    );
    assert_eq!(found["a"][0].name, "somchai jaidee");
}

#[test]
fn same_email_is_never_flagged() {
    let rows = [
        attendee("a", "Somchai", "a@x.com"),
        attendee("b", "Somchai", "A@X.com"),
    ];
    assert!(possible_duplicates(&rows).is_empty());
}

#[test]
fn wallet_matches_exactly_not_case_folded() {
    let mut a = attendee("a", "One", "a@x.com");
    let mut b = attendee("b", "Two", "b@x.com");
    let mut c = attendee("c", "Three", "c@x.com");
    a.solana_address = Some("7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU".into());
    b.solana_address = Some(" 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU ".into());
    c.solana_address = Some("7xkxtg2cw87d97txjsdpbd5jbkhetqa83tzrujosgasu".into());
    let found = possible_duplicates(&[a, b, c]);
    assert_eq!(
        reasons(&found, "a"),
        vec![("b".into(), DuplicateReason::SameWallet)]
    );
    assert!(
        !found.contains_key("c"),
        "a case-folded wallet is a different wallet"
    );
}

#[test]
fn handle_matches_per_channel_ignoring_case_and_at_sign() {
    let mut a = attendee("a", "One", "a@x.com");
    let mut b = attendee("b", "Two", "b@x.com");
    let mut c = attendee("c", "Three", "c@x.com");
    a.contact_channel = Some("telegram".into());
    a.contact_handle = Some("@Somchai_J".into());
    b.contact_channel = Some("Telegram".into());
    b.contact_handle = Some("somchai_j".into());
    c.contact_channel = Some("github".into());
    c.contact_handle = Some("somchai_j".into());
    let found = possible_duplicates(&[a, b, c]);
    assert_eq!(
        reasons(&found, "a"),
        vec![("b".into(), DuplicateReason::SameHandle)]
    );
    assert!(
        !found.contains_key("c"),
        "the same handle on another channel is another account"
    );
}

#[test]
fn several_reasons_are_all_listed_in_a_stable_order() {
    let mut a = attendee("a", "Somchai Jaidee", "a@x.com");
    let mut b = attendee("b", "Somchai Jaidee", "b@x.com");
    a.solana_address = Some("W1".into());
    b.solana_address = Some("W1".into());
    let found = possible_duplicates(&[a, b]);
    assert_eq!(
        reasons(&found, "a"),
        vec![
            ("b".into(), DuplicateReason::SameName),
            ("b".into(), DuplicateReason::SameWallet),
        ]
    );
}

#[test]
fn blank_and_one_letter_fields_are_not_signals() {
    let mut a = attendee("a", "J", "a@x.com");
    let mut b = attendee("b", "j", "b@x.com");
    a.solana_address = Some("  ".into());
    b.solana_address = Some("".into());
    a.contact_handle = Some("@".into());
    b.contact_handle = Some("@".into());
    assert!(possible_duplicates(&[a, b]).is_empty());
}

#[test]
fn distinct_people_are_not_flagged() {
    let rows = [
        attendee("a", "Somchai Jaidee", "a@x.com"),
        attendee("b", "Suda Jaidee", "b@x.com"),
    ];
    assert!(possible_duplicates(&rows).is_empty());
}
