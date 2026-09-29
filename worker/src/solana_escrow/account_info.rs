//! One `getAccountInfo` read shared by every on-chain account check.
//!
//! JSON-RPC failures still use HTTP 200. Four hand-rolled copies of this call
//! lived in `wire.rs` and only one checked the `error` field; the other three
//! read an RPC error (a missing API key, a rate limit) as "account absent".
//! For the escrow reset in `handlers/events/update.rs` that meant "PDA closed,
//! reset allowed". Every caller now goes through [`account_value`], which reads
//! the response with [`super::json_rpc::rpc_result`].
//!
//! Errors never include the RPC URL: it carries the provider API key.

use super::EscrowError;

/// The `result.value` of a `getAccountInfo` response: `Ok(None)` when the
/// account does not exist, `Err` when the RPC answered with an error or with
/// no `result` at all.
pub fn account_value(json: serde_json::Value) -> Result<Option<serde_json::Value>, EscrowError> {
    let result = super::json_rpc::rpc_result(&json).map_err(EscrowError::RpcFailed)?;
    match result.get("value") {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => Ok(Some(value.clone())),
    }
}

/// Read `account_b58` with `getAccountInfo` (base64, confirmed). `request_id`
/// labels the call in provider logs.
pub(crate) async fn get_account_info(
    rpc_url: &str,
    request_id: &str,
    account_b58: &str,
) -> Result<Option<serde_json::Value>, EscrowError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "getAccountInfo",
        "params": [account_b58, { "encoding": "base64", "commitment": "confirmed" }]
    });
    let json_body = serde_json::to_string(&body)
        .map_err(|e| EscrowError::RpcFailed(format!("serialize: {e}")))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| EscrowError::RpcFailed(format!("headers: {e:?}")))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| EscrowError::RpcFailed(format!("request: {e:?}")))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("fetch: {e:?}")))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(EscrowError::RpcFailed(format!("HTTP {status}: {text}")));
    }

    let text = response
        .text()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("read body: {e:?}")))?;

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| EscrowError::RpcFailed(format!("parse json: {e}")))?;

    account_value(json)
}
