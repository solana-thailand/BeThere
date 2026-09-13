#!/usr/bin/env bash
# ============================================================================
# BeThere E2E — shared output helpers
# ============================================================================
# Issue 076. Every script in `scripts/e2e/` used to carry its own copy of these
# helpers, so a fix to one copy left the siblings wrong — the shape that
# produced Issue 072, where an ownership assertion could never match and the
# duplicate had no single place to be noticed.
#
# Source it from a script in this directory:
#
#     SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
#     # shellcheck source=lib/common.sh
#     source "$SCRIPT_DIR/lib/common.sh"
#
# Not executable and has no side effects beyond defining the counters and the
# colour constants below.
# ============================================================================

# Colours. `YELLOW` is bright (`1;33`) so SKIP/WARN stay legible on a dark
# terminal; `test_full_e2e.sh` had `\031[1;33m` here, a typo for `\033`, which
# emitted a stray control byte instead of switching colour.
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
# Read by the sourcing script's banner and summary blocks, not in here.
# shellcheck disable=SC2034
BOLD='\033[1m'
NC='\033[0m'

# Assertion tallies. Scripts read these in their own summary blocks, which stay
# script-specific — the counts are shared, the wording is not.
PASS=0
FAIL=0
SKIP=0

pass() { PASS=$((PASS + 1)); echo -e "  ${GREEN}✅ PASS${NC} $1"; }
fail() { FAIL=$((FAIL + 1)); echo -e "  ${RED}❌ FAIL${NC} $1"; }
skip() { SKIP=$((SKIP + 1)); echo -e "  ${YELLOW}⏭️  SKIP${NC} $1"; }
info() { echo -e "  ${CYAN}ℹ️  INFO${NC} $1"; }
warn() { echo -e "  ${YELLOW}⚠️  WARN${NC} $1"; }
section() { echo -e "\n${CYAN}━━━ $1 ━━━${NC}"; }

# Assert that `$key` (a Python subscript expression such as `['data']['id']`)
# in the JSON `$response` equals `$expected`. Prints both values on mismatch so
# the caller's `fail` line does not have to.
check_json() {
    local response="$1"
    local key="$2"
    local expected="$3"
    local actual
    actual=$(echo "$response" | python3 -c "import sys,json; print(json.load(sys.stdin)$key)" 2>/dev/null || echo "PARSE_ERROR")
    if [ "$actual" = "$expected" ]; then
        return 0
    else
        echo "     expected: $expected"
        echo "     actual:   $actual"
        return 1
    fi
}
