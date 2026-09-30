//! Solana JSON-RPC errors arrive with HTTP 200. `rpc_result` turns them into an
//! `Err`; a reader that looked only at `result` saw a missing transaction or an
//! absent account instead. The escrow poller counted a failed `getTransaction`
//! as "no escrow event".

use event_checkin_worker::solana_escrow::json_rpc::rpc_result;
use serde_json::json;

#[test]
fn error_object_is_an_error() {
    let body = json!({
        "jsonrpc": "2.0",
        "id": "bethere-tx",
        "error": { "code": 429, "message": "Too many requests" }
    });
    let err = rpc_result(&body).expect_err("an RPC error must not read as a result");
    assert!(err.contains("Too many requests"), "{err}");
}

#[test]
fn null_result_is_ok_null() {
    let body = json!({ "jsonrpc": "2.0", "id": "bethere-tx", "result": null });
    assert_eq!(rpc_result(&body), Ok(&serde_json::Value::Null));
}

#[test]
fn present_result_is_returned() {
    let body =
        json!({ "jsonrpc": "2.0", "id": "bethere-poll", "result": [{ "signature": "abc" }] });
    assert_eq!(rpc_result(&body), Ok(&json!([{ "signature": "abc" }])));
}

#[test]
fn neither_result_nor_error_is_an_error() {
    let body = json!({ "jsonrpc": "2.0", "id": "bethere-tx" });
    assert!(rpc_result(&body).is_err());
}

/// The readers that once skipped the `error` field must keep going through
/// `rpc_result` rather than reading `result` by hand.
#[test]
fn rpc_readers_do_not_read_result_by_hand() {
    for path in [
        "src/escrow_indexer/poller.rs",
        "src/solana_escrow/account_info.rs",
        "src/solana_escrow/blockhash.rs",
    ] {
        let source = std::fs::read_to_string(path).expect(path);
        for by_hand in [r#".get("result")"#, r#"["result"]"#] {
            assert!(
                !source.contains(by_hand),
                "{path} reads `result` without checking `error`; use json_rpc::rpc_result"
            );
        }
        assert!(
            source.contains("rpc_result"),
            "{path} no longer uses rpc_result"
        );
    }
}

fn rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        match path.is_dir() {
            true => rust_sources(&path, out),
            false if path.extension().is_some_and(|e| e == "rs") => out.push(path),
            false => {}
        }
    }
}

/// Every Solana JSON-RPC request is built by `json_rpc::post_request`; a
/// hand-rolled envelope elsewhere is a second copy of the request plumbing.
#[test]
fn json_rpc_envelope_is_built_in_one_place() {
    let mut files = Vec::new();
    rust_sources(std::path::Path::new("src"), &mut files);
    assert!(files.len() > 50, "walked too few files: {}", files.len());
    let owner = std::path::Path::new("src/solana_escrow/json_rpc.rs");
    let offenders: Vec<_> = files
        .iter()
        .filter(|p| p.as_path() != owner)
        .filter(|p| {
            std::fs::read_to_string(p)
                .expect("read source")
                .contains(r#""jsonrpc": "2.0""#)
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "build JSON-RPC requests with json_rpc::post_request: {offenders:?}"
    );
    assert!(
        std::fs::read_to_string(owner)
            .expect("read json_rpc.rs")
            .contains(r#""jsonrpc": "2.0""#)
    );
}
