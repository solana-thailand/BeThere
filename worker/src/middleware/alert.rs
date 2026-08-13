//! Slack alerting middleware — fire-and-forget 5xx alerts.
//!
//! Isolated from the correlation middleware on purpose: it reads the
//! `x-correlation-id` header that correlation adds to the response, so it must be
//! layered OUTSIDE correlation. No-op when `SLACK_WEBHOOK_URL` is unset, and the
//! send is best-effort via `ctx.wait_until` so it never blocks or fails a request.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

/// On a 5xx response, post a best-effort Slack alert with the method, path, and
/// correlation id. Disabled (pass-through) when no webhook is configured.
pub async fn slack_alert_layer(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;

    let code = response.status().as_u16();
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
async fn post_slack(webhook_url: &str, text: &str) -> Result<(), String> {
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
