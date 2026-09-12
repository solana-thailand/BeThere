#!/usr/bin/env bash
# ============================================================================
# BeThere E2E — shared Solana helpers
# ============================================================================
# Issue 076. Requires `lib/common.sh` to have been sourced first (uses `pass`,
# `fail` and `skip`).
#
#     SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
#     # shellcheck source=lib/common.sh
#     source "$SCRIPT_DIR/lib/common.sh"
#     # shellcheck source=lib/solana.sh
#     source "$SCRIPT_DIR/lib/solana.sh"
# ============================================================================

# Directory holding the Python signer, resolved from this file rather than from
# `$0`, so the helper keeps working if a caller is invoked through a symlink or
# from another directory.
E2E_LIB_PARENT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Assert that an escrow address handed to us by the Worker is a real account
# owned by the escrow program.
#
# The address arrives in an API response, so nothing downstream proves it is a
# genuine EventEscrow PDA. Every later check reads it indirectly — vault
# balances go through `spl-token balance --owner "$ADDR"`, which reports "0" for
# a wrong address rather than an error, so a bad address would surface as a
# plausible-looking balance instead of a failure. `test_escrow_devnet.sh` has
# this check; the rollover scripts did not (.issues/072).
#
# Reads `$ESCROW_PROGRAM`, `$RPC_URL` and `${SKIP_SETUP:-false}` from the caller.
assert_escrow_owned_by_program() {
    local label="$1" addr="$2" account_info owner
    case "$addr" in
        "")
            case "${SKIP_SETUP:-false}" in
                true) skip "$label escrow ownership — address not captured in --skip-setup mode" ;;
                *) fail "$label escrow address is empty — nothing to verify" ;;
            esac
            return
            ;;
    esac
    # `$RPC_URL` is the caller's, not the `rpc_url` local of the function
    # below, which ShellCheck offers as a correction.
    # shellcheck disable=SC2153
    account_info=$(solana account "$addr" --url "$RPC_URL" 2>&1 || echo "NOT_FOUND")
    # `solana account` prints capitalized, line-anchored field names
    # (`Owner:`, `Length:` — verified against solana-cli 3.1.10). Matched
    # case-insensitively and anchored: unanchored would also hit the hexdump,
    # and a case-sensitive lowercase pattern never matches at all, which is how
    # the equivalent check in `test_escrow_devnet.sh` silently never passed.
    if ! echo "$account_info" | grep -qi "^length:"; then
        fail "$label escrow PDA not found on-chain: $addr"
        return
    fi
    owner=$(echo "$account_info" | grep -i "^owner:" | awk '{print $2}' || echo "?")
    case "$owner" in
        "$ESCROW_PROGRAM") pass "$label escrow owned by the escrow program ($addr)" ;;
        *) fail "$label escrow $addr owned by $owner, expected $ESCROW_PROGRAM" ;;
    esac
}

# Sign a base64 transaction with `$keypair_path` and submit it to `$rpc_url`
# (default `$RPC_URL`).
sign_and_submit_tx() {
    local tx_b64="$1"
    local keypair_path="$2"
    local rpc_url="${3:-$RPC_URL}"
    # The signer travels as a path and the RPC URL via the environment, so
    # neither lands in argv, where `ps` exposes it to any local user
    # (.issues/073). sign_and_submit.py reads the file itself.
    SIGNER_KEYPAIR_PATH="$keypair_path" \
    SOLANA_RPC_URL="$rpc_url" \
        python3 "$E2E_LIB_PARENT/sign_and_submit.py" "$tx_b64"
}
