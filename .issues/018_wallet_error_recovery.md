# 018 — Wallet Error Recovery Messages

## Summary

Wallet errors are generic and provide no recovery guidance. Users see "Transaction rejected or failed. Please try again." for every possible wallet error — user rejection, insufficient balance, wrong network, timeout, simulation failure — with no differentiation.

## Problem

### JS interop swallows all error context

The wallet JS bridge (`frontend-leptos/js/solana_wallet.js`) catches structured wallet error objects (Phantom's `{code: 4001, message: "..."}`, program logs, etc.) but returns `null` on ANY failure. The Rust layer receives `Option<String>` = `None` with zero error context.

### No error categorization

All wallet errors map to the same generic message: `"Transaction rejected or failed. Please try again."` — whether the user:
- Rejected the transaction in their wallet
- Is on the wrong network
- Has insufficient SOL/USDC
- Sent a transaction that failed on-chain (program error)
- Hit a timeout

### Server errors displayed raw

Helius RPC errors, on-chain simulation failures, and other server-side errors are passed through as-is to the user toast. `"helius rpc error: ... (code ...)"` means nothing to attendees.

## Proposed Solution

### A. Fix JS interop to return error details

Change wallet JS functions to return structured error objects instead of `null`:

```typescript
// Instead of returning null on error, return:
{ ok: true, value: "..." }
// or
{ ok: false, error: { code: number|null, message: string, logs: string[]|null } }
```

This applies to:
- `connectWallet(name)` — map error codes: 4001 = user rejected, -32603 = internal
- `signAndSendTransaction(name, tx)` — extract program logs, error code, instruction error
- `getWalletCluster(name)` — return cluster name for network mismatch detection

### B. Rust-side error type for wallet errors

```rust
struct WalletError {
    code: Option<i32>,
    message: String,
    logs: Option<Vec<String>>,
}
```

Map `WalletError` to user-friendly messages with actionable guidance.

### C. Error-to-guidance mapping

| Error | User Message | Action |
|-------|-------------|--------|
| User rejected (4001) | "You cancelled the transaction. Tap the deposit button to try again." | Retry CTA |
| Wrong network | "Your wallet is on {network}. Switch to {expected} in your wallet settings and try again." | Network guide link |
| Insufficient SOL (0x1, custom error) | "Not enough SOL for transaction fees. You need at least 0.01 SOL. Top up at [faucet link]" | Faucet link (devnet) / CEX deposit (mainnet) |
| Insufficient USDC | "Not enough USDC. You need {amount} USDC for this deposit." | Shows required amount |
| Program error (sim failed) | "Transaction simulation failed. This may be a temporary issue. Please try again in a few seconds." | Retry CTA |
| RPC timeout | "The network is busy. Your transaction may still be processing. Wait a minute and check your wallet." | Wait guidance |
| Already processed | "This transaction was already submitted. Check your wallet for the confirmed transaction." | Check wallet |
| Unknown | "Something went wrong. Please try again or contact support." | Retry + support link |

### D. Server error translation

Map API errors to user-friendly messages in `api.rs`:
- `"deposit not enabled"` → "Deposits are no longer being accepted for this event."
- `"event has ended"` → "This event has ended. Deposits are no longer accepted."
- `"already has a deposit"` → "You have already made a deposit for this event."
- `"helius rpc error"` → "Network issue. Please try again in a moment."
- Generic 5xx → "Server error. Please try again later."

## Files to Modify

| File | Change |
|------|--------|
| `frontend-leptos/js/solana_wallet.js` | Return structured `{ok, value/error}` instead of `null` on failure |
| `frontend-leptos/src/pages/deposit.rs` | Parse `WalletError`, map to guidance messages with CTAs |
| `frontend-leptos/src/pages/claim.rs` | Same wallet error parsing + guidance |
| `frontend-leptos/src/api.rs` | Add `translate_api_error()` for server error messages |
| `frontend-leptos/src/wallet_error.rs` (new) | `WalletError` struct + `fn user_friendly_message()` |

## Acceptance Criteria

- [ ] Wallet rejection (user clicks "Reject") shows "You cancelled" message (not "rejected or failed")
- [ ] Wrong network shows which network wallet is on + which is expected + how to switch
- [ ] Insufficient balance shows required amount
- [ ] Server errors show human-readable messages (no raw error codes)
- [ ] All error states have a clear "What do I do next?" action (retry button, link, etc.)
- [ ] Claim page wallet connect failure shows inline error (not silent)

## Dependencies

- None — purely frontend JS + Rust changes

## Refs

- `frontend-leptos/js/solana_wallet.js` — current wallet bridge
- `frontend-leptos/src/pages/deposit.rs` — deposit error states
- `frontend-leptos/src/pages/claim.rs` — claim error states
