//! The registration consent authorizes a payment, so it must say in what
//! currency (`.issues/166`). A THB-only event with escrow open used to read
//! "authorize the 500 commitment deposit".

use event_checkin_frontend::utils::money::deposit_consent_label;

const THB: u64 = 500;
const USDC: u64 = 15_000_000;

#[test]
fn every_non_zero_arm_names_a_currency() {
    for escrow_closed in [false, true] {
        for (thb, usdc) in [(THB, 0), (0, USDC), (THB, USDC)] {
            let label = deposit_consent_label(thb, usdc, escrow_closed);
            assert!(
                label.contains("Baht") || label.contains("USDC"),
                "no currency in {label:?} (thb={thb}, usdc={usdc}, closed={escrow_closed})"
            );
        }
    }
}

#[test]
fn thb_only_open_escrow_reads_baht() {
    assert_eq!(deposit_consent_label(THB, 0, false), "500 Baht");
}

#[test]
fn both_currencies_open_escrow_shows_both() {
    assert_eq!(
        deposit_consent_label(THB, USDC, false),
        "15.00 USDC (~500 Baht)"
    );
}

#[test]
fn closed_escrow_leads_with_thb() {
    assert_eq!(deposit_consent_label(THB, USDC, true), "500 Baht");
    assert_eq!(deposit_consent_label(0, USDC, true), "15.00 USDC");
}
