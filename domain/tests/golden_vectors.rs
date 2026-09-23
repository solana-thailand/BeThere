//! Pinned golden vectors (`.plans/030` §3): USDC conversions and the on-chain
//! event id. The escrow PDA half of the same fixture is asserted in
//! `worker/tests/golden_vectors_escrow.rs`.

use event_checkin_domain::money::{parse_usdc_atomic, usdc_ui_to_atomic};
use event_checkin_domain::onchain::on_chain_event_id;
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    usdc_parse: Vec<UsdcParse>,
    usdc_ui: Vec<UsdcUi>,
    on_chain_event_id: Vec<OnChainId>,
}

#[derive(Deserialize)]
struct UsdcParse {
    input: String,
    atomic: Option<u64>,
}

#[derive(Deserialize)]
struct UsdcUi {
    ui: f64,
    atomic: Option<u64>,
}

#[derive(Deserialize)]
struct OnChainId {
    event_id: String,
    id: u64,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/golden_vectors.json"))
        .expect("golden_vectors.json must parse")
}

#[test]
fn usdc_strings_parse_exactly() {
    let cases = fixture().usdc_parse;
    assert!(cases.len() >= 20, "fixture lost its usdc_parse cases");
    for case in cases {
        assert_eq!(
            parse_usdc_atomic(&case.input),
            case.atomic,
            "parse_usdc_atomic({:?})",
            case.input
        );
    }
}

#[test]
fn usdc_ui_amounts_round_to_nearest() {
    let cases = fixture().usdc_ui;
    assert!(cases.len() >= 8, "fixture lost its usdc_ui cases");
    for case in cases {
        assert_eq!(
            usdc_ui_to_atomic(case.ui),
            case.atomic,
            "usdc_ui_to_atomic({})",
            case.ui
        );
    }
}

/// Issue 146: the old `(x * 1e6) as u64` got these wrong. Proves the fixture
/// exercises the truncation class instead of only easy values.
#[test]
fn fixture_covers_values_that_truncation_gets_wrong() {
    let truncating = |ui: f64| (ui * 1_000_000.0) as u64;
    let caught = fixture()
        .usdc_ui
        .iter()
        .filter(|case| {
            case.atomic
                .is_some_and(|atomic| truncating(case.ui) != atomic)
        })
        .count();
    assert!(caught >= 3, "only {caught} usdc_ui cases catch truncation");
}

#[test]
fn usdc_ui_rejects_non_finite() {
    for ui in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e20] {
        assert_eq!(usdc_ui_to_atomic(ui), None, "usdc_ui_to_atomic({ui})");
    }
}

/// Every two-decimal amount up to the 1,000 USDC cap parses and converts to
/// the same atomic value by both paths.
#[test]
fn every_cent_up_to_the_cap_agrees_across_both_paths() {
    for cents in 0..=100_000u64 {
        let text = format!("{}.{:02}", cents / 100, cents % 100);
        let expected = cents * 10_000;
        assert_eq!(parse_usdc_atomic(&text), Some(expected), "{text}");
        let ui: f64 = text.parse().expect("valid float");
        assert_eq!(usdc_ui_to_atomic(ui), Some(expected), "{text} as f64");
    }
}

#[test]
fn on_chain_event_ids_are_pinned() {
    let cases = fixture().on_chain_event_id;
    assert!(
        !cases.is_empty(),
        "fixture lost its on_chain_event_id cases"
    );
    for case in cases {
        assert_eq!(
            on_chain_event_id(&case.event_id),
            case.id,
            "{:?}: the on-chain id changed; every escrow PDA for it would be orphaned",
            case.event_id
        );
    }
}
