//! `.issues/145`: the refund-proof URL is validated where it is written and
//! filtered where it is rendered. The handler and the frontend are not callable
//! from here, so pin the calls.

#[test]
fn refund_proof_validator_checks_links_and_images() {
    let src = include_str!("../src/handlers/deposit/thb/handlers/refund.rs");
    let body = &src[src
        .find("fn validate_refund_proof(")
        .expect("refund.rs must define validate_refund_proof")..];
    let body = &body[..body.find("\n}\n").expect("end of validate_refund_proof")];
    assert!(
        body.contains("safe_document_link(proof)"),
        "a proof link must be checked with safe_document_link"
    );
    assert!(
        body.contains("validate_slip_url(proof)"),
        "an uploaded proof image must be checked like a slip"
    );
}

/// Both cash-refund handlers validate the proof before storing it. The batch
/// used to refund every verified deposit with no proof at all.
#[test]
fn every_cash_refund_validates_the_proof_before_storing_it() {
    let src = include_str!("../src/handlers/deposit/thb/handlers/refund.rs");
    for handler in ["mark_refund_handler", "batch_thb_refund_handler"] {
        let start = src
            .find(&format!("pub async fn {handler}("))
            .unwrap_or_else(|| panic!("{handler} missing"));
        let rest = &src[start..];
        let body = &rest[..rest.find("\n}\n").expect("end of handler")];
        let check = body
            .find("validate_refund_proof(&body.refund_proof_url)?")
            .unwrap_or_else(|| panic!("{handler} must validate the refund proof"));
        let store = body
            .find("maybe_upload_to_r2(")
            .unwrap_or_else(|| panic!("{handler} stores the proof via maybe_upload_to_r2"));
        let settle = body
            .find("try_settle_refund(")
            .unwrap_or_else(|| panic!("{handler} settles via the CAS"));
        assert!(
            check < store && store < settle,
            "{handler}: validate, store, then settle"
        );
    }
}

/// The batch refund writes D1 `attendees` like the single refund does.
#[test]
fn batch_refund_marks_the_attendee_in_d1() {
    let src = include_str!("../src/handlers/deposit/thb/handlers/refund.rs");
    let rest = &src[src.find("pub async fn batch_thb_refund_handler(").unwrap()..];
    let body = &rest[..rest.find("\n}\n").unwrap()];
    assert!(body.contains("crate::db::attendees::mark_refund("));
}

#[test]
fn frontend_renders_only_safe_proof_links() {
    for (path, src) in [
        (
            "ticket/action_cards.rs",
            include_str!("../../frontend-leptos/src/pages/ticket/action_cards.rs"),
        ),
        (
            "admin_deposit.rs",
            include_str!("../../frontend-leptos/src/pages/admin_deposit.rs"),
        ),
    ] {
        assert!(
            src.contains("safe_document_link"),
            "{path} must filter refund_proof_url through safe_document_link"
        );
    }
}
