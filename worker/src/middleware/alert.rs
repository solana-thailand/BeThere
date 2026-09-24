//! Slack alerting middleware — fire-and-forget 5xx and security-spike alerts.
//!
//! Isolated from the correlation middleware on purpose: it reads the
//! `x-correlation-id` header that correlation adds to the response, so it must be
//! layered OUTSIDE correlation. No-op when `SLACK_WEBHOOK_URL` is unset, and the
//! send is best-effort via `ctx.wait_until` so it never blocks or fails a request.
//!
//! Security spikes (bursts of 401s with credentials, or of 429s) are detected
//! by `crate::spike`, per isolate; see that module for what counts and why.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use std::sync::{LazyLock, Mutex};

use crate::spike::{SecuritySignal, SpikeCounter, classify};
use crate::state::AppState;

/// Per-isolate spike counters, one per [`SecuritySignal`].
static SPIKES: LazyLock<Mutex<[SpikeCounter; 2]>> = LazyLock::new(|| {
    Mutex::new(SecuritySignal::ALL.map(|signal| SpikeCounter::new(signal.rule())))
});

/// Record a signal; `Some(count)` when it just crossed its spike threshold.
fn record_spike(signal: SecuritySignal) -> Option<u32> {
    let now_ms = u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0);
    let mut counters = SPIKES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    counters[signal.index()].record(now_ms)
}

/// On a 5xx response, post a best-effort Slack alert with the method, path, and
/// correlation id. Disabled (pass-through) when no webhook is configured.
pub async fn slack_alert_layer(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    let had_credentials = crate::auth::extract_token_from_headers(req.headers()).is_some();

    let response = next.run(req).await;

    let code = response.status().as_u16();
    if !state.config.slack_webhook_url.is_empty()
        && let Some(signal) = classify(code, had_credentials, &path)
        && let Some(count) = record_spike(signal)
    {
        let rule = signal.rule();
        tracing::warn!(
            signal = signal.label(),
            count,
            window_secs = rule.window_ms / 1000,
            "security spike detected"
        );
        let webhook = state.config.slack_webhook_url.clone();
        // Path only: never the IP, token or query string (PII / secrets).
        let text = format!(
            "⚠️ *BeThere security spike*: {count}× {label} within {window}s on one isolate \
             (edge-wide total is at least this). Latest: `{method} {path}`. \
             Next alert for this signal in ≥{cooldown} min.",
            label = signal.label(),
            window = rule.window_ms / 1000,
            cooldown = rule.cooldown_ms / 60_000,
        );
        if let Some(ctx) = &state.worker_ctx {
            ctx.wait_until(async move {
                if let Err(e) = post_slack(&webhook, &text).await {
                    tracing::warn!(error = %e, "slack spike alert post failed");
                }
            });
        }
    }

    if code >= 500 && !state.config.slack_webhook_url.is_empty() {
        let correlation_id = response
            .headers()
            .get("x-correlation-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-")
            .to_string();
        let webhook = state.config.slack_webhook_url.clone();
        let text = format!(
            "🚨 *BeThere server error* `{code}`\n`{method} {path}`\ncorrelation_id: `{correlation_id}`"
        );
        // Fire-and-forget: detached from the response so a slow/failed Slack call
        // never affects the user. Requires the worker ctx (present during a real
        // request); if absent, we simply skip (the error is still in the logs).
        if let Some(ctx) = &state.worker_ctx {
            ctx.wait_until(async move {
                if let Err(e) = post_slack(&webhook, &text).await {
                    tracing::warn!(error = %e, "slack alert post failed");
                }
            });
        }
    }

    response
}

/// POST a plain-text message to a Slack incoming webhook. Best-effort.
/// `pub(crate)` so the admin `test-alert` endpoint can reuse it to verify wiring.
pub(crate) async fn post_slack(webhook_url: &str, text: &str) -> Result<(), String> {
    use worker::{Fetch, Headers, Method, Request as WReq, RequestInit};

    let body = serde_json::json!({ "text": text }).to_string();
    let headers = Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("header: {e:?}"))?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&body)));
    let request =
        WReq::new_with_init(webhook_url, &init).map_err(|e| format!("build request: {e:?}"))?;
    Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("send: {e:?}"))?;
    Ok(())
}
