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
