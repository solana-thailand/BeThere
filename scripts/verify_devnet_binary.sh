#!/usr/bin/env bash
# ============================================================================
# BeThere — prove on-chain bytecode == pinned source  (runbook Phase 0.3 gate)
# ============================================================================
# Fetches the deployed program from a Solana cluster and proves, byte-for-byte,
# that it matches the locally built `bethere_escrow.so`. This is the gate that
# must pass before ANY Phase 1+ devnet result is trusted (the deployed binary
# was found to differ from source — see runbook Phase 0).
#
# Why a script (not `sha256sum` by hand): `solana program dump` returns the full
# allocated programdata, ZERO-PADDED up to the deploy's max_len. So the dump is
# almost always LONGER than the local .so even for a perfect match (e.g. the
# 99,104 B on-chain vs 89,856 B local noted in the runbook is padding, not a
# mismatch). A naive length- or whole-file-sha compare FALSELY FAILS. The correct
# check is: the first len(local) bytes must be identical, and every byte after
# must be zero.
#
# This step is READ-ONLY — no keypair required. (Run 0.1/0.2 build+deploy first.)
#
# Usage:
#   scripts/verify_devnet_binary.sh                 # verify against current local .so
#   scripts/verify_devnet_binary.sh --build         # `quasar build` from pinned source first
#   PROGRAM_ID=... CLUSTER_URL=... scripts/verify_devnet_binary.sh
#
# Env overrides:
#   PROGRAM_ID   default: C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T (devnet)
#   CLUSTER_URL  default: devnet   (accepts devnet|mainnet-beta|testnet|<rpc-url>)
#   LOCAL_SO     default: bethere-escrow/target/deploy/bethere_escrow.so
# ============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROGRAM_ID="${PROGRAM_ID:-C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T}"
CLUSTER_URL="${CLUSTER_URL:-devnet}"
LOCAL_SO="${LOCAL_SO:-$REPO_ROOT/bethere-escrow/target/deploy/bethere_escrow.so}"
DO_BUILD=0
[[ "${1:-}" == "--build" ]] && DO_BUILD=1

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
bold()  { printf '\033[1m%s\033[0m\n' "$*"; }

die() { red "✗ $*"; exit 1; }

sha256() { # portable: linux sha256sum / macOS shasum
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
  elif command -v shasum   >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'
  else die "need sha256sum or shasum"; fi
}
filesize() { # portable byte count
  if stat -c%s "$1" >/dev/null 2>&1; then stat -c%s "$1"; else stat -f%z "$1"; fi
}

command -v solana >/dev/null 2>&1 || die "solana CLI not found (install the Agave/Solana tool suite)"

bold "== Phase 0.3 — prove on-chain == source =="
echo "  program id : $PROGRAM_ID"
echo "  cluster    : $CLUSTER_URL"
echo "  local .so  : $LOCAL_SO"
echo

if [[ "$DO_BUILD" == 1 ]]; then
  bold "Building from pinned source (quasar build)…"
  ( cd "$REPO_ROOT/bethere-escrow" && quasar build )
  echo
fi

[[ -f "$LOCAL_SO" ]] || die "local .so not found: $LOCAL_SO  (run with --build, or 'cd bethere-escrow && quasar build')"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
ONCHAIN="$TMP/onchain.so"

bold "Dumping deployed program…"
solana program dump "$PROGRAM_ID" "$ONCHAIN" --url "$CLUSTER_URL" >/dev/null \
  || die "solana program dump failed (is $PROGRAM_ID deployed on $CLUSTER_URL?)"

LOCAL_LEN="$(filesize "$LOCAL_SO")"
ONCHAIN_LEN="$(filesize "$ONCHAIN")"
echo "  local  bytes: $LOCAL_LEN"
echo "  onchain bytes: $ONCHAIN_LEN  (padded up to the deploy's allocated max_len)"
echo

(( ONCHAIN_LEN >= LOCAL_LEN )) \
  || die "on-chain ($ONCHAIN_LEN B) is SHORTER than local ($LOCAL_LEN B) — truncated/older deploy. NOT a match."

# 1) First len(local) bytes must be byte-identical.
head -c "$LOCAL_LEN" "$ONCHAIN" > "$TMP/onchain_head.so"
LOCAL_SHA="$(sha256 "$LOCAL_SO")"
HEAD_SHA="$(sha256 "$TMP/onchain_head.so")"
echo "  local   sha256           : $LOCAL_SHA"
echo "  onchain sha256 (first ${LOCAL_LEN} B): $HEAD_SHA"
[[ "$LOCAL_SHA" == "$HEAD_SHA" ]] \
  || die "CODE BYTES DIFFER — deployed program is NOT this source. Rebuild+redeploy (Phase 0.1/0.2)."

# 2) Everything after must be zero padding (no smuggled trailing bytes).
if (( ONCHAIN_LEN > LOCAL_LEN )); then
  NONZERO_TAIL="$(tail -c "+$((LOCAL_LEN + 1))" "$ONCHAIN" | tr -d '\0' | wc -c | tr -d ' ')"
  [[ "$NONZERO_TAIL" == "0" ]] \
    || die "trailing $((ONCHAIN_LEN - LOCAL_LEN)) B contain $NONZERO_TAIL non-zero byte(s) — not pure padding. NOT a clean match."
  echo "  trailing $((ONCHAIN_LEN - LOCAL_LEN)) B: all zero (padding) ✓"
fi

echo
green "✓ MATCH — on-chain bytecode == pinned source."
green "  Phase 0.3 gate PASSED for $PROGRAM_ID on $CLUSTER_URL."
echo "  Record this sha256 in docs/audit_submission.md: $LOCAL_SHA"
