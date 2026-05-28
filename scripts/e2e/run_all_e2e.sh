#!/usr/bin/env bash
# ============================================================================
# BeThere E2E Test Orchestrator
# ============================================================================
# Runs all devnet E2E test scripts sequentially and reports a summary.
#
# Prerequisites:
#   - `cd worker && npx wrangler dev --port 8787` running
#   - DEV_MODE=1 in worker/.dev.vars
#   - HELIUS_API_KEY in worker/.dev.vars
#   - solana CLI installed + configured for devnet
#   - Devnet USDC in test wallet (use https://faucet.circle.com/)
#
# Usage:
#   bash scripts/e2e/run_all_e2e.sh
#   bash scripts/e2e/run_all_e2e.sh --skip-lifecycle
#   bash scripts/e2e/run_all_e2e.sh --only rollover
#   bash scripts/e2e/run_all_e2e.sh --only full-lifecycle
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# Defaults
BASE_URL="${BASE_URL:-http://localhost:8787}"
SKIP_LIFECYCLE=false
SKIP_ESCROW=false
SKIP_ROLLOVER=false
SKIP_FULL_LIFECYCLE=false
ONLY=""

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-lifecycle)      SKIP_LIFECYCLE=true; shift ;;
        --skip-escrow)         SKIP_ESCROW=true; shift ;;
        --skip-rollover)       SKIP_ROLLOVER=true; shift ;;
        --skip-full-lifecycle) SKIP_FULL_LIFECYCLE=true; shift ;;
        --only)
            ONLY="${2:-}"
            shift 2
            ;;
        *) shift ;;
    esac
done

echo ""
echo -e "${BOLD}🧪 BeThere E2E Test Orchestrator${NC}"
echo "   BASE_URL: $BASE_URL"
echo ""

# Pre-flight health check
HEALTH=$(curl -s "$BASE_URL/api/health" 2>&1 || echo "")
if ! echo "$HEALTH" | grep -q '"ok"'; then
    echo -e "  ${RED}❌ Worker not healthy at $BASE_URL${NC}"
    echo "     Start with: cd worker && npx wrangler dev --port 8787"
    echo "     Response: $(echo "$HEALTH" | head -c 200)"
    exit 1
fi
echo -e "  ${GREEN}✅ Worker healthy${NC}"
echo ""

# Test definitions: name | script | description
declare -a TESTS=(
    "lifecycle|test_lifecycle.sh|Escrow lifecycle (create → deactivate → claim → close)"
    "escrow|test_escrow_devnet.sh|Full escrow flow (deposit → check-in → refund → close)"
    "rollover|test_rollover_devnet.sh|Rollover deposit + refund from target + close"
    "full-lifecycle|test_rollover_full_lifecycle.sh|Full rollover lifecycle (2 attendees, USDC round-trip)"
)

TOTAL=0
PASSED=0
FAILED=0
SKIPPED=0
declare -a RESULTS=()

START_TIME=$(date +%s)

for TEST_DEF in "${TESTS[@]}"; do
    IFS='|' read -r NAME SCRIPT DESC <<< "$TEST_DEF"

    # Check skip conditions
    SKIP=false
    case "$NAME" in
        lifecycle)       [ "$SKIP_LIFECYCLE" = true ] && SKIP=true ;;
        escrow)          [ "$SKIP_ESCROW" = true ] && SKIP=true ;;
        rollover)        [ "$SKIP_ROLLOVER" = true ] && SKIP=true ;;
        full-lifecycle)  [ "$SKIP_FULL_LIFECYCLE" = true ] && SKIP=true ;;
    esac

    # --only filter
    if [ -n "$ONLY" ] && [ "$NAME" != "$ONLY" ]; then
        SKIP=true
    fi

    TOTAL=$((TOTAL + 1))

    if [ "$SKIP" = true ]; then
        echo -e "  ${YELLOW}⏭️  SKIP${NC} $NAME: $DESC"
        SKIPPED=$((SKIPPED + 1))
        RESULTS+=("$NAME|SKIP|$DESC")
        continue
    fi

    echo -e "${CYAN}━━━ Running: $NAME ━━━${NC}"
    echo -e "  $DESC"
    echo ""

    TEST_START=$(date +%s)

    if bash "$SCRIPT_DIR/$SCRIPT" 2>&1; then
        TEST_END=$(date +%s)
        DURATION=$((TEST_END - TEST_START))
        echo ""
        echo -e "  ${GREEN}✅ PASSED${NC} $NAME (${DURATION}s)"
        PASSED=$((PASSED + 1))
        RESULTS+=("$NAME|PASS|${DURATION}s|$DESC")
    else
        TEST_END=$(date +%s)
        DURATION=$((TEST_END - TEST_START))
        echo ""
        echo -e "  ${RED}❌ FAILED${NC} $NAME (${DURATION}s)"
        FAILED=$((FAILED + 1))
        RESULTS+=("$NAME|FAIL|${DURATION}s|$DESC")
    fi

    echo ""
done

END_TIME=$(date +%s)
TOTAL_DURATION=$((END_TIME - START_TIME))

# ============================================================================
# Summary
# ============================================================================
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BOLD}  E2E Test Summary${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

printf "  %-20s %-8s %-10s %s\n" "Test" "Status" "Time" "Description"
printf "  %-20s %-8s %-10s %s\n" "────" "──────" "────" "───────────"

for RESULT in "${RESULTS[@]}"; do
    IFS='|' read -r NAME STATUS TIME_DESC DESC <<< "$RESULT"
    case "$STATUS" in
        PASS) STATUS_FMT="${GREEN}PASS${NC}" ;;
        FAIL) STATUS_FMT="${RED}FAIL${NC}" ;;
        SKIP) STATUS_FMT="${YELLOW}SKIP${NC}" ;;
    esac
    printf "  %-20s %-8s %-10s %s\n" "$NAME" "$(echo -e $STATUS_FMT)" "$TIME_DESC" "$DESC"
done

echo ""
echo -e "  Total:  $TOTAL"
echo -e "  ${GREEN}Passed: $PASSED${NC}"
echo -e "  ${RED}Failed: $FAILED${NC}"
echo -e "  ${YELLOW}Skipped: $SKIPPED${NC}"
echo -e "  Duration: ${TOTAL_DURATION}s"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "  ${GREEN}${BOLD}🎉 All E2E tests PASSED!${NC}"
    exit 0
else
    echo -e "  ${RED}${BOLD}❌ $FAILED test(s) FAILED${NC}"
    exit 1
fi
