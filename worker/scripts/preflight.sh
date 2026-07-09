#!/usr/bin/env bash
# preflight.sh — §3.5 pre-deploy gate for BeThere production deploys.
#
# Reads the `.last-green` sentinel written by `flow-harness` when all E2E flows
# pass, and enforces that a green run exists within a freshness window
# (default 3600s = 1 hour). A production deploy that fails this gate is blocked
# unless `deploy.sh` is invoked with `--force` (which logs a mandatory audit
# entry to worker/scripts/.preflight-bypass.log).
#
# The gate reads the sentinel's MTIME — not its contents. The sentinel body
# (an ISO timestamp + run directory) exists for human triage; mtime is the
# load-bearing signal so the gate stays robust against content-format drift.
#
# Usage:
#   preflight.sh                 # Check the gate (default action).
#   preflight.sh check           #   (same — explicit form)
#   preflight.sh run             # Run the harness, then check the gate.
#   preflight.sh run-only        # Run the harness only (skip the gate check).
#   preflight.sh status          # Print sentinel state (always exits 0).
#   preflight.sh --help          # Show this help.
#
# Exit codes (mirrors flow-harness/src/main.rs so CI can treat them uniformly):
#   0 — gate passed / command succeeded
#   1 — gate failed (sentinel missing or stale), or a flow failed
#   2 — usage error or misconfiguration
#
# Environment:
#   PREFLIGHT_MAX_AGE_SECONDS    Freshness window in seconds (default: 3600).
#   PREFLIGHT_RESULTS_ROOT       Override path to flow-harness/results
#                                (default: <repo-root>/flow-harness/results).
#   PREFLIGHT_HARNESS_RELEASE    Set to 1 to build the harness in --release mode
#                                for `run`/`run-only` (default: debug build).
#   FLOW_HARNESS_WORKER_URL      Staging worker base URL (required for run*).
#
# See plan 005 §3.5 and handover 126 for the design rationale.
set -euo pipefail

# ── Path resolution ──────────────────────────────────────────────────────────
# SCRIPT_DIR = worker/scripts; REPO_ROOT = SCRIPT_DIR/../..
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
HARNESS_DIR="$REPO_ROOT/flow-harness"
RESULTS_ROOT="${PREFLIGHT_RESULTS_ROOT:-$HARNESS_DIR/results}"
SENTINEL="$RESULTS_ROOT/.last-green"

# ── Helpers ──────────────────────────────────────────────────────────────────

# Portable mtime-in-epoch-seconds. macOS `stat -f %m` and GNU `stat -c %Y`
# disagree on flags; detect the kernel and dispatch.
file_mtime_epoch() {
    local file="$1"
    if [ "$(uname -s)" = "Darwin" ]; then
        stat -f %m "$file"
    else
        stat -c %Y "$file"
    fi
}

# Format an epoch second as ISO-8601 UTC for log lines (macOS `date -r`,
# GNU `date -d @epoch`).
epoch_to_iso() {
    local epoch="$1"
    if [ "$(uname -s)" = "Darwin" ]; then
        date -u -r "$epoch" +"%Y-%m-%dT%H:%M:%SZ"
    else
        date -u -d "@$epoch" +"%Y-%m-%dT%H:%M:%SZ"
    fi
}

print_help() {
    cat <<'EOF'
preflight.sh — §3.5 pre-deploy gate for BeThere production deploys.

Usage:
  preflight.sh                 Check the gate (default): read .last-green, verify freshness.
  preflight.sh check           Same as above (explicit).
  preflight.sh run             Run the harness, then check the gate.
  preflight.sh run-only        Run the harness only (skip the gate check).
  preflight.sh status          Print sentinel state (always exits 0).
  preflight.sh --help          Show this help.

Exit codes:
  0  gate passed / command succeeded
  1  gate failed (sentinel missing or stale) or a flow failed
  2  usage error or misconfiguration

Environment:
  PREFLIGHT_MAX_AGE_SECONDS    Freshness window in seconds (default: 3600).
  PREFLIGHT_RESULTS_ROOT       Override path to flow-harness/results.
  PREFLIGHT_HARNESS_RELEASE    Set to 1 to build the harness in --release for run*.
  FLOW_HARNESS_WORKER_URL      Staging worker base URL (required for run*).
EOF
}

# ── Subcommands ──────────────────────────────────────────────────────────────

# Run the harness via cargo. Requires FLOW_HARNESS_WORKER_URL. Propagates the
# harness exit code (0 all-pass, 1 flow-fail, 2 misconfig). On a green run the
# harness itself touches the sentinel, so a subsequent `check_gate` sees a
# fresh mtime.
run_harness() {
    local worker_url="${FLOW_HARNESS_WORKER_URL:-}"
    if [ -z "$worker_url" ]; then
        echo "❌ preflight: FLOW_HARNESS_WORKER_URL is required to run the harness." >&2
        echo "   Example: export FLOW_HARNESS_WORKER_URL=https://bethere-staging.solana-thailand.workers.dev" >&2
        return 2
    fi

    if [ ! -d "$HARNESS_DIR" ]; then
        echo "❌ preflight: flow-harness crate not found at $HARNESS_DIR" >&2
        return 2
    fi

    local release_flag=""
    if [ "${PREFLIGHT_HARNESS_RELEASE:-0}" = "1" ]; then
        release_flag="--release"
    fi

    echo "🏃 Running flow-harness against $worker_url ..."
    # Subshell so the harness's own `cd` does not move this script's CWD.
    ( cd "$HARNESS_DIR" && cargo run $release_flag --package flow-harness -- --worker "$worker_url" )
}

# Check the gate: sentinel must exist and be younger than PREFLIGHT_MAX_AGE_SECONDS.
check_gate() {
    local max_age="${PREFLIGHT_MAX_AGE_SECONDS:-3600}"

    if ! [[ "$max_age" =~ ^[0-9]+$ ]] || [ "$max_age" -le 0 ]; then
        echo "❌ preflight: PREFLIGHT_MAX_AGE_SECONDS must be a positive integer (got: $max_age)" >&2
        return 2
    fi

    if [ ! -f "$SENTINEL" ]; then
        echo "❌ Preflight gate FAILED: no green run on record." >&2
        echo "   Sentinel not found: $SENTINEL" >&2
        echo "   Run the harness first: bash worker/scripts/preflight.sh run" >&2
        return 1
    fi

    local now_epoch mtime_epoch age
    now_epoch=$(date +%s)
    mtime_epoch=$(file_mtime_epoch "$SENTINEL")
    age=$(( now_epoch - mtime_epoch ))

    if [ "$age" -gt "$max_age" ]; then
        local stale_iso
        stale_iso=$(epoch_to_iso "$mtime_epoch")
        echo "❌ Preflight gate FAILED: last green run is stale." >&2
        echo "   Sentinel mtime: $stale_iso (${age}s ago, max allowed ${max_age}s)" >&2
        echo "   Re-run the harness: bash worker/scripts/preflight.sh run" >&2
        return 1
    fi

    local fresh_iso
    fresh_iso=$(epoch_to_iso "$mtime_epoch")
    echo "✅ Preflight gate passed (last green at ${fresh_iso}, ${age}s ago; max ${max_age}s)."
    return 0
}

# Print sentinel state. Always exits 0 — informational only, never blocks.
status_gate() {
    if [ ! -f "$SENTINEL" ]; then
        echo "○  No green run on record."
        echo "   Sentinel missing: $SENTINEL"
        return 0
    fi

    local now_epoch mtime_epoch age max_age mtime_iso freshness
    now_epoch=$(date +%s)
    mtime_epoch=$(file_mtime_epoch "$SENTINEL")
    age=$(( now_epoch - mtime_epoch ))
    max_age="${PREFLIGHT_MAX_AGE_SECONDS:-3600}"
    mtime_iso=$(epoch_to_iso "$mtime_epoch")

    if [ "$age" -gt "$max_age" ]; then
        freshness="STALE"
    else
        freshness="FRESH"
    fi

    echo "●  Sentinel: $SENTINEL"
    echo "   mtime:      $mtime_iso"
    echo "   age:        ${age}s"
    echo "   max_age:    ${max_age}s"
    echo "   freshness:  $freshness"
    echo "   contents (informational):"
    # Indent the sentinel body (ISO ts + run dir) for triage readability.
    sed 's/^/     /' "$SENTINEL"
    return 0
}

# ── Arg parsing ──────────────────────────────────────────────────────────────

subcommand="${1:-check}"
if [ $# -gt 0 ]; then
    shift
fi

case "$subcommand" in
    check)
        check_gate
        ;;
    run)
        # Run the harness; if it exits 0 (all-passed), the sentinel was just
        # touched, so check_gate will confirm freshness. A non-zero harness
        # exit short-circuits the && and propagates as this script's exit code.
        run_harness && check_gate
        ;;
    run-only)
        run_harness
        ;;
    status)
        status_gate
        ;;
    -h|--help|help)
        print_help
        exit 0
        ;;
    *)
        echo "❌ preflight: unknown subcommand '$subcommand'" >&2
        echo "   Usage: preflight.sh [check|run|run-only|status|--help]" >&2
        exit 2
        ;;
esac
