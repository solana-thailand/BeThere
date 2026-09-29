//! Wallet error messages come from the EN/TH catalog (`.plans/037` §2):
//! the category order is pinned, and every category has Thai text.

use event_checkin_frontend::i18n::Locale;
use event_checkin_frontend::pages::escrow_init::ClusterMismatch;
use event_checkin_frontend::wallet_error::{
    WalletError, WalletFailure, classify, user_friendly_message,
};

fn err(code: Option<i32>, msg: &str, logs: Option<Vec<&str>>) -> WalletError {
    WalletError {
        code,
        raw_message: msg.to_string(),
        logs: logs.map(|l| l.into_iter().map(str::to_string).collect()),
    }
}

#[test]
fn classify_first_match_wins() {
    let cases = [
        (
            err(Some(-32603), "User rejected", None),
            WalletFailure::Internal,
        ),
        (err(Some(4001), "", None), WalletFailure::Rejected),
        (
            err(None, "insufficient lamports", None),
            WalletFailure::InsufficientBalance,
        ),
        (
            err(
                None,
                "Simulation failed",
                Some(vec!["Program log: insufficient funds"]),
            ),
            WalletFailure::SimulationInsufficientTokens,
        ),
        (
            err(None, "Simulation failed", None),
            WalletFailure::SimulationFailed,
        ),
        (err(None, "RPC timeout", None), WalletFailure::Network),
        (
            err(None, "Unknown wallet error", None),
            WalletFailure::Unknown,
        ),
        (err(None, "", None), WalletFailure::Unknown),
        (
            err(None, "blockhash expired", None),
            WalletFailure::Other("blockhash expired".into()),
        ),
    ];
    for (e, want) in cases {
        assert_eq!(classify(&e), want, "{:?}", e.raw_message);
    }
}

#[test]
fn every_category_is_translated() {
    let samples = [
        err(Some(-32603), "", None),
        err(Some(4001), "", None),
        err(None, "insufficient", None),
        err(None, "Simulation failed", Some(vec!["insufficient"])),
        err(None, "Simulation failed", None),
        err(None, "network down", None),
        err(None, "", None),
        err(None, "blockhash expired", None),
    ];
    for e in samples {
        let en = user_friendly_message(&e, Locale::en);
        let th = user_friendly_message(&e, Locale::th);
        assert!(!en.is_empty(), "{:?}", e.raw_message);
        assert_ne!(en, th, "no Thai text for {:?}", e.raw_message);
    }
}

#[test]
fn raw_message_is_interpolated() {
    let e = err(None, "blockhash expired", None);
    assert_eq!(
        user_friendly_message(&e, Locale::en),
        "Transaction failed: blockhash expired"
    );
    assert!(user_friendly_message(&e, Locale::th).ends_with("blockhash expired"));
}

#[test]
fn cluster_mismatch_names_both_clusters() {
    let m = ClusterMismatch {
        wallet_cluster: "mainnet-beta".into(),
        expected: "devnet".into(),
    };
    for locale in [Locale::en, Locale::th] {
        let text = m.message(locale);
        assert!(text.contains("mainnet-beta"), "{text}");
        assert_eq!(text.matches("devnet").count(), 2, "{text}");
    }
}
