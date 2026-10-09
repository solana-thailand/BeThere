//! `/api/sandbox/*`: the public devnet sandbox (`.plans/042` 0.4,
//! `crate::sandbox`). Every route answers 404 unless the sandbox is on.
//!
//! The Worker signs only as the sandbox organizer (create the event, check
//! the visitor in) and as the faucet (the grant). Everything the visitor
//! does — deposit, refund, returning the test USDC — comes back unsigned
//! for the browser burner wallet to sign and send.

use axum::{Json, extract::State};
use base64::Engine;
use event_checkin_domain::models::error::AppError;
use serde::{Deserialize, Serialize};

use crate::error::{ApiOk, WorkerError};
use crate::sandbox::config::{
    BROWSER_RPC, DEPOSIT_AMOUNT, EVENT_SECONDS, EVENTS_PER_DAY, FAUCET_GRANTS_PER_DAY,
    FAUCET_LAMPORTS, FAUCET_USDC, REFUND_WINDOW_SECONDS, SandboxKeys, sandbox,
};
use crate::sandbox::keys::SandboxSigner;
use crate::sandbox::quota::{self, Slot};
use crate::sandbox::send::{send_and_confirm, token_balance};
use crate::sandbox::tx::{faucet_grant_tx, token_return_tx};
use crate::solana_escrow::blockhash::get_latest_blockhash;
use crate::solana_escrow::{
    EscrowError, build_deposit_transaction, build_init_escrow_transaction,
    build_mark_checked_in_transaction, build_refund_and_close_transaction, escrow_program_id,
    get_associated_token_address, pubkey_from_base58, pubkey_to_base58, usdc_mint,
    verify_attendee_deposit_onchain,
};
use crate::state::AppState;

const EVENT_PREFIX: &str = "sandbox:event:";
const FAUCET_PREFIX: &str = "sandbox:faucet:";

#[derive(Serialize)]
pub struct SandboxConfigResponse {
    pub enabled: bool,
    pub organizer: String,
    pub faucet: String,
    pub usdc_mint: String,
    pub program_id: String,
    pub deposit_amount: u64,
    pub event_seconds: i64,
    pub browser_rpc: String,
}

#[derive(Serialize)]
pub struct SandboxEventResponse {
    /// u64 as a string: JavaScript numbers lose precision above 2^53.
    pub event_id: String,
    pub event_end: i64,
    pub signature: String,
}

#[derive(Serialize)]
pub struct SandboxSignatureResponse {
    pub signature: String,
}

#[derive(Serialize)]
pub struct SandboxTxResponse {
    pub transaction_b64: String,
}

#[derive(Deserialize)]
pub struct WalletRequest {
    pub wallet: String,
}

#[derive(Deserialize)]
pub struct EventWalletRequest {
    pub event_id: String,
    pub wallet: String,
}

fn keys() -> Result<&'static SandboxKeys, AppError> {
    sandbox().map_err(|_| AppError::NotFound("the sandbox is not available here".to_string()))
}

fn d1(state: &AppState) -> Result<&worker::D1Database, AppError> {
    state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".to_string()))
}

/// A devnet failure, logged in full, answered with only the step that failed
/// (provider text can carry URLs and keys).
fn chain_error(step: &str, error: EscrowError) -> AppError {
    tracing::warn!(step, error = %error, "sandbox devnet call failed");
    AppError::External {
        service: "solana devnet".to_string(),
        status: 502,
        body: format!("{step} did not go through on devnet; try again"),
    }
}

/// A visitor wallet: valid base58, and not one of the sandbox's own keys.
fn visitor_wallet(keys: &SandboxKeys, wallet: &str) -> Result<[u8; 32], AppError> {
    let wallet = wallet.trim();
    crate::solana::validate_wallet_address(wallet).map_err(AppError::Validation)?;
    let pubkey = pubkey_from_base58(wallet)
        .map_err(|_| AppError::Validation("wallet is not a Solana address".to_string()))?;
    if &pubkey == keys.organizer.pubkey() || &pubkey == keys.faucet.pubkey() {
        return Err(AppError::Validation(
            "use a visitor wallet, not a sandbox key".to_string(),
        ));
    }
    Ok(pubkey)
}

fn event_id(raw: &str) -> Result<u64, AppError> {
    raw.trim()
        .parse::<u64>()
        .map_err(|_| AppError::Validation("event_id is not a sandbox event id".to_string()))
}

/// A fresh on-chain event id: the random low half of a v7 UUID.
fn new_event_id() -> u64 {
    let bytes = uuid::Uuid::now_v7().into_bytes();
    let mut low = [0u8; 8];
    low.copy_from_slice(&bytes[8..]);
    u64::from_le_bytes(low)
}

/// Sign base64 `tx` as `signer`, send it, and wait for `confirmed`.
async fn sign_and_send(
    rpc_url: &str,
    signer: &SandboxSigner,
    tx_b64: &str,
    step: &str,
) -> Result<String, AppError> {
    let mut tx = base64::engine::general_purpose::STANDARD
        .decode(tx_b64)
        .map_err(|e| AppError::Internal(format!("{step}: builder emitted bad base64: {e}")))?;
    signer
        .sign_transaction(&mut tx)
        .map_err(|e| AppError::Internal(format!("{step}: {e}")))?;
    send_and_confirm(rpc_url, &tx)
        .await
        .map_err(|e| chain_error(step, e))
}

fn b64(tx: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(tx)
}

/// `GET /api/sandbox/config`: whether the sandbox is on, and the public
/// numbers and addresses the page shows.
#[worker::send]
pub async fn config() -> Json<SandboxConfigResponse> {
    let (enabled, organizer, faucet) = match sandbox() {
        Ok(keys) => (
            true,
            keys.organizer.address().to_string(),
            keys.faucet.address().to_string(),
        ),
        Err(_) => (false, String::new(), String::new()),
    };
    Json(SandboxConfigResponse {
        enabled,
        organizer,
        faucet,
        usdc_mint: usdc_mint().to_string(),
        program_id: escrow_program_id().to_string(),
        deposit_amount: DEPOSIT_AMOUNT,
        event_seconds: EVENT_SECONDS,
        browser_rpc: BROWSER_RPC.to_string(),
    })
}

/// `POST /api/sandbox/events`: the sandbox organizer creates a two-minute
/// event with a 1 test USDC deposit. Nothing about the visitor is sent.
#[worker::send]
pub async fn create_event(
    State(state): State<AppState>,
) -> Result<ApiOk<SandboxEventResponse>, WorkerError> {
    let keys = keys()?;
    let db = d1(&state)?;
    let id = new_event_id();
    let item = id.to_string();
    match quota::claim(db, EVENT_PREFIX, &item, EVENTS_PER_DAY)
        .await
        .map_err(AppError::Internal)?
    {
        Slot::Granted => {}
        Slot::AlreadyHeld | Slot::CapReached => {
            return Err(AppError::RateLimited(
                "the sandbox has made all its events for today; try again tomorrow".to_string(),
            )
            .into());
        }
    }

    let rpc_url = state.config.solana.full_rpc_url();
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let event_end = now + EVENT_SECONDS;
    let created = async {
        let built = build_init_escrow_transaction(
            &rpc_url,
            keys.organizer.address(),
            id,
            DEPOSIT_AMOUNT,
            event_end,
            event_end + REFUND_WINDOW_SECONDS,
        )
        .await
        .map_err(|e| chain_error("creating the event", e))?;
        sign_and_send(
            &rpc_url,
            &keys.organizer,
            &built.transaction_b64,
            "creating the event",
        )
        .await
    }
    .await;
    match created {
        Ok(signature) => Ok(ApiOk::new(SandboxEventResponse {
            event_id: item,
            event_end,
            signature,
        })),
        Err(error) => {
            quota::release(db, EVENT_PREFIX, &item).await;
            Err(error.into())
        }
    }
}

/// `POST /api/sandbox/faucet`: one grant per wallet per day — 1 test USDC
/// and 0.003 SOL — within the faucet's daily cap.
#[worker::send]
pub async fn faucet(
    State(state): State<AppState>,
    Json(body): Json<WalletRequest>,
) -> Result<ApiOk<SandboxSignatureResponse>, WorkerError> {
    let keys = keys()?;
    let recipient = visitor_wallet(keys, &body.wallet)?;
    let db = d1(&state)?;
    let item = body.wallet.trim().to_string();
    match quota::claim(db, FAUCET_PREFIX, &item, FAUCET_GRANTS_PER_DAY)
        .await
        .map_err(AppError::Internal)?
    {
        Slot::Granted => {}
        Slot::AlreadyHeld => {
            return Err(AppError::Conflict(
                "this wallet already got its test USDC today".to_string(),
            )
            .into());
        }
        Slot::CapReached => {
            return Err(AppError::RateLimited(
                "the faucet has given out today's test USDC; try again tomorrow".to_string(),
            )
            .into());
        }
    }

    let rpc_url = state.config.solana.full_rpc_url();
    let granted = async {
        let mint = pubkey_from_base58(usdc_mint()).map_err(|e| chain_error("the faucet", e))?;
        let faucet_ata = get_associated_token_address(keys.faucet.pubkey(), &mint)
            .await
            .map_err(|e| chain_error("the faucet", e))?;
        let held = token_balance(&rpc_url, &pubkey_to_base58(&faucet_ata))
            .await
            .map_err(|e| chain_error("the faucet", e))?;
        if held < FAUCET_USDC {
            tracing::warn!(held, "sandbox faucet is out of test USDC");
            return Err(AppError::RateLimited(
                "the faucet is out of test USDC right now; try again later".to_string(),
            ));
        }
        let blockhash = get_latest_blockhash(&rpc_url)
            .await
            .and_then(|bh| pubkey_from_base58(&bh.value))
            .map_err(|e| chain_error("the faucet", e))?;
        let tx = faucet_grant_tx(
            keys.faucet.pubkey(),
            &recipient,
            &mint,
            FAUCET_LAMPORTS,
            FAUCET_USDC,
            &blockhash,
        )
        .await
        .map_err(|e| chain_error("the faucet", e))?;
        sign_and_send(&rpc_url, &keys.faucet, &b64(&tx), "the faucet").await
    }
    .await;
    match granted {
        Ok(signature) => Ok(ApiOk::new(SandboxSignatureResponse { signature })),
        Err(error) => {
            quota::release(db, FAUCET_PREFIX, &item).await;
            Err(error.into())
        }
    }
}

/// `POST /api/sandbox/deposit-tx`: the unsigned deposit for the burner.
#[worker::send]
pub async fn deposit_tx(
    State(state): State<AppState>,
    Json(body): Json<EventWalletRequest>,
) -> Result<ApiOk<SandboxTxResponse>, WorkerError> {
    let keys = keys()?;
    visitor_wallet(keys, &body.wallet)?;
    let id = event_id(&body.event_id)?;
    let rpc_url = state.config.solana.full_rpc_url();
    let built = build_deposit_transaction(
        &rpc_url,
        keys.organizer.address(),
        id,
        body.wallet.trim(),
        DEPOSIT_AMOUNT,
    )
    .await
    .map_err(|e| chain_error("building the deposit", e))?;
    Ok(ApiOk::new(SandboxTxResponse {
        transaction_b64: built.transaction_b64,
    }))
}

/// `POST /api/sandbox/check-in`: the sandbox organizer checks the visitor in,
/// once their deposit is on-chain. Only sandbox events can be marked: the
/// escrow address is derived from the sandbox organizer.
#[worker::send]
pub async fn check_in(
    State(state): State<AppState>,
    Json(body): Json<EventWalletRequest>,
) -> Result<ApiOk<SandboxSignatureResponse>, WorkerError> {
    let keys = keys()?;
    visitor_wallet(keys, &body.wallet)?;
    let id = event_id(&body.event_id)?;
    let wallet = body.wallet.trim();
    let rpc_url = state.config.solana.full_rpc_url();
    let deposited = verify_attendee_deposit_onchain(
        &rpc_url,
        keys.organizer.address(),
        id,
        wallet,
        DEPOSIT_AMOUNT,
    )
    .await
    .map_err(|e| chain_error("reading the deposit", e))?;
    if !deposited {
        return Err(AppError::Validation(
            "no deposit from this wallet for this event yet".to_string(),
        )
        .into());
    }
    let built = build_mark_checked_in_transaction(&rpc_url, keys.organizer.address(), id, wallet)
        .await
        .map_err(|e| chain_error("the check-in", e))?;
    let signature = sign_and_send(
        &rpc_url,
        &keys.organizer,
        &built.transaction_b64,
        "the check-in",
    )
    .await?;
    Ok(ApiOk::new(SandboxSignatureResponse { signature }))
}

/// `POST /api/sandbox/refund-tx`: the unsigned refund-and-close for the
/// burner. The program opens it only after the event ends.
#[worker::send]
pub async fn refund_tx(
    State(state): State<AppState>,
    Json(body): Json<EventWalletRequest>,
) -> Result<ApiOk<SandboxTxResponse>, WorkerError> {
    let keys = keys()?;
    visitor_wallet(keys, &body.wallet)?;
    let id = event_id(&body.event_id)?;
    let rpc_url = state.config.solana.full_rpc_url();
    let built = build_refund_and_close_transaction(
        &rpc_url,
        keys.organizer.address(),
        id,
        body.wallet.trim(),
    )
    .await
    .map_err(|e| chain_error("building the refund", e))?;
    Ok(ApiOk::new(SandboxTxResponse {
        transaction_b64: built.transaction_b64,
    }))
}

/// `POST /api/sandbox/return-tx`: the unsigned transfer of the 1 test USDC
/// back to the faucet, so the next visitor can use it.
#[worker::send]
pub async fn return_tx(
    State(state): State<AppState>,
    Json(body): Json<WalletRequest>,
) -> Result<ApiOk<SandboxTxResponse>, WorkerError> {
    let keys = keys()?;
    let owner = visitor_wallet(keys, &body.wallet)?;
    let rpc_url = state.config.solana.full_rpc_url();
    let blockhash = get_latest_blockhash(&rpc_url)
        .await
        .and_then(|bh| pubkey_from_base58(&bh.value))
        .map_err(|e| chain_error("building the return", e))?;
    let mint =
        pubkey_from_base58(usdc_mint()).map_err(|e| chain_error("building the return", e))?;
    let tx = token_return_tx(&owner, keys.faucet.pubkey(), &mint, FAUCET_USDC, &blockhash)
        .await
        .map_err(|e| chain_error("building the return", e))?;
    Ok(ApiOk::new(SandboxTxResponse {
        transaction_b64: b64(&tx),
    }))
}
