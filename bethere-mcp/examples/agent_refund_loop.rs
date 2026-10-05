//! The whole agent loop on devnet, as one run: the agent books and pays, the
//! organizer scans, the event ends, the agent claims the deposit back.
//!
//! Run `demo_fixture` first with a short event, then pass its event id:
//!
//! ```sh
//! BETHERE_ORGANIZER_KEYPAIR=~/.config/solana/id.json BETHERE_DEMO_END_MIN=6 \
//!   cargo run --example demo_fixture            # prints {"event_id": …, "slug": …}
//! BETHERE_ORGANIZER_KEYPAIR=~/.config/solana/id.json \
//! BETHERE_AGENT_KEYPAIR=~/.config/solana/bethere-agent-demo.json \
//!   cargo run --example agent_refund_loop -- <event_id> <slug>
//! ```
//!
//! Every agent step goes through the MCP tools (`register`, `pay_deposit`,
//! `claim_refund`); only the scan is the organizer's, with the same two calls
//! the scanner makes (`/api/checkin/{id}`, then `mark_checked_in` signed by the
//! organizer). Staging only (`dev-token` admin auth). Prints the three
//! transactions with Explorer links.

use bethere_mcp::api::unwrap_envelope;
use bethere_mcp::config::Config;
use bethere_mcp::tools::{ToolName, Tools};
use reqwest::Method;
use serde_json::{json, Value};
use solana_sdk::signer::Signer;

const ADMIN_TOKEN: &str = "dev-token";
/// Margin past the event end for validator clock drift before claiming.
const END_MARGIN_MS: i64 = 20_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (event_id, slug) = match (args.next(), args.next()) {
        (Some(id), Some(slug)) => (id, slug),
        _ => return Err("usage: agent_refund_loop <event_id> <slug>".into()),
    };
    let config = Config::from_env()?;
    let organizer_path = std::env::var_os("BETHERE_ORGANIZER_KEYPAIR")
        .ok_or("set BETHERE_ORGANIZER_KEYPAIR to the organizer's devnet keypair")?;
    let organizer = flow_harness::context::load_keypair_file(
        "BETHERE_ORGANIZER_KEYPAIR",
        std::path::Path::new(&organizer_path),
    )?;
    let agent_path = std::env::var_os("BETHERE_AGENT_KEYPAIR")
        .ok_or("set BETHERE_AGENT_KEYPAIR to the agent's devnet keypair")?;
    let agent = flow_harness::context::load_keypair_file(
        "BETHERE_AGENT_KEYPAIR",
        std::path::Path::new(&agent_path),
    )?;
    let agent_address = agent.pubkey().to_string();
    let http = reqwest::Client::new();
    let base = config.api_url.as_str().trim_end_matches('/').to_string();
    let rpc = config.rpc_url.as_str().to_string();
    let tools = Tools::new(config, Some(agent))?;

    // 1. The agent books and pays.
    let stamp = now_ms() / 1000;
    let registered = tools
        .call(
            ToolName::Register,
            &json!({
                "slug": slug,
                "name": "Agent Loop Tester",
                "email": format!("agent-loop+{stamp}@bethere.invalid"),
                "consent_given": true,
                "deposit_agreed": true,
                "participation_type": "In-Person",
                "contact_channel": "Telegram",
                "contact_handle": "@agent_loop_tester",
            }),
        )
        .await?;
    let attendee_id = registered["attendee_id"]
        .as_str()
        .ok_or("register returned no attendee_id")?
        .to_string();
    eprintln!("registered: {attendee_id}");
    let attendee = json!({ "event_id": event_id, "attendee_id": attendee_id });
    let paid = tools.call(ToolName::PayDeposit, &attendee).await?;
    let deposit_sig = paid["tx_signature"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    eprintln!("deposit: {deposit_sig} verified={}", paid["verified"]);

    // 2. The organizer scans: off-chain check-in, then mark_checked_in on chain.
    admin_send(
        &http,
        Method::POST,
        &base,
        &format!("/api/checkin/{attendee_id}?event_id={event_id}"),
        &json!({}),
    )
    .await?;
    let mark = admin_send(
        &http,
        Method::POST,
        &base,
        "/api/escrow/mark-checked-in",
        &json!({
            "event_id": event_id,
            "attendee_id": attendee_id,
            "attendee_wallet": agent_address,
        }),
    )
    .await?;
    let mark_tx = mark["transaction"]
        .as_str()
        .ok_or("no transaction from escrow/mark-checked-in")?;
    let checkin_sig = flow_harness::chain::submit_signed_by(&rpc, &organizer, mark_tx)
        .await?
        .to_string();
    eprintln!("checked in on chain: {checkin_sig}");

    // 3. Wait for the end, then the agent claims back.
    let status = tools.call(ToolName::TicketStatus, &attendee).await?;
    let end_ms = status["deposit"]["event_end_ms"].as_i64().unwrap_or(0);
    let wait_ms = (end_ms + END_MARGIN_MS - now_ms()).max(0);
    eprintln!("waiting {}s for the event to end", wait_ms / 1000);
    tokio::time::sleep(std::time::Duration::from_millis(u64::try_from(wait_ms)?)).await;
    let refunded = tools.call(ToolName::ClaimRefund, &attendee).await?;
    let refund_sig = refunded["tx_signature"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    eprintln!("refunded: {refund_sig}");

    let link = |sig: &str| format!("https://explorer.solana.com/tx/{sig}?cluster=devnet");
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "event_id": event_id,
            "attendee_id": attendee_id,
            "agent_wallet": agent_address,
            "deposit": { "tx": deposit_sig, "explorer": link(&deposit_sig) },
            "mark_checked_in": { "tx": checkin_sig, "explorer": link(&checkin_sig) },
            "refund": { "tx": refund_sig, "explorer": link(&refund_sig) },
        }))?
    );
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
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
