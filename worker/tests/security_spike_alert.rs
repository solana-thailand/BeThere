//! Security-spike alerting (`.plans/029` §2, ISO 27001 A.8.16).

use event_checkin_worker::spike::{
    SESSION_PROBE_PATH, SecuritySignal, SpikeCounter, SpikeRule, classify,
};

const RULE: SpikeRule = SpikeRule {
    threshold: 3,
    window_ms: 1_000,
    cooldown_ms: 10_000,
};

#[test]
fn classify_counts_429s_and_credentialed_401s_only() {
    use SecuritySignal::*;
    assert_eq!(
        classify(429, false, "/api/deposit/thb/upload"),
        Some(RateLimited)
    );
    assert_eq!(classify(429, true, SESSION_PROBE_PATH), Some(RateLimited));
    assert_eq!(
        classify(401, true, "/api/events"),
        Some(RejectedCredentials)
    );
    // A signed-out visitor's 401 is routine traffic.
    assert_eq!(classify(401, false, "/api/events"), None);
    // The session probe refuses expired cookies on every page load.
    assert_eq!(classify(401, true, SESSION_PROBE_PATH), None);
    for code in [200, 204, 302, 400, 403, 404, 500, 503] {
        assert_eq!(classify(code, true, "/api/events"), None, "{code}");
    }
}

#[test]
fn alerts_once_when_the_threshold_is_reached_inside_one_window() {
    let mut c = SpikeCounter::new(RULE);
    assert_eq!(c.record(100), None);
    assert_eq!(c.record(200), None);
    assert_eq!(c.record(300), Some(3));
    // Further events in the same window do not re-alert.
    assert_eq!(c.record(400), None);
    assert_eq!(c.record(500), None);
}

#[test]
fn a_slow_trickle_never_alerts() {
    let mut c = SpikeCounter::new(RULE);
    // Two per window, forever: always under the threshold of 3.
    for window in 0..50u64 {
        let base = window * RULE.window_ms;
        assert_eq!(c.record(base + 10), None);
        assert_eq!(c.record(base + 20), None);
    }
}

#[test]
fn cooldown_suppresses_a_sustained_attack_then_rearms() {
    let mut c = SpikeCounter::new(RULE);
    let burst =
        |c: &mut SpikeCounter, start: u64| (0..3).filter_map(|i| c.record(start + i)).count();
    assert_eq!(burst(&mut c, 0), 1, "first spike alerts");
    assert_eq!(burst(&mut c, 2_000), 0, "inside the cooldown: silent");
    assert_eq!(burst(&mut c, 5_000), 0, "still inside the cooldown");
    assert_eq!(burst(&mut c, 10_000), 1, "cooldown elapsed: alerts again");
}

#[test]
fn a_clock_that_moves_backwards_starts_a_new_window() {
    let mut c = SpikeCounter::new(RULE);
    c.record(5_000);
    c.record(5_001);
    // Earlier timestamp: must not underflow or carry the old count over.
    assert_eq!(c.record(1_000), None);
    assert_eq!(c.record(1_001), None);
    assert_eq!(c.record(1_002), Some(3));
}

#[test]
fn production_rules_are_sane() {
    for signal in SecuritySignal::ALL {
        let rule = signal.rule();
        assert!(
            rule.threshold >= 5,
            "{signal:?}: a threshold this low pages on noise"
        );
        assert!(
            rule.cooldown_ms >= rule.window_ms,
            "{signal:?}: cooldown shorter than a window"
        );
        assert_eq!(SecuritySignal::ALL[signal.index()], signal);
    }
}

#[test]
fn alert_layer_wires_the_detector_and_keeps_secrets_out() {
    let src = include_str!("../src/middleware/alert.rs");
    assert!(src.contains("classify(code, had_credentials, &path)"));
    assert!(src.contains("extract_token_from_headers(req.headers()).is_some()"));
    // The message may carry the path, never the token, IP or query string.
    let text = &src[src.find("security spike*").expect("spike message")..];
    let text = &text[..text.find(");").expect("end of format!")];
    for forbidden in ["token", "ip", "query", "uri()", "cookie"] {
        assert!(
            !text.to_lowercase().contains(&format!("{{{forbidden}")),
            "spike alert must not interpolate {forbidden}"
        );
    }
}
