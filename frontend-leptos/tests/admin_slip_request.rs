//! `.issues/154`: the admin record-slip request must always carry
//! `auto_verify`. The worker defaults a missing field to `true`, so omitting
//! `false` silently verified slips the organizer meant to leave pending.

use event_checkin_frontend::api::AdminSlipUploadRequest;

fn to_json(auto_verify: bool) -> serde_json::Value {
    let req = AdminSlipUploadRequest {
        auto_verify,
        ..AdminSlipUploadRequest::default()
    };
    serde_json::to_value(&req).expect("serialize AdminSlipUploadRequest")
}

#[test]
fn unchecked_auto_verify_is_sent_as_false() {
    assert_eq!(
        to_json(false)["auto_verify"],
        serde_json::Value::Bool(false)
    );
}

#[test]
fn checked_auto_verify_is_sent_as_true() {
    assert_eq!(to_json(true)["auto_verify"], serde_json::Value::Bool(true));
}
