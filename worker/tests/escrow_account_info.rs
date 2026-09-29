//! Every `getAccountInfo` read goes through `account_value`. A JSON-RPC error
//! arrives with HTTP 200; it must be an `Err`, never "account absent". Three of
//! four hand-rolled copies in `solana_escrow/wire.rs` read it as absent, which
//! let an escrow reset through on an RPC failure.

use event_checkin_worker::solana_escrow::EscrowError;
use event_checkin_worker::solana_escrow::account_info::account_value;
use serde_json::json;

#[test]
fn rpc_error_is_an_error_not_an_absent_account() {
    let body = json!({
        "jsonrpc": "2.0",
        "id": "bethere-check-pda-available",
        "error": { "code": -32401, "message": "invalid api key provided" }
    });
    match account_value(body) {
        Err(EscrowError::RpcFailed(msg)) => assert!(msg.contains("invalid api key"), "{msg}"),
        other => panic!("expected RpcFailed, got {other:?}"),
    }
}

#[test]
fn response_without_result_is_an_error() {
    let body = json!({ "jsonrpc": "2.0", "id": "x" });
    assert!(matches!(
        account_value(body),
        Err(EscrowError::RpcFailed(_))
    ));
}

#[test]
fn null_value_is_an_absent_account() {
    let body = json!({
        "jsonrpc": "2.0",
        "id": "x",
        "result": { "context": { "slot": 1 }, "value": null }
    });
    assert!(matches!(account_value(body), Ok(None)));
}

#[test]
fn present_value_is_returned() {
    let body = json!({
        "jsonrpc": "2.0",
        "id": "x",
        "result": {
            "context": { "slot": 1 },
            "value": { "owner": "C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T", "data": ["", "base64"] }
        }
    });
    let value = account_value(body).expect("ok").expect("some");
    assert_eq!(
        value["owner"],
        "C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T"
    );
}

/// No hand-rolled `getAccountInfo` may come back: each copy would have to
/// re-learn the HTTP-200 error rule.
#[test]
fn get_account_info_is_only_built_in_the_shared_module() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/solana_escrow/wire.rs"
    ))
    .expect("read wire.rs");
    assert!(
        !src.contains("\"method\": \"getAccountInfo\""),
        "wire.rs builds its own getAccountInfo request; use account_info::get_account_info"
    );
}

/// The escrow reset may tell the organizer "the escrow still exists" only
/// when the check found an account there (`AccountNotFound`). An RPC failure
/// or a wallet that does not derive a PDA once fell into a catch-all arm that
/// said so too.
#[test]
fn reset_says_still_exists_only_for_an_existing_account() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/handlers/events/update.rs"
    ))
    .expect("read update.rs");
    let message = src
        .find("on-chain escrow account still exists")
        .expect("reset message moved; update this guard");
    let arm = src[..message]
        .rfind("Err(e")
        .expect("no match arm before the message");
    assert!(
        src[arm..message]
            .starts_with("Err(e @ crate::solana_escrow::EscrowError::AccountNotFound(_))"),
        "the \"still exists\" reset message must sit in the AccountNotFound arm, not a catch-all"
    );
}
