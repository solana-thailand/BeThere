//! The agent-facing tools. Each one is a thin composition of existing worker
//! routes; the escrow program, not this server, decides whether a deposit is
//! valid.

use std::sync::Arc;
use std::time::Duration;

use domain::models::deposit::DepositStatusResponse;
use flow_harness::client::{DepositSignatureRequest, DepositUsdcRequest};
use reqwest::Method;
use serde_json::{json, Value};
use solana_sdk::signer::{keypair::Keypair, Signer};

use crate::api::BeThereApi;
use crate::config::{Config, ENV_AGENT_KEYPAIR};
use crate::error::ToolError;

const CONFIRM_POLLS: usize = 15;
const CONFIRM_INTERVAL: Duration = Duration::from_secs(2);
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;
const USDC_UNITS: f64 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolName {
    FindEvents,
    EventDetails,
    AgentWallet,
    Register,
    TicketStatus,
    DepositTx,
    PayDeposit,
}

impl ToolName {
    pub const ALL: [Self; 7] = [
        Self::FindEvents,
        Self::EventDetails,
        Self::AgentWallet,
        Self::Register,
        Self::TicketStatus,
        Self::DepositTx,
        Self::PayDeposit,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::FindEvents => "find_events",
            Self::EventDetails => "event_details",
            Self::AgentWallet => "agent_wallet",
            Self::Register => "register",
            Self::TicketStatus => "ticket_status",
            Self::DepositTx => "deposit_tx",
            Self::PayDeposit => "pay_deposit",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == name)
    }

    fn description(self) -> &'static str {
        match self {
            Self::FindEvents => "List upcoming public BeThere events (id, slug, name, dates, location, whether a deposit is required). Use event_details for the amount.",
            Self::EventDetails => "Full public details of one event by slug: time, location, capacity, deposit amount (USDC smallest units, 6 decimals) and whether the on-chain escrow is accepting deposits.",
            Self::AgentWallet => "The agent's own Solana devnet wallet: its address and SOL balance. BeThere never sees this key; signing happens inside this MCP server on the agent's machine.",
            Self::Register => "Register a person for an event. Signs in with the agent wallet (Sign-In With Solana). The person must have agreed to the privacy notice (consent_given) and, for deposit events, to the deposit commitment. Returns attendee_id and the next step.",
            Self::TicketStatus => "Ticket and deposit state for an attendee of an event: registration, check-in, and whether the USDC deposit is verified on chain.",
            Self::DepositTx => "Build the unsigned devnet escrow deposit transaction for this attendee, as the wallet would see it. Read-only preview: nothing is signed or sent.",
            Self::PayDeposit => "Pay the event's USDC deposit into the Solana escrow from the agent wallet: builds the transaction, signs it locally, sends it to devnet, and waits until BeThere verifies it on chain. Idempotent: a verified or in-flight deposit is never paid twice. Refuses amounts above the operator's cap.",
        }
    }

    fn input_schema(self) -> Value {
        let attendee_ref = json!({
            "type": "object",
            "properties": {
                "event_id": { "type": "string", "description": "Event id (not the slug)." },
                "attendee_id": { "type": "string", "description": "attendee_id returned by register." }
            },
            "required": ["event_id", "attendee_id"],
            "additionalProperties": false
        });
        match self {
            Self::FindEvents | Self::AgentWallet => {
                json!({ "type": "object", "properties": {}, "additionalProperties": false })
            }
            Self::EventDetails => json!({
                "type": "object",
                "properties": { "slug": { "type": "string" } },
                "required": ["slug"],
                "additionalProperties": false
            }),
            Self::Register => json!({
                "type": "object",
                "properties": {
                    "slug": { "type": "string" },
                    "name": { "type": "string", "maxLength": 100 },
                    "email": { "type": "string", "description": "Where the ticket is filed; required for wallet sign-in." },
                    "consent_given": { "type": "boolean", "description": "The person agreed to the privacy notice. Must be true." },
                    "deposit_agreed": { "type": "boolean", "description": "The person agreed to the deposit commitment (deposit events)." },
                    "participation_type": { "type": "string", "enum": ["In-Person", "Online"] },
                    "contact_channel": { "type": "string", "description": "Telegram, Line, Facebook or X (Twitter), when the event requires contact info." },
                    "contact_handle": { "type": "string" }
                },
                "required": ["slug", "name", "email", "consent_given"],
                "additionalProperties": false
            }),
            Self::TicketStatus | Self::DepositTx | Self::PayDeposit => attendee_ref,
        }
    }

    pub fn definition(self) -> Value {
        json!({
            "name": self.as_str(),
            "description": self.description(),
            "inputSchema": self.input_schema(),
        })
    }
}

pub struct Tools {
    config: Config,
    api: BeThereApi,
    wallet: Option<Arc<Keypair>>,
}

impl Tools {
    pub fn new(config: Config, wallet: Option<Keypair>) -> Result<Self, ToolError> {
        let api = BeThereApi::new(config.api_url.clone())?;
        Ok(Self {
            config,
            api,
            wallet: wallet.map(Arc::new),
        })
    }

    pub async fn call(&self, tool: ToolName, args: &Value) -> Result<Value, ToolError> {
        match tool {
            ToolName::FindEvents => self.api.get("/api/public/events", &[]).await,
            ToolName::EventDetails => {
                let slug = path_segment(args, "slug")?;
                self.api
                    .get(&format!("/api/public/event/{slug}"), &[])
                    .await
            }
            ToolName::AgentWallet => self.agent_wallet().await,
            ToolName::Register => self.register(args).await,
            ToolName::TicketStatus => self.ticket_status(args).await,
            ToolName::DepositTx => {
                let (event_id, attendee_id) = attendee_ref(args)?;
                self.deposit_tx(&event_id, &attendee_id).await
            }
            ToolName::PayDeposit => self.pay_deposit(args).await,
        }
    }

    fn wallet(&self) -> Result<&Keypair, ToolError> {
        self.wallet.as_deref().ok_or_else(|| {
            ToolError::Config(format!(
                "no agent wallet: set {ENV_AGENT_KEYPAIR} to a Solana keypair JSON file"
            ))
        })
    }

    async fn agent_wallet(&self) -> Result<Value, ToolError> {
        let wallet = self.wallet()?;
        let lamports = flow_harness::chain::sol_balance_lamports(
            self.config.rpc_url.as_str(),
            &wallet.pubkey(),
        )
        .await?;
        Ok(json!({
            "address": wallet.pubkey().to_string(),
            "cluster": "devnet",
            "sol": lamports as f64 / LAMPORTS_PER_SOL,
            "max_deposit_usdc": self.config.max_deposit_usdc as f64 / USDC_UNITS,
        }))
    }

    async fn register(&self, args: &Value) -> Result<Value, ToolError> {
        if args.get("consent_given").and_then(Value::as_bool) != Some(true) {
            return Err(ToolError::InvalidArgs(
                "consent_given must be true: the person has to agree to the privacy notice"
                    .to_string(),
            ));
        }
        let mut body = json!({
            "slug": required_str(args, "slug")?,
            "name": required_str(args, "name")?,
            "email": required_str(args, "email")?,
            "consent_given": true,
        });
        for key in [
            "deposit_agreed",
            "participation_type",
            "contact_channel",
            "contact_handle",
        ] {
            if let Some(v) = args.get(key) {
                body[key] = v.clone();
            }
        }
        self.api
            .authed(
                self.wallet()?,
                Method::POST,
                "/api/public/register",
                &[],
                Some(&body),
            )
            .await
    }

    async fn ticket_status(&self, args: &Value) -> Result<Value, ToolError> {
        let (event_id, attendee_id) = attendee_ref(args)?;
        // Always pass event_id: without it the ticket route serves the ACTIVE
        // event, which reads falsely green for any other event.
        let query = [("event_id", event_id.as_str())];
        let ticket = self
            .api
            .get(&format!("/api/public/ticket/{attendee_id}"), &query)
            .await?;
        let deposit = self
            .api
            .get(&format!("/api/deposit/status/{attendee_id}"), &query)
            .await?;
        Ok(json!({
            "ticket": ticket,
            "deposit": deposit,
            "ticket_url": self.ticket_url(&event_id, &attendee_id),
        }))
    }

    async fn deposit_tx(&self, event_id: &str, attendee_id: &str) -> Result<Value, ToolError> {
        let wallet = self.wallet()?;
        let wallet_address = wallet.pubkey().to_string();
        // Records the pending deposit the worker later verifies against.
        let initiate = DepositUsdcRequest {
            event_id: event_id.to_string(),
            attendee_id: attendee_id.to_string(),
            wallet_address: wallet_address.clone(),
        };
        self.api
            .authed(
                wallet,
                Method::POST,
                "/api/deposit/usdc",
                &[],
                Some(&to_json(&initiate)?),
            )
            .await?;
        let tx = self
            .api
            .get(
                "/api/deposit/usdc/tx",
                &[
                    ("event_id", event_id),
                    ("attendee_id", attendee_id),
                    ("wallet", wallet_address.as_str()),
                ],
            )
            .await?;
        Ok(json!({
            "signer": wallet_address,
            "message": tx.get("message").cloned().unwrap_or(Value::Null),
            "transaction_base64": tx.get("transaction").cloned().unwrap_or(Value::Null),
            "signed": false,
        }))
    }

    async fn pay_deposit(&self, args: &Value) -> Result<Value, ToolError> {
        let (event_id, attendee_id) = attendee_ref(args)?;
        let wallet = self.wallet()?;
        let status: DepositStatusResponse = BeThereApi::decode(
            self.api
                .get(
                    &format!("/api/deposit/status/{attendee_id}"),
                    &[("event_id", event_id.as_str())],
                )
                .await?,
        )?;

        if let Some(existing) = status.status.as_ref() {
            let signature = existing.tx_signature.as_deref().filter(|s| !s.is_empty());
            match (existing.verified, signature) {
                (true, _) => {
                    return Ok(self.paid(
                        &event_id,
                        &attendee_id,
                        signature,
                        true,
                        "already verified",
                    ))
                }
                // Sent before but not yet verified: never pay twice, just wait.
                (false, Some(sig)) => {
                    let sig = sig.to_string();
                    return self.await_verified(&event_id, &attendee_id, &sig).await;
                }
                (false, None) => {}
            }
        }
        match (
            status.deposit_enabled,
            status.usdc_deposits_accepted,
            status.deposit_amount_usdc,
        ) {
            (false, _, _) => return Err(refuse("this event takes no deposit")),
            (_, false, _) => {
                return Err(refuse(
                    "the escrow for this event is not accepting USDC deposits",
                ))
            }
            (_, _, 0) => return Err(refuse("the event has no USDC deposit amount")),
            (_, _, amount) if amount > self.config.max_deposit_usdc => {
                return Err(refuse(&format!(
                    "deposit {} USDC is above this agent's cap of {} USDC",
                    amount as f64 / USDC_UNITS,
                    self.config.max_deposit_usdc as f64 / USDC_UNITS
                )))
            }
            _ => {}
        }

        let built = self.deposit_tx(&event_id, &attendee_id).await?;
        let tx_b64 = built["transaction_base64"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::Upstream("worker returned no transaction".to_string()))?;
        let signature =
            flow_harness::chain::submit_signed_by(self.config.rpc_url.as_str(), wallet, tx_b64)
                .await?
                .to_string();

        // Hand the exact signature to the worker instead of waiting for it to
        // rediscover the deposit from RPC history.
        let record = DepositSignatureRequest {
            event_id: event_id.clone(),
            attendee_id: attendee_id.clone(),
            tx_signature: signature.clone(),
        };
        self.api
            .authed(
                wallet,
                Method::POST,
                "/api/deposit/usdc/webhook",
                &[],
                Some(&to_json(&record)?),
            )
            .await?;
        self.await_verified(&event_id, &attendee_id, &signature)
            .await
    }

    async fn await_verified(
        &self,
        event_id: &str,
        attendee_id: &str,
        signature: &str,
    ) -> Result<Value, ToolError> {
        let wallet = self.wallet()?;
        let query = [("event_id", event_id), ("attendee_id", attendee_id)];
        for _ in 0..CONFIRM_POLLS {
            let confirm = self
                .api
                .authed(
                    wallet,
                    Method::GET,
                    "/api/deposit/usdc/confirm",
                    &query,
                    None,
                )
                .await?;
            if confirm.get("confirmed").and_then(Value::as_bool) == Some(true) {
                return Ok(self.paid(event_id, attendee_id, Some(signature), true, "verified"));
            }
            tokio::time::sleep(CONFIRM_INTERVAL).await;
        }
        Ok(self.paid(
            event_id,
            attendee_id,
            Some(signature),
            false,
            "sent and confirmed on devnet; BeThere has not verified it yet. Call ticket_status again shortly.",
        ))
    }

    fn paid(
        &self,
        event_id: &str,
        attendee_id: &str,
        signature: Option<&str>,
        verified: bool,
        note: &str,
    ) -> Value {
        json!({
            "verified": verified,
            "note": note,
            "tx_signature": signature,
            "explorer_url": signature
                .map(|s| format!("https://explorer.solana.com/tx/{s}?cluster=devnet")),
            "ticket_url": self.ticket_url(event_id, attendee_id),
        })
    }

    fn ticket_url(&self, event_id: &str, attendee_id: &str) -> String {
        let mut url = self.api.base().clone();
        url.set_path(&format!("/ticket/{attendee_id}"));
        url.query_pairs_mut().append_pair("event_id", event_id);
        url.to_string()
    }
}

fn refuse(reason: &str) -> ToolError {
    ToolError::InvalidArgs(format!("not paying: {reason}"))
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<Value, ToolError> {
    serde_json::to_value(value).map_err(|e| ToolError::Upstream(e.to_string()))
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::InvalidArgs(format!("'{key}' is required")))
}

/// A value that is interpolated into a URL path: reject anything that could
/// change which route is hit.
pub fn path_segment(args: &Value, key: &str) -> Result<String, ToolError> {
    let value = required_str(args, key)?;
    match value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !value.starts_with('.')
    {
        true => Ok(value.to_string()),
        false => Err(ToolError::InvalidArgs(format!(
            "'{key}' may contain only letters, digits, '-', '_' and '.'"
        ))),
    }
}

fn attendee_ref(args: &Value) -> Result<(String, String), ToolError> {
    Ok((
        path_segment(args, "event_id")?,
        path_segment(args, "attendee_id")?,
    ))
}
