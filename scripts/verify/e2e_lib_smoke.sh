#!/usr/bin/env bash
# Smoke test for the shared e2e helper library (.issues/076).
#
# `scripts/e2e/` is run by hand against devnet, so a refactor of its helpers has
# no CI that would catch a break. This exercises everything in
# `scripts/e2e/lib/` that does not need a chain or a Worker: the tallies, the
# JSON assertion, and both address-less branches of the ownership assertion.
# The on-chain branch is devnet-only and is deliberately not covered here.
#
# Usage: bash scripts/verify/e2e_lib_smoke.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

E2E_DIR="scripts/e2e"
# shellcheck source=scripts/e2e/lib/common.sh
source "$E2E_DIR/lib/common.sh"
# shellcheck source=scripts/e2e/lib/solana.sh
source "$E2E_DIR/lib/solana.sh"

failures=0
expect() {
    local label="$1" expected="$2" actual="$3"
    case "$actual" in
        "$expected") echo "  ok   $label" ;;
        *)
            echo "  FAIL $label: expected '$expected', got '$actual'"
            failures=$((failures + 1))
            ;;
    esac
}

echo "━━━ tallies ━━━"
expect "counters start at zero" "0/0/0" "$PASS/$FAIL/$SKIP"
pass "sample" > /dev/null
pass "sample" > /dev/null
fail "sample" > /dev/null
skip "sample" > /dev/null
expect "each helper tallies once" "2/1/1" "$PASS/$FAIL/$SKIP"

echo "━━━ check_json ━━━"
body='{"success":true,"data":{"id":"evt-1","count":3}}'
# `set -e` would abort on the expected non-zero returns, so capture the status
# instead of letting it propagate.
status_of() { local rc=0; "$@" > /dev/null || rc=$?; echo "$rc"; }

expect "matching value returns 0" "0" \
    "$(status_of check_json "$body" "['data']['id']" "evt-1")"
expect "mismatched value returns 1" "1" \
    "$(status_of check_json "$body" "['data']['id']" "evt-2")"
expect "unparseable body returns 1" "1" \
    "$(status_of check_json "not json at all" "['data']" "x")"

echo "━━━ assert_escrow_owned_by_program (address-less branches) ━━━"
# Set by the callers; the on-chain branch is not reached below.
ESCROW_PROGRAM="C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T"
RPC_URL="https://api.devnet.solana.com"

PASS=0; FAIL=0; SKIP=0
SKIP_SETUP=true
assert_escrow_owned_by_program "Source" "" > /dev/null
expect "empty address under --skip-setup skips" "0/0/1" "$PASS/$FAIL/$SKIP"

PASS=0; FAIL=0; SKIP=0
SKIP_SETUP=false
assert_escrow_owned_by_program "Source" "" > /dev/null
expect "empty address otherwise fails" "0/1/0" "$PASS/$FAIL/$SKIP"

echo "━━━ signer plumbing ━━━"
expect "signer script resolves from the library" \
    "ok" "$([ -f "$E2E_LIB_PARENT/sign_and_submit.py" ] && echo ok || echo missing)"

echo "━━━ every e2e script resolves its helpers ━━━"
# Catches the regression this refactor could introduce: a call site left
# pointing at a helper name that no longer exists anywhere (the `log_pass` →
# `pass` rename in test_escrow_surfpool.sh), or a script that stopped sourcing
# the library but still calls into it.
HELPERS="pass fail skip info warn section check_json sign_and_submit_tx assert_escrow_owned_by_program"
for script in "$E2E_DIR"/*.sh; do
    missing=""
    for helper in $HELPERS; do
        grep -qE "(^|[^a-zA-Z0-9_])${helper}[[:space:]]+[\"'$-]" "$script" || continue
        grep -q "source \"\$SCRIPT_DIR/lib/common.sh\"" "$script" || missing="$missing $helper(no-common)"
        case "$helper" in
            sign_and_submit_tx|assert_escrow_owned_by_program)
                grep -q "source \"\$SCRIPT_DIR/lib/solana.sh\"" "$script" \
                    || missing="$missing $helper(no-solana)"
                ;;
        esac
    done
    expect "$(basename "$script")" "" "$missing"
done

# And nothing anywhere still defines a copy of a library helper.
duplicates=$(grep -lE "^(pass|fail|skip|info|warn|section|check_json|sign_and_submit_tx|assert_escrow_owned_by_program|log_pass|log_fail|log_info|log_warn)\(\) *\{" "$E2E_DIR"/*.sh || true)
expect "no script redefines a library helper" "" "$duplicates"

echo ""
case "$failures" in
    0) echo "✅ e2e library smoke test clean." ;;
    *) echo "❌ $failures assertion(s) failed."; exit 1 ;;
esac
