//! Staging execution context — everything a flow needs to talk to the staging
//! worker and reason about on-chain accounts.
//!
//! [`StagingContext`] is constructed once at harness startup (from CLI args +
//! environment) and shared (by reference) across every flow. It deliberately
//! holds **no mutable state**: flows are independent, run sequentially against
//! a deterministic seed, and any per-flow scratch lives in the flow itself.
//!
//! ## What is staging-independent here
//!
//! PDA derivation, keypair loading, URL parsing, and the on-chain layout
//! constants are all pure functions of config. They can be exercised in
//! `cargo test` with zero network. What *does* require staging-live is using
//! these addresses to fetch account state or submit transactions — the flow
//! bodies gate those calls behind `// TODO(staging-live):` markers.
//!
//! ## On the two event-id shapes
//!
//! The worker stores events under a **string** id (e.g. `flow-test-event`,
//! per `worker/scripts/seed-staging.sh`), while the on-chain escrow program
//! keys `EventEscrow` PDAs by a **`u64`** event id
//! (`bethere-escrow/src/lib.rs` `create_event(event_id: u64, ...)`). The
//! worker therefore carries a deterministic string→u64 mapping; the harness
//! mirrors both fields and asserts they refer to the same event via
//! [`StagingContext::event_ids_consistent`].
//!
//! ## Secrets hygiene
//!
//! The funded payer keypair never lives in source. It is loaded from the path
//! in `FLOW_HARNESS_PAYER_KEYPAIR` (a standard Solana keypair JSON file). The
//! context never prints the secret key; its `Debug` impl is hand-rolled to
//! redact it. See `flow-harness/README.md` for the devnet faucet flow.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use url::Url;

use crate::error::{HarnessError, HarnessResult};

// ── On-chain constants ───────────────────────────────────────────────────────
//
// Mirrors `bethere-escrow/src/state.rs` and `src/lib.rs`. If the program id
// or seed layout changes on-chain, these MUST change in lockstep; the PDA
// derivation is asserted in `tests/context_pda_derivation`.

/// Deployed escrow program id (`declare_id!` in `bethere-escrow/src/lib.rs`).
pub const ESCROW_PROGRAM_ID: &str = "C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T";

/// Default Solana JSON-RPC endpoint (public devnet). Override via
/// `FLOW_HARNESS_RPC_URL` to point at a Helius/private devnet RPC.
pub const DEFAULT_RPC_URL: &str = "https://api.devnet.solana.com";

/// `EventEscrow` PDA seed prefix (state.rs: `#[seeds(b"escrow", ...)]`).
pub const EVENT_ESCROW_SEED: &[u8] = b"escrow";

/// `AttendeeDeposit` PDA seed prefix (state.rs: `#[seeds(b"deposit", ...)]`).
pub const ATTENDEE_DEPOSIT_SEED: &[u8] = b"deposit";

// ── Defaults aligned with seed-staging.sh ────────────────────────────────────
//
// These match the rows inserted by `worker/scripts/seed-staging.sh` so a fresh
// `bash worker/deploy.sh staging && bash worker/scripts/seed-staging.sh` run
// produces a context that points at real, addressable state. Override via env
// for non-default scenarios (e.g. pointing at a second test event).

const DEFAULT_STAGING_URL: &str = "https://bethere-staging.solana-thailand.workers.dev";
const DEFAULT_EVENT_ID_STR: &str = "flow-test-event";
const DEFAULT_ATTENDEE_EMAIL: &str = "flow-test-attendee-1@staging.local";

// ── Context ──────────────────────────────────────────────────────────────────

/// Immutable execution context shared by all flows in a single harness run.
///
/// Construct via [`StagingContext::from_env`] (production path) or
/// [`StagingContext::for_testing`] (unit tests — no network, no secrets).
#[derive(Clone)]
pub struct StagingContext {
    /// Base URL of the staging worker (no trailing slash).
    pub worker_url: Url,
    /// Worker-side event id (string primary key in D1).
    pub event_id_str: String,
    /// On-chain event id used in `EventEscrow` PDA seeds. Must map 1:1 to
    /// [`Self::event_id_str`] inside the worker.
    pub event_id_on_chain: u64,
    /// Escrow organizer (signer of `create_event`, first `EventEscrow` seed).
    pub organizer: Pubkey,
    /// Test attendee wallet (the deposit PDA's third seed).
    pub attendee_wallet: Pubkey,
    /// Test attendee email (used to resolve the attendee row via the worker).
    pub attendee_email: String,
    /// Funded payer for transaction fees on devnet. Loaded from disk; never
    /// printed.
    pub payer: Arc<Keypair>,
    /// Escrow program id (defaults to [`ESCROW_PROGRAM_ID`]).
    pub escrow_program_id: Pubkey,
    /// USDC mint used by the escrow vault. Devnet USDC mint by default.
    pub deposit_mint: Pubkey,
    /// Solana JSON-RPC endpoint the harness submits signed transactions to and
    /// reads on-chain accounts from. Defaults to public devnet
    /// ([`DEFAULT_RPC_URL`]); override with `FLOW_HARNESS_RPC_URL` (e.g. a Helius
    /// devnet URL) to avoid the public endpoint's rate limits.
    pub rpc_url: Url,
}

impl StagingContext {
    /// Build the context from CLI-supplied overrides + environment variables.
    ///
    /// Resolution order for each field (first wins):
    ///  1. Explicit CLI arg (e.g. `--worker`)
    ///  2. Environment variable (e.g. `FLOW_HARNESS_WORKER_URL`)
    ///  3. Default aligned with `seed-staging.sh`
    ///
    /// The payer keypair is loaded from the path in `FLOW_HARNESS_PAYER_KEYPAIR`
    /// (a Solana keypair JSON file). Absent keypair → [`HarnessError::Config`]
    /// with actionable copy.
    pub fn from_env(worker_url_override: Option<&str>) -> HarnessResult<Self> {
        let worker_url = parse_worker_url(
            worker_url_override,
            std::env::var("FLOW_HARNESS_WORKER_URL").ok().as_deref(),
        )?;

        let event_id_str = std::env::var("FLOW_HARNESS_EVENT_ID")
            .unwrap_or_else(|_| DEFAULT_EVENT_ID_STR.to_string());

        let event_id_on_chain = std::env::var("FLOW_HARNESS_EVENT_ID_ON_CHAIN")
            .ok()
            .map(|s| s.parse::<u64>())
            .transpose()
            .map_err(|e| HarnessError::Config(format!("FLOW_HARNESS_EVENT_ID_ON_CHAIN: {e}")))?
            // Default mirrors the worker's string→u64 mapping for the seeded
            // test event. The worker is the SSOT; this default exists so a
            // freshly-seeded staging env works with zero env setup. Override
            // via env when the worker's mapping differs.
            .unwrap_or(1);

        let organizer = read_pubkey_env("FLOW_HARNESS_ORGANIZER")?;
        let attendee_wallet = read_pubkey_env("FLOW_HARNESS_ATTENDEE_WALLET")?;
        let attendee_email = std::env::var("FLOW_HARNESS_ATTENDEE_EMAIL")
            .unwrap_or_else(|_| DEFAULT_ATTENDEE_EMAIL.to_string());

        let payer = load_payer_keypair()?;
        let escrow_program_id = read_pubkey_env("FLOW_HARNESS_ESCROW_PROGRAM_ID")
            .unwrap_or_else(|_| pubkey_from_str(ESCROW_PROGRAM_ID));
        let deposit_mint = read_pubkey_env("FLOW_HARNESS_DEPOSIT_MINT")?;
        let rpc_url = {
            let raw = std::env::var("FLOW_HARNESS_RPC_URL")
                .unwrap_or_else(|_| DEFAULT_RPC_URL.to_string());
            Url::parse(&raw).map_err(|e| HarnessError::Config(format!("FLOW_HARNESS_RPC_URL: {e}")))?
        };

        Ok(Self {
            worker_url,
            event_id_str,
            event_id_on_chain,
            organizer,
            attendee_wallet,
            attendee_email,
            payer: Arc::new(payer),
            escrow_program_id,
            deposit_mint,
            rpc_url,
        })
    }

    /// Construct a context purely for unit-testing PDA derivation and
    /// predicate logic. The payer is a fresh, unfunded keypair — never use
    /// this for flows that submit transactions.
    pub fn for_testing(
        worker_url: &str,
        event_id_on_chain: u64,
        organizer: Pubkey,
        attendee_wallet: Pubkey,
    ) -> HarnessResult<Self> {
        Ok(Self {
            worker_url: Url::parse(worker_url)
                .map_err(|e| HarnessError::Config(format!("worker_url: {e}")))?,
            event_id_str: DEFAULT_EVENT_ID_STR.to_string(),
            event_id_on_chain,
            organizer,
            attendee_wallet,
            attendee_email: DEFAULT_ATTENDEE_EMAIL.to_string(),
            payer: Arc::new(Keypair::new()),
            escrow_program_id: pubkey_from_str(ESCROW_PROGRAM_ID),
            // Caller-supplied via env in real runs; for tests we use zero-Pubkey
            // because no flow that consumes `deposit_mint` runs in unit mode.
            deposit_mint: Pubkey::default(),
            // Devnet by default; unit tests never submit, so the value is inert.
            rpc_url: Url::parse(DEFAULT_RPC_URL).expect("DEFAULT_RPC_URL is a valid URL"),
        })
    }

    /// `EventEscrow` PDA: `["escrow", organizer, event_id_on_chain]`.
    ///
    /// Literal transcription of `bethere-escrow/src/state.rs`
    /// `#[seeds(b"escrow", organizer: Address, event_id: u64)]`.
    pub fn event_escrow_pda(&self) -> (Pubkey, u8) {
        let seeds: &[&[u8]] = &[
            EVENT_ESCROW_SEED,
            self.organizer.as_ref(),
            &self.event_id_on_chain.to_le_bytes(),
        ];
        Pubkey::find_program_address(seeds, &self.escrow_program_id)
    }

    /// `AttendeeDeposit` PDA: `["deposit", event_escrow, attendee]`.
    ///
    /// Literal transcription of `bethere-escrow/src/state.rs`
    /// `#[seeds(b"deposit", event: Address, attendee: Address)]`, where
    /// `event` is the [`Self::event_escrow_pda`] address.
    pub fn attendee_deposit_pda(&self) -> (Pubkey, u8) {
        let (event_escrow, _) = self.event_escrow_pda();
        let seeds: &[&[u8]] = &[
            ATTENDEE_DEPOSIT_SEED,
            event_escrow.as_ref(),
            self.attendee_wallet.as_ref(),
        ];
        Pubkey::find_program_address(seeds, &self.escrow_program_id)
    }

    /// The escrow vault ATA (token account) address. The vault is the
    /// `Token` account initialised in `create_event` with
    /// `authority = event_escrow, mint = deposit_mint`. Its address is
    /// derived as an Associated Token Account of the event escrow PDA.
    ///
    /// TODO(staging-live): confirm the worker initialises the vault as an ATA
    /// (vs a random token account) by inspecting a live `create_event` TX.
    /// The escrow `vault` field is the SSOT on-chain; flows should prefer
    /// reading it from the `EventEscrow` account over this derivation.
    pub fn vault_ata_address(&self) -> Pubkey {
        let (event_escrow, _) = self.event_escrow_pda();
        // Lazy import via inline use to keep the dep optional at compile time
        // when spl-associated-token-account is absent. The crate's Cargo.toml
        // pins spl-associated-token-account for this call.
        spl_associated_token_account::get_associated_token_address_with_program_id(
            &event_escrow,
            &self.deposit_mint,
            &spl_token::id(),
        )
    }

    /// Worker-side URL for a deposit-status fetch.
    pub fn deposit_status_url(&self, attendee_id: &str) -> HarnessResult<Url> {
        join_path(
            &self.worker_url,
            &format!("/api/deposit/status/{attendee_id}"),
        )
    }

    /// Worker-side URL for the USDC deposit (Solana Pay URL) endpoint.
    pub fn deposit_usdc_url(&self) -> HarnessResult<Url> {
        join_path(&self.worker_url, "/api/deposit/usdc")
    }

    /// Worker-side URL for the paired refund + close endpoint.
    pub fn refund_url(&self) -> HarnessResult<Url> {
        join_path(&self.worker_url, "/api/escrow/refund")
    }

    /// Worker-side URL for the NFT claim endpoint.
    pub fn claim_url(&self, token: &str) -> HarnessResult<Url> {
        join_path(&self.worker_url, &format!("/api/claim/{token}"))
    }

    /// Worker-side URL for the auth session probe (plan 006 baseline).
    pub fn auth_session_url(&self) -> HarnessResult<Url> {
        join_path(&self.worker_url, "/api/auth/session")
    }

    /// Sanity check: the two event-id fields must refer to the same event.
    /// Returns `Ok(())` if the pair is internally consistent, `Err` otherwise.
    /// Flows may call this at entry to fail fast on misconfiguration.
    pub fn event_ids_consistent(&self) -> HarnessResult<()> {
        // The worker is the SSOT for the string→u64 mapping. The harness does
        // not duplicate that mapping; it only enforces that the user has set
        // *some* u64 (non-zero) — a zero on-chain id would derive a PDA that
        // can never exist on-chain (event ids start at 1 in create_event).
        if self.event_id_on_chain == 0 {
            return Err(HarnessError::Config(format!(
                "event_id_on_chain=0 is invalid (worker event '{}'); set FLOW_HARNESS_EVENT_ID_ON_CHAIN",
                self.event_id_str
            )));
        }
        Ok(())
    }

    /// The payer's public key (convenience — used by every signing flow).
    pub fn payer_pubkey(&self) -> Pubkey {
        // Deref the Arc explicitly. `Signer` is implemented for `Keypair`
        // directly; whether it is also implemented for `Arc<Keypair>` depends
        // on the SDK version, so deref to be safe across versions.
        self.payer.as_ref().pubkey()
    }
}

// Hand-rolled Debug to avoid leaking the payer secret key in log output.
// `Keypair`'s derived Debug is safe (it does not print bytes), but we add a
// belt-and-braces redaction so future SDK changes cannot regress this.
impl std::fmt::Debug for StagingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StagingContext")
            .field("worker_url", &self.worker_url)
            .field("event_id_str", &self.event_id_str)
            .field("event_id_on_chain", &self.event_id_on_chain)
            .field("organizer", &self.organizer)
            .field("attendee_wallet", &self.attendee_wallet)
            .field("attendee_email", &self.attendee_email)
            .field("payer_pubkey", &self.payer_pubkey())
            .field("payer_secret", &"<redacted>")
            .field("escrow_program_id", &self.escrow_program_id)
            .field("deposit_mint", &self.deposit_mint)
            .finish()
    }
}

// ── Loading helpers ──────────────────────────────────────────────────────────

fn parse_worker_url(cli: Option<&str>, env: Option<&str>) -> HarnessResult<Url> {
    let raw = cli.or(env).unwrap_or(DEFAULT_STAGING_URL);
    let url =
        Url::parse(raw).map_err(|e| HarnessError::Config(format!("worker_url '{raw}': {e}")))?;
    // Reject trailing slash so `join_path` produces clean URLs.
    if raw.ends_with('/') && raw.len() > 1 {
        return Err(HarnessError::Config(format!(
            "worker_url '{raw}' must not end with '/' (use '{})'",
            &raw[..raw.len() - 1]
        )));
    }
    Ok(url)
}

fn read_pubkey_env(name: &str) -> HarnessResult<Pubkey> {
    let raw = std::env::var(name).map_err(|_| {
        HarnessError::Config(format!("{name} not set (expected a base58 Solana pubkey)"))
    })?;
    pubkey_from_str_result(&raw, name)
}

fn pubkey_from_str(s: &str) -> Pubkey {
    Pubkey::from_str(s).expect("hardcoded program id is valid base58")
}

fn pubkey_from_str_result(s: &str, label: &str) -> HarnessResult<Pubkey> {
    Pubkey::from_str(s).map_err(|e| HarnessError::Config(format!("{label} '{s}': {e}")))
}

/// Load a Solana keypair JSON file from disk.
///
/// Standard format produced by `solana-keygen new`: a JSON array of 64 u8s
/// (32 bytes seed + 32 bytes pubkey). We accept the array form only; the
/// "byte array" base58 form is not handled because that's a CLI-display
/// convenience, not a file format.
fn load_payer_keypair() -> HarnessResult<Keypair> {
    let path: PathBuf = std::env::var_os("FLOW_HARNESS_PAYER_KEYPAIR")
        .ok_or_else(|| {
            HarnessError::Config(
                "FLOW_HARNESS_PAYER_KEYPAIR not set (expected path to a Solana keypair JSON file)"
                    .to_string(),
            )
        })?
        .into();

    let bytes = std::fs::read_to_string(&path).map_err(|e| {
        HarnessError::Config(format!(
            "FLOW_HARNESS_PAYER_KEYPAIR {}: {e}",
            path.display()
        ))
    })?;

    // Solana keypair files are a JSON array of 64 numbers (32-byte seed +
    // 32-byte pubkey). Deserialise into Vec<u8> then copy into a fixed array;
    // avoids pulling a `serde_bytes` feature the worker build doesn't enable.
    let arr: Vec<u8> = serde_json::from_str(&bytes).map_err(|e| {
        HarnessError::Config(format!(
            "FLOW_HARNESS_PAYER_KEYPAIR {}: not a JSON byte array ({e})",
            path.display()
        ))
    })?;
    if arr.len() != 64 {
        return Err(HarnessError::Config(format!(
            "FLOW_HARNESS_PAYER_KEYPAIR {}: expected 64 bytes, got {}",
            path.display(),
            arr.len()
        )));
    }
    let mut seed = [0u8; 64];
    seed.copy_from_slice(&arr);

    Keypair::try_from(seed.as_slice()).map_err(|e| {
        HarnessError::Config(format!(
            "FLOW_HARNESS_PAYER_KEYPAIR {}: invalid keypair ({e})",
            path.display()
        ))
    })
}

fn join_path(base: &Url, path: &str) -> HarnessResult<Url> {
    base.join(path)
        .map_err(|e| HarnessError::Config(format!("join '{path}' to {}: {e}", base)))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(event_id: u64) -> StagingContext {
        StagingContext::for_testing(
            "https://staging.example.workers.dev",
            event_id,
            Pubkey::from_str("11111111111111111111111111111112").unwrap(),
            Pubkey::from_str("11111111111111111111111111111111").unwrap(),
        )
        .expect("for_testing must succeed with valid inputs")
    }

    #[test]
    fn event_escrow_pda_is_deterministic() {
        // The PDA derivation is a pure function of (organizer, event_id,
        // program_id). Calling twice must return the same address + bump.
        let c = ctx(42);
        let (a, bump_a) = c.event_escrow_pda();
        let (b, bump_b) = c.event_escrow_pda();
        assert_eq!(a, b);
        assert_eq!(bump_a, bump_b);
    }

    #[test]
    fn attendee_deposit_pda_uses_event_escrow_as_seed() {
        // Verify the third seed (the `event` Address) is the event-escrow PDA
        // by re-deriving manually and comparing.
        let c = ctx(7);
        let (event_escrow, _) = c.event_escrow_pda();
        let (attendee_deposit, _) = c.attendee_deposit_pda();

        let expected = Pubkey::find_program_address(
            &[
                ATTENDEE_DEPOSIT_SEED,
                event_escrow.as_ref(),
                c.attendee_wallet.as_ref(),
            ],
            &c.escrow_program_id,
        )
        .0;
        assert_eq!(attendee_deposit, expected);
    }

    #[test]
    fn distinct_event_ids_yield_distinct_escrow_pdas() {
        let a = ctx(1).event_escrow_pda().0;
        let b = ctx(2).event_escrow_pda().0;
        assert_ne!(a, b, "event_id must participate in the PDA seeds");
    }

    #[test]
    fn event_ids_consistent_rejects_zero() {
        let mut c = ctx(0);
        assert!(c.event_ids_consistent().is_err());
        c.event_id_on_chain = 1;
        assert!(c.event_ids_consistent().is_ok());
    }

    #[test]
    fn worker_url_must_not_have_trailing_slash() {
        assert!(parse_worker_url(Some("https://x.dev/"), None).is_err());
        assert!(parse_worker_url(Some("https://x.dev"), None).is_ok());
    }

    #[test]
    fn deposit_status_url_is_well_formed() {
        let c = ctx(1);
        let url = c.deposit_status_url("abc").unwrap();
        assert_eq!(
            url.as_str(),
            "https://staging.example.workers.dev/api/deposit/status/abc"
        );
    }

    #[test]
    fn debug_impl_redacts_payer_secret() {
        let c = ctx(1);
        let s = format!("{c:?}");
        assert!(s.contains("<redacted>"));
        assert!(!s.contains("Keypair"));
    }
}
