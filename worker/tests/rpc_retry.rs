//! `.issues/156` follow-up — which Solana RPC failures earn the one retry of a
//! blockhash fetch, and how long the jittered pause can be.

use event_checkin_worker::solana_escrow::rpc_retry::{
    RETRY_BASE_DELAY_MS, RETRY_JITTER_MS, is_transient_status, retry_delay_ms,
};

#[test]
fn rate_limit_and_server_errors_are_transient() {
    for status in [429, 500, 502, 503, 504, 599] {
        assert!(is_transient_status(status), "{status} should retry");
    }
}

#[test]
fn client_errors_and_success_are_final() {
    for status in [200, 204, 301, 400, 401, 403, 404, 413, 428, 430, 600] {
        assert!(!is_transient_status(status), "{status} should not retry");
    }
}

#[test]
fn delay_spans_base_to_base_plus_jitter() {
    assert_eq!(retry_delay_ms(0.0), RETRY_BASE_DELAY_MS);
    assert_eq!(retry_delay_ms(250.0), RETRY_BASE_DELAY_MS + 250);
    assert_eq!(
        retry_delay_ms(RETRY_JITTER_MS as f64),
        RETRY_BASE_DELAY_MS + RETRY_JITTER_MS
    );
}

#[test]
fn neighbouring_failures_retry_at_different_times() {
    let now = 1_790_546_831_000.0;
    assert_ne!(retry_delay_ms(now), retry_delay_ms(now + 1.0));
    assert_ne!(retry_delay_ms(now), retry_delay_ms(now + 37.0));
}

#[test]
fn every_input_stays_in_bounds() {
    let max = RETRY_BASE_DELAY_MS + RETRY_JITTER_MS;
    let odd = [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.4, 1e15];
    let clock = (0..2_000).map(|ms| 1_790_546_831_000.0 + f64::from(ms));
    for at in odd.into_iter().chain(clock) {
        let delay = retry_delay_ms(at);
        assert!(
            (RETRY_BASE_DELAY_MS..=max).contains(&delay),
            "{at} gave {delay}"
        );
    }
}
