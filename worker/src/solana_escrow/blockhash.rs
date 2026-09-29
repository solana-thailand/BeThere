//! Recent blockhash for transaction builders: a per-isolate cache in front of
//! one `getLatestBlockhash` call, retried once on a transient failure.

use super::EscrowError;

/// How long one isolate reuses a fetched blockhash.
///
/// A blockhash is valid for 150 slots (roughly 60–90 s), and the wallet still
/// has to sign after we hand the transaction over, so the copy we serve must be
/// young. This used to be a 30 s KV entry, but KV rejects `expiration_ttl`
/// below 60, so every put failed and every build went to the RPC (.issues/156).
/// KV would be the wrong tier even at 60 s: an edge read can lag a write by up
/// to 60 s, so a served hash could be two minutes old.
const BLOCKHASH_CACHE_TTL_MS: f64 = 20_000.0;

type BlockhashIsolateCache =
    crate::isolate_cache::BoundedCache<String, crate::isolate_cache::Expiring<String>>;

thread_local! {
    /// The last blockhash per RPC URL, for this isolate only.
    static BLOCKHASH_IN_ISOLATE: std::cell::RefCell<BlockhashIsolateCache> =
        const { std::cell::RefCell::new(crate::isolate_cache::BoundedCache::new(2)) };
}

/// Recent blockhash from Solana RPC.
pub(crate) struct RecentBlockhash {
    /// The blockhash as base58 string.
    pub(crate) value: String,
}

/// Fetch the latest blockhash, reusing this isolate's copy while it is younger
/// than [`BLOCKHASH_CACHE_TTL_MS`].
pub(crate) async fn get_latest_blockhash(rpc_url: &str) -> Result<RecentBlockhash, EscrowError> {
    let now_ms = js_sys::Date::now();
    let cached = BLOCKHASH_IN_ISOLATE
        .with_borrow(|cache| cache.get(rpc_url).and_then(|entry| entry.get(now_ms)));
    if let Some(value) = cached {
        tracing::debug!("using cached blockhash");
        return Ok(RecentBlockhash { value });
    }

    let blockhash = fetch_blockhash_with_retry(rpc_url).await?;
    let expires_at_ms = now_ms + BLOCKHASH_CACHE_TTL_MS;
    let entry = crate::isolate_cache::Expiring::new(blockhash.value.clone(), expires_at_ms);
    BLOCKHASH_IN_ISOLATE.with_borrow_mut(|cache| cache.insert(rpc_url.to_string(), entry));
    Ok(blockhash)
}

/// Why one blockhash fetch failed: worth a retry, or final.
enum BlockhashFetchFailure {
    /// Rate limited, a server error, or no response at all.
    Transient(EscrowError),
    /// An answer that asking again will not change.
    Final(EscrowError),
}

/// Fetch the latest blockhash, retrying once after a jittered pause when the
/// first attempt fails transiently (see [`super::rpc_retry`]).
async fn fetch_blockhash_with_retry(rpc_url: &str) -> Result<RecentBlockhash, EscrowError> {
    let first_error = match fetch_blockhash_from_rpc(rpc_url).await {
        Ok(blockhash) => return Ok(blockhash),
        Err(BlockhashFetchFailure::Final(e)) => return Err(e),
        Err(BlockhashFetchFailure::Transient(e)) => e,
    };
    let delay_ms = super::rpc_retry::retry_delay_ms(js_sys::Date::now());
    tracing::warn!(error = %first_error, delay_ms, "getLatestBlockhash failed, retrying once");
    worker::Delay::from(std::time::Duration::from_millis(delay_ms)).await;
    match fetch_blockhash_from_rpc(rpc_url).await {
        Ok(blockhash) => Ok(blockhash),
        Err(BlockhashFetchFailure::Transient(e) | BlockhashFetchFailure::Final(e)) => Err(e),
    }
}

/// One `getLatestBlockhash` call to the Solana JSON-RPC endpoint.
async fn fetch_blockhash_from_rpc(rpc_url: &str) -> Result<RecentBlockhash, BlockhashFetchFailure> {
    let fail = |msg: String| BlockhashFetchFailure::Final(EscrowError::RpcFailed(msg));
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-deposit",
        "method": "getLatestBlockhash",
        "params": [{ "commitment": "finalized" }]
    });

    let json_body = serde_json::to_string(&body).map_err(|e| fail(format!("serialize: {e}")))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| fail(format!("headers: {e:?}")))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| fail(format!("request: {e:?}")))?;

    let mut response = worker::Fetch::Request(request).send().await.map_err(|e| {
        BlockhashFetchFailure::Transient(EscrowError::RpcFailed(format!("fetch: {e:?}")))
    })?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        let error = EscrowError::RpcFailed(format!("HTTP {status}: {text}"));
        return Err(match super::rpc_retry::is_transient_status(status) {
            true => BlockhashFetchFailure::Transient(error),
            false => BlockhashFetchFailure::Final(error),
        });
    }

    let text = response
        .text()
        .await
        .map_err(|e| fail(format!("read body: {e:?}")))?;

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| fail(format!("parse json: {e}")))?;

    // An RPC error (HTTP 200) keeps the provider's message instead of reading
    // as "no blockhash".
    let result = super::json_rpc::rpc_result(&json).map_err(fail)?;
    let blockhash = result["value"]["blockhash"]
        .as_str()
        .ok_or_else(|| fail("no blockhash in response".to_string()))?;

    Ok(RecentBlockhash {
        value: blockhash.to_string(),
    })
}
