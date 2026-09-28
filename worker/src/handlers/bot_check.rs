//! `GET /api/public/turnstile/config` — tells the frontend whether to render
//! the Turnstile widget, and with which site key (`.issues/170`).

use axum::{Json, extract::State};
use serde_json::{Value, json};

use crate::state::AppState;
use crate::turnstile::{Gate, gate};

/// Public. The site key is public by design; the secret never leaves the
/// worker. `enabled` mirrors the server-side gate exactly, so the widget shows
/// if and only if the token will be checked.
#[worker::send]
pub async fn turnstile_config(State(state): State<AppState>) -> Json<Value> {
    let config = &state.config;
    let enabled = gate(
        &config.turnstile_site_key,
        &config.turnstile_secret_key,
        config.dev_mode,
    ) == Gate::Enforced;
    Json(json!({
        "enabled": enabled,
        "site_key": if enabled { config.turnstile_site_key.trim() } else { "" },
    }))
}
