//! Staging fixture for the W3 demo: a public, active event with a 1 USDC
//! deposit whose devnet escrow is initialized, so `pay_deposit` has something
//! real to pay into. Same two admin calls the Manage Events escrow panel makes
//! (`/api/escrow/init` then `/api/escrow/confirm-init`), as in
//! `scripts/e2e_devnet_test.sh` step 2–3.
//!
//! It also does the two steps a fresh API-created event needs before an
//! agent can register: activate it (create ignores `status`) and give it one
//! D1 attendee, a checked-in host walk-in. Until an event has a D1 attendee,
//! registration's duplicate check reads Google Sheets, and the placeholder
//! `sheet_id` below is unreachable there.
//!
//! ```sh
//! BETHERE_ORGANIZER_KEYPAIR=~/.config/solana/id.json \
//!   cargo run --example demo_fixture
//! ```
//!
//! By default the event starts in 48 h and lasts 3 h. For a filmed
//! scan-then-refund take, set `BETHERE_DEMO_END_MIN=<n>` (n >= 2): the event
//! ends n minutes from now and starts one minute earlier. Registration closes
//! at the start and `mark_checked_in` needs `clock <= event_end`, so register,
//! pay and scan inside that window; `refund` opens once it ends (.issues/164).
//! Staging only (`dev-token` admin auth); the config guard refuses prod.

use bethere_mcp::api::unwrap_envelope;
use bethere_mcp::config::Config;
use reqwest::Method;
use serde_json::{json, Value};
use solana_sdk::signer::Signer;

const ADMIN_TOKEN: &str = "dev-token";
const DEPOSIT_USDC: u64 = 1_000_000;
const HOUR_MS: i64 = 3_600_000;
const MINUTE_MS: i64 = 60_000;
const END_MIN_VAR: &str = "BETHERE_DEMO_END_MIN";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let organizer_path = std::env::var_os("BETHERE_ORGANIZER_KEYPAIR")
        .ok_or("set BETHERE_ORGANIZER_KEYPAIR to the organizer's devnet keypair")?;
    let organizer = flow_harness::context::load_keypair_file(
        "BETHERE_ORGANIZER_KEYPAIR",
        std::path::Path::new(&organizer_path),
    )?;
    let http = reqwest::Client::new();
    let base = config.api_url.as_str().trim_end_matches('/').to_string();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let stamp = now_ms / 1000;
    let (start_ms, end_ms) = event_window(now_ms)?;
    let event = json!({
        "id": format!("agent-demo-{stamp}"),
        "name": format!("Agent Demo Meetup {stamp}"),
        "description": "Staging fixture for the BeThere MCP agent demo (plan 033 W3).",
        "location": "Bangkok (staging)",
        "organizer_emails": ["ratchapon.poc@gmail.com"],
        "staff_emails": ["ratchapon.poc@gmail.com"],
        "deposit_enabled": true,
        "deposit_amount_usdc": DEPOSIT_USDC,
        "deposit_amount_thb": 0,
        "organizer_wallet": organizer.pubkey().to_string(),
        "event_start_ms": start_ms,
        "event_end_ms": end_ms,
        "refund_deadline_hours": 168,
        // Required by the API; not a real sheet (the post-deploy smoke fixture
        // does the same). Attendee reads are D1-first.
        "sheet_id": "agent-demo-no-sheet",
        "visibility": "public",
    });
    let created = admin_send(&http, Method::POST, &base, "/api/events", &event).await?;
    let event_id = created["id"]
        .as_str()
        .ok_or("no event id in create response")?
        .to_string();
    eprintln!("event created: {event_id}");

    admin_send(
        &http,
        Method::PUT,
        &base,
        &format!("/api/events/{event_id}"),
        &json!({ "status": "active" }),
    )
    .await?;
    eprintln!("event activated");

    admin_send(
        &http,
        Method::POST,
        &base,
        "/api/walkin/register",
        &json!({
            "event_id": event_id,
            "name": "Demo Host",
            "email": format!("host+{stamp}@bethere.invalid"),
            "override_capacity": true,
        }),
    )
    .await?;
    eprintln!("host walk-in seeded (D1 attendee)");

    let init = admin_send(
        &http,
        Method::POST,
        &base,
        "/api/escrow/init",
        &json!({ "event_id": event_id }),
    )
    .await?;
    let tx_b64 = init["transaction"]
        .as_str()
        .ok_or("no transaction from escrow/init")?;
    let sig =
        flow_harness::chain::submit_signed_by(config.rpc_url.as_str(), &organizer, tx_b64).await?;
    eprintln!("escrow init tx: {sig}");

    let confirmed = admin_send(
        &http,
        Method::POST,
        &base,
        "/api/escrow/confirm-init",
        &json!({ "event_id": event_id }),
    )
    .await?;
    println!(
        "{}",
        json!({
            "event_id": event_id,
            "slug": created.get("slug").cloned().unwrap_or(Value::Null),
            "escrow_address": confirmed.get("escrow_address"),
            "escrow_status": confirmed.get("escrow_status"),
            "init_tx": sig.to_string(),
        })
    );
    Ok(())
}

/// `(event_start_ms, event_end_ms)`: 48 h out for 3 h, or a short event
/// ending `BETHERE_DEMO_END_MIN` minutes from now when that is set.
fn event_window(now_ms: i64) -> Result<(i64, i64), Box<dyn std::error::Error>> {
    let Some(raw) = std::env::var_os(END_MIN_VAR) else {
        let start_ms = now_ms + 48 * HOUR_MS;
        return Ok((start_ms, start_ms + 3 * HOUR_MS));
    };
    let minutes = match raw.to_str().map(str::parse::<u32>) {
        Some(Ok(n)) if n >= 2 => i64::from(n),
        _ => return Err(format!("{END_MIN_VAR} must be a whole number of minutes >= 2").into()),
    };
    let end_ms = now_ms + minutes * MINUTE_MS;
    Ok((end_ms - MINUTE_MS, end_ms))
}

async fn admin_send(
    http: &reqwest::Client,
    method: Method,
    base: &str,
    path: &str,
    body: &Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let resp = http
        .request(method, format!("{base}{path}"))
        .bearer_auth(ADMIN_TOKEN)
        .json(body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    match status.is_success() {
        true => Ok(unwrap_envelope(&text)?),
        false => Err(format!("{path} → {status}: {text}").into()),
    }
}
