//! Issue 180: a mint the provider accepted but has not confirmed is not a
//! failure. The claim path must answer 504 with a fixed body, so the page can
//! retry the same claim (which resumes the mint) instead of saying "failed".

use event_checkin_domain::models::error::{AppError, MintError, UPSTREAM_PENDING_MESSAGE};

const JOB: &str = "crossmint mint still pending after 12 polls (nft_id=2a43be06)";

#[test]
fn pending_mint_becomes_a_504_with_a_fixed_public_body() {
    let error = MintError::Pending(JOB.into()).into_resumable_app_error("crossmint");
    assert!(matches!(&error, AppError::UpstreamPending(detail) if detail == JOB));
    assert_eq!(error.status_code(), 504);
    assert_eq!(error.public_message(), UPSTREAM_PENDING_MESSAGE);
    assert!(!error.public_message().contains("nft_id"), "job id leaked");
    assert!(
        error.to_string().contains(JOB),
        "the log line lost the detail"
    );
}

#[test]
fn failed_mint_stays_a_502_from_the_service() {
    let error = MintError::Failed("crossmint returned HTTP 402".into())
        .into_resumable_app_error("crossmint");
    assert_eq!(error.status_code(), 502);
    assert_eq!(
        error.public_message(),
        "external service error: crossmint returned 502"
    );
}

#[test]
fn a_plain_string_error_is_a_failure_not_pending() {
    let error: MintError = String::from("D1 write failed").into();
    assert_eq!(error, MintError::Failed("D1 write failed".into()));
}

#[test]
fn scrubbing_the_detail_keeps_the_variant() {
    let wallet = "9Bz7exampleWallet";
    let scrub = |text: &str| text.replace(wallet, "[recipient]");
    assert_eq!(
        MintError::Pending(format!("owner {wallet}")).map_detail(scrub),
        MintError::Pending("owner [recipient]".into())
    );
    assert_eq!(
        MintError::Failed(format!("owner {wallet}")).map_detail(scrub),
        MintError::Failed("owner [recipient]".into())
    );
}
