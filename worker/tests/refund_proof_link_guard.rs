//! `.issues/145`: the refund-proof URL is validated where it is written and
//! filtered where it is rendered. The handler and the frontend are not callable
//! from here, so pin the calls.

#[test]
fn mark_refund_validates_the_proof_before_storing_it() {
    let src = include_str!("../src/handlers/deposit/thb/handlers/refund.rs");
    let check = src
        .find("safe_document_link(&body.refund_proof_url)")
        .expect("mark_refund_handler must check the proof link");
    let data_check = src
        .find("validate_slip_url(&body.refund_proof_url)")
        .expect("an uploaded proof image must be checked like a slip");
    let store = src
        .find("maybe_upload_to_r2(")
        .expect("the proof is stored via maybe_upload_to_r2");
    assert!(
        check < store && data_check < store,
        "validate before storing"
    );
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
