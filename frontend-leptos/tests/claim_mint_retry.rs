//! Issue 180: the claim page retries a pending mint (504) on its own a bounded
//! number of times, and never retries a real failure.

use event_checkin_frontend::pages::claim::mint_retry::{
    MAX_PENDING_RETRIES, MINT_PENDING_STATUS, MintFailure, classify, should_retry,
};

#[test]
fn a_504_is_pending_and_anything_else_is_a_failure() {
    assert_eq!(
        classify(MINT_PENDING_STATUS, "x".into()),
        MintFailure::StillPending
    );
    for status in [0, 400, 409, 429, 500, 502, 503] {
        assert_eq!(
            classify(status, "boom".into()),
            MintFailure::Failed("boom".into()),
            "status {status}"
        );
    }
}

#[test]
fn pending_retries_stop_at_the_budget() {
    let pending = MintFailure::StillPending;
    assert!(MAX_PENDING_RETRIES > 0);
    for done in 0..MAX_PENDING_RETRIES {
        assert!(should_retry(&pending, done), "retry {done}");
    }
    assert!(!should_retry(&pending, MAX_PENDING_RETRIES));
}

#[test]
fn a_failure_is_never_retried_automatically() {
    assert!(!should_retry(&MintFailure::Failed("402".into()), 0));
}
