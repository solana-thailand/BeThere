//! When the sandbox is on, its fixed numbers, and its two keys.

use std::sync::OnceLock;

use super::keys::SandboxSigner;

/// Deposit of a sandbox event: 1 test USDC.
pub const DEPOSIT_AMOUNT: u64 = 1_000_000;
/// Test USDC per faucet grant: one deposit.
pub const FAUCET_USDC: u64 = DEPOSIT_AMOUNT;
/// SOL per faucet grant. The deposit PDA's rent (96 bytes, 1,559,040 lamports)
/// plus three fees, and the fee payer must stay above the 890,880-lamport rent
/// floor after paying that rent; 0.003 SOL covers both. The refund closes the
/// PDA and pays its rent back.
pub const FAUCET_LAMPORTS: u64 = 3_000_000;
/// A sandbox event ends this long after it is created. Check-in must land
/// before the end and the refund only opens after it (the program's rules).
pub const EVENT_SECONDS: i64 = 120;
/// No-show refunds stay open this long after the end (the 24 h expiry).
pub const REFUND_WINDOW_SECONDS: i64 = 24 * 3600;
/// Faucet grants per rolling 24 h, across all visitors.
pub const FAUCET_GRANTS_PER_DAY: u32 = 50;
/// Events created per rolling 24 h. Each locks about 0.0043 SOL of organizer
/// rent (escrow PDA + vault), so this bounds a day at under 1 SOL.
pub const EVENTS_PER_DAY: u32 = 200;
/// The public devnet RPC the visitor's browser sends its own transactions to.
pub const BROWSER_RPC: &str = "https://api.devnet.solana.com";

/// The organizer that creates and checks in every sandbox event, and the
/// faucet that funds visitors.
#[derive(Debug)]
pub struct SandboxKeys {
    pub organizer: SandboxSigner,
    pub faucet: SandboxSigner,
}

/// Why the sandbox is off; reported by `/api/sandbox/config` without detail
/// that would help anyone but us.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Off {
    /// `SANDBOX_ENABLED` is not "1" on this deploy.
    Disabled,
    /// The escrow cluster is not devnet.
    NotDevnet,
    /// A key secret is unset.
    KeysMissing,
    /// A key secret is set but does not parse.
    KeysInvalid,
}

static SANDBOX: OnceLock<Result<SandboxKeys, Off>> = OnceLock::new();

/// The pure on/off rule, for tests: switched on, devnet, and both secrets parse.
pub fn decide(
    enabled: bool,
    cluster: &str,
    organizer_json: Option<&str>,
    faucet_json: Option<&str>,
) -> Result<SandboxKeys, Off> {
    if !enabled {
        return Err(Off::Disabled);
    }
    if cluster != "devnet" {
        return Err(Off::NotDevnet);
    }
    let (Some(organizer), Some(faucet)) = (organizer_json, faucet_json) else {
        return Err(Off::KeysMissing);
    };
    let organizer = SandboxSigner::from_keypair_json(organizer).map_err(|_| Off::KeysInvalid)?;
    let faucet = SandboxSigner::from_keypair_json(faucet).map_err(|_| Off::KeysInvalid)?;
    if organizer.pubkey() == faucet.pubkey() {
        return Err(Off::KeysInvalid);
    }
    Ok(SandboxKeys { organizer, faucet })
}

/// Seed once per isolate, after the escrow cluster is seeded.
pub(crate) fn seed_from_env(env: &worker::Env) {
    if SANDBOX.get().is_some() {
        return;
    }
    let secret = |name: &str| env.secret(name).ok().map(|s| s.to_string());
    let organizer = secret("SANDBOX_ORGANIZER_KEY");
    let faucet = secret("SANDBOX_FAUCET_KEY");
    let enabled = env
        .var("SANDBOX_ENABLED")
        .is_ok_and(|v| v.to_string() == "1");
    let decided = decide(
        enabled,
        crate::solana_escrow::cluster(),
        organizer.as_deref(),
        faucet.as_deref(),
    );
    if let Err(off) = &decided {
        match off {
            Off::KeysInvalid => tracing::error!(?off, "sandbox keys set but unusable"),
            _ => tracing::debug!(?off, "sandbox off"),
        }
    }
    let _ = SANDBOX.set(decided);
}

/// The keys when the sandbox is on.
pub(crate) fn sandbox() -> Result<&'static SandboxKeys, Off> {
    match SANDBOX.get() {
        Some(Ok(keys)) => Ok(keys),
        Some(Err(off)) => Err(*off),
        None => Err(Off::KeysMissing),
    }
}
