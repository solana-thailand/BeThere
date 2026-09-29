//! The `result` of a Solana JSON-RPC response, or why there is none.
//!
//! JSON-RPC failures (a bad API key, a rate limit, an unknown method) arrive
//! with HTTP 200 and an `error` object instead of `result`. A reader that only
//! looks at `result` sees "nothing there": an absent account, a missing
//! transaction. Every reader goes through [`rpc_result`] so an RPC failure is
//! always an `Err`.

/// The `result` field of `json`: `Err` when the RPC answered with an `error`
/// or with neither field. A `null` result (an unknown transaction, an absent
/// account) is `Ok(&Value::Null)`, not an error.
pub fn rpc_result(json: &serde_json::Value) -> Result<&serde_json::Value, String> {
    if let Some(error) = json.get("error") {
        return Err(format!("RPC error: {error}"));
    }
    json.get("result")
        .ok_or_else(|| "RPC response has no result".to_string())
}

/// A POST of one JSON-RPC 2.0 call to `rpc_url`. Every Solana RPC caller
/// builds its request here; each keeps its own send, timeout and retry
/// policy. `id` labels the call in provider logs.
pub fn post_request(
    rpc_url: &str,
    id: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<worker::Request, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let json_body =
        serde_json::to_string(&body).map_err(|e| format!("serialize {method} request: {e}"))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("set {method} headers: {e:?}"))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| format!("create {method} request: {e:?}"))
}
