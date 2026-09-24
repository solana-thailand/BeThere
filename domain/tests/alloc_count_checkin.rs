#![cfg(feature = "alloc_count")]
//! Zero-alloc audit for the pure part of the staff check-in path
//! (`POST /api/checkin/{id}`, `worker/src/handlers/checkin.rs`). Plan 030 §3.
//!
//! The handler is mostly I/O (KV, D1, Sheets). What runs in memory on every
//! scan is the eligibility gate (`Attendee::can_check_in`, or
//! `can_check_in_virtually` for the online track) and the `CheckInResponse`
//! serialization. Both must allocate nothing (the response into a reused
//! buffer). Until 2026-09-24 the gate allocated once per call, because
//! `ParticipationType::parse` lowered the string with `to_lowercase()`.
//!
//! Counts are native (System allocator). dlmalloc on wasm32 differs, so treat
//! them as relative: they catch a new heap step, not absolute cost.
//! The reject paths allocate (the error carries a `String`) and are not
//! audited, since a rejected scan is not the hot path.
//!
//! ```sh
//! cargo test -p event-checkin-domain --locked --features alloc_count \
//!     --test alloc_count_checkin -- --test-threads=1
//! ```

mod common;

use common::counting_alloc::{allocs_after_warmup, assert_installed};
use event_checkin_domain::models::api::CheckInResponse;
use event_checkin_domain::models::attendee::{Attendee, CheckInStatus};

fn attendee(participation_type: &str) -> Attendee {
    Attendee {
        api_id: "gst-0192f0c4".to_string(),
        first_name: "Somchai".to_string(),
        last_name: "Jaidee".to_string(),
        name: "Somchai Jaidee".to_string(),
        email: "somchai@example.com".to_string(),
        ticket_name: "General".to_string(),
        approval_status: CheckInStatus::Approved,
        participation_type: participation_type.to_string(),
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
        row_index: 2,
    }
}

#[test]
fn counting_allocator_is_installed() {
    assert_installed();
}

#[test]
fn in_person_gate_accept_path_is_zero_alloc() {
    for participation in [
        "",
        "In-Person",
        "In-Person (Physical Attendance)",
        "PHYSICAL",
    ] {
        let a = attendee(participation);
        let count = allocs_after_warmup(|| a.can_check_in());
        assert!(
            a.can_check_in().is_ok(),
            "{participation:?} must be accepted"
        );
        assert_eq!(
            count, 0,
            "can_check_in({participation:?}) allocated {count} times"
        );
    }
}

#[test]
fn virtual_gate_accept_path_is_zero_alloc() {
    for participation in ["Online", "Virtual (Zoom)"] {
        let a = attendee(participation);
        let count = allocs_after_warmup(|| a.can_check_in_virtually());
        assert!(
            a.can_check_in_virtually().is_ok(),
            "{participation:?} must be accepted"
        );
        assert_eq!(
            count, 0,
            "can_check_in_virtually({participation:?}) allocated {count} times"
        );
    }
}

#[test]
fn participation_parse_is_zero_alloc() {
    let a = attendee("Hybrid (Online + In-Person)");
    let count = allocs_after_warmup(|| (a.is_in_person(), a.counts_toward_online_track()));
    assert_eq!(count, 0, "participation parsing allocated {count} times");
}

#[test]
fn response_serialization_into_a_reused_buffer_is_zero_alloc() {
    let a = attendee("In-Person");
    let response = CheckInResponse {
        api_id: a.api_id.clone(),
        name: a.display_name().to_string(),
        checked_in_at: "2026-09-24T10:15:00.000000+00:00".to_string(),
        checked_in_by: "staff@example.com".to_string(),
        claim_token: Some("0192f0c4-7b3e-7cc1-9a55-3f1f5a3c2d10".to_string()),
        message: format!("{} checked in successfully", a.display_name()),
    };
    let mut buf = Vec::with_capacity(1024);
    let count = allocs_after_warmup(|| {
        buf.clear();
        serde_json::to_writer(&mut buf, &response).expect("serialize");
        buf.len()
    });
    println!(
        "CheckInResponse -> {} bytes JSON, {count} allocs",
        buf.len()
    );
    // A non-zero count means the response grew a heap step (a map, a
    // `to_string`, a nested `Value`).
    assert_eq!(
        count, 0,
        "CheckInResponse serialization allocated {count} times"
    );
}
