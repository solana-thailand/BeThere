#!/usr/bin/env bash
# check_size.sh — Worker size budget guard.
#
# Runs `wrangler deploy --dry-run`, parses the gzip-compressed Worker size
# from the output, and fails (exits non-zero) if it exceeds the configured
# budget. Keeps deployments clear of Cloudflare's Worker size limit.
#
# Reference (Cloudflare docs):
#   Free tier hard limit:  3.00 MB after gzip
#   Paid tier hard limit: 10.00 MB after gzip
#
# Usage:
#   bash scripts/check_size.sh                       # Default budget (2.5 MiB)
#   SIZE_BUDGET_MIB=2.0 bash scripts/check_size.sh   # Custom budget
#   SIZE_WARN_MIB=2.0  bash scripts/check_size.sh    # Custom warning threshold
#   SKIP_BUILD=1 bash scripts/check_size.sh          # Measure existing artifacts only
#
# Budgets (after gzip compression):
#   Free tier hard limit:  3.00 MiB  (Cloudflare rejects deploy above this)
#   Default budget:        2.50 MiB  (0.50 MiB buffer — recommended)
#   Default warn:          2.25 MiB  (yellow flag)
#
# Exit codes:
#   0 — under budget (or under warning threshold — still passes)
#   1 — OVER budget (CI should block deploy)
#   2 — wrangler failed, build failed, or output could not be parsed

set -euo pipefail
cd "$(dirname "$0")/.."

# Force plain wrangler output (no ANSI codes) so grep parses cleanly.
export NO_COLOR=1

# ── fnm/Node setup (mirror deploy.sh) ───────────────────────────────────────
# wrangler runs via npx, which needs node. Use fnm's default node if present.
FNM_NODE_BIN="$HOME/.local/share/fnm/node-versions/v24.16.0/installation/bin"
if [ ! -d "$FNM_NODE_BIN" ]; then
    FNM_NODE_BIN="$(find "$HOME/.local/share/fnm/node-versions" -path "*/installation/bin" -type d 2>/dev/null | head -1)"
fi
if [ -n "$FNM_NODE_BIN" ] && [ -d "$FNM_NODE_BIN" ]; then
    export PATH="$FNM_NODE_BIN:$PATH"
fi

# ── Yarn PnP workaround (mirror deploy.sh) ──────────────────────────────────
# wrangler's esbuild bundler crashes if Yarn PnP's ~/.pnp.cjs is present.
# Temporarily move it for the duration of this script, restore on exit.
PNP_FILE="$HOME/.pnp.cjs"
PNP_BACKUP="$HOME/.pnp.cjs.bak"
MOVED=false

move_pnp() {
    if [ -f "$PNP_FILE" ] && [ ! -f "$PNP_BACKUP" ]; then
        mv "$PNP_FILE" "$PNP_BACKUP"
        MOVED=true
    fi
}

restore_pnp() {
    if [ "$MOVED" = true ] && [ -f "$PNP_BACKUP" ]; then
        mv "$PNP_BACKUP" "$PNP_FILE"
    fi
}

# ── Configuration ───────────────────────────────────────────────────────────
BUDGET_MIB="${SIZE_BUDGET_MIB:-2.5}"
WARN_MIB="${SIZE_WARN_MIB:-2.25}"
HARD_LIMIT_MIB="${SIZE_HARD_LIMIT_MIB:-3.0}"
SKIP_BUILD="${SKIP_BUILD:-0}"

# ── Prepare temp dir + cleanup trap ─────────────────────────────────────────
DRY_DIR="$(mktemp -d)"
cleanup() {
    restore_pnp
    rm -rf "$DRY_DIR" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

move_pnp

echo "📏 Worker size budget guard"
echo "   budget:     ${BUDGET_MIB} MiB (over → exit 1)"
echo "   warn at:    ${WARN_MIB} MiB"
echo "   hard limit: ${HARD_LIMIT_MIB} MiB (Cloudflare free tier rejects above this)"
echo ""

# ── Obtain worker size ──────────────────────────────────────────────────────
# Two paths:
#   SKIP_BUILD=1 → measure existing build/worker/*.wasm with gzip directly.
#                  Fast, approximate (skips the JS shim, ~1-2 KB).
#   default      → run full `wrangler deploy --dry-run`. Authoritative.

WORKER_MIB=""
SIZE_LINE=""

if [ "$SKIP_BUILD" = "1" ]; then
    # Fast path — measure existing artifacts
    WASM_FILE="$(ls build/worker/*.wasm 2>/dev/null | head -1 || true)"
    if [ -z "$WASM_FILE" ] || [ ! -f "$WASM_FILE" ]; then
        echo "❌ SKIP_BUILD=1 but no build/worker/*.wasm found."
        echo "   Run the full check (without SKIP_BUILD) to produce artifacts first."
        exit 2
    fi
    GZIP_BYTES=$(gzip -c -9 "$WASM_FILE" | wc -c | tr -d ' ')
    WORKER_MIB=$(python3 -c "print(${GZIP_BYTES} / 1048576.0)")
    SIZE_LINE="artifact gzip: ${GZIP_BYTES} B  (from ${WASM_FILE})"
    echo "⏩  SKIP_BUILD=1 — measuring existing artifact"
else
    # Authoritative path — run wrangler dry-run
    echo "📦 Running wrangler deploy --dry-run (this may take a minute)..."
    DRY_OUTPUT="$(npx wrangler deploy --dry-run --outdir "$DRY_DIR" 2>&1)" || {
        echo "❌ wrangler dry-run failed"
        echo ""
        echo "$DRY_OUTPUT" | tail -30
        exit 2
    }

    # Expected line, e.g.:
    #   "Total Upload: 1792.45 KiB / gzip: 1792.45 KiB"
    #   "Total Upload: 1.72 MiB / gzip: 1.72 MiB"
    # Take the last match in case wrangler prints intermediate steps.
    SIZE_LINE="$(echo "$DRY_OUTPUT" | grep -iE 'Total Upload:.*gzip:' | tail -1 || true)"

    if [ -z "$SIZE_LINE" ]; then
        echo "❌ Could not find 'Total Upload: ... gzip:' line in wrangler output."
        echo ""
        echo "$DRY_OUTPUT" | tail -30
        exit 2
    fi

    # Parse gzip number + unit via python3 (handles B / KiB / MiB, floats).
    # Pipe the line through stdin to avoid shell-interpolation issues.
    WORKER_MIB="$(printf '%s\n' "$SIZE_LINE" | python3 -c '
import re, sys
line = sys.stdin.read()
m = re.search(r"gzip:\s*([0-9.]+)\s*(B|KiB|MiB)", line, re.IGNORECASE)
if not m:
    sys.stderr.write("no gzip match in line: %r\n" % line)
    sys.exit(2)
val = float(m.group(1))
unit = m.group(2).lower()
multiplier = {"b": 1.0, "kib": 1024.0, "mib": 1048576.0}[unit]
print(val * multiplier / 1048576.0)
')" || {
        echo "❌ Failed to parse gzip size from line: '$SIZE_LINE'"
        exit 2
    }
fi

echo "   source: $SIZE_LINE"
echo ""

# ── Compare against budget ──────────────────────────────────────────────────
# Use python3 for clean float math + formatted output. Capture exit code under
# set -e via `|| exit_code=$?` pattern.
exit_code=0
python3 - "$WORKER_MIB" "$BUDGET_MIB" "$WARN_MIB" "$HARD_LIMIT_MIB" <<'PYEOF' || exit_code=$?
import sys

worker_mib = float(sys.argv[1])
budget_mib = float(sys.argv[2])
warn_mib   = float(sys.argv[3])
hard_mib   = float(sys.argv[4])

worker_bytes = int(worker_mib * 1048576)
budget_used  = (worker_mib / budget_mib) * 100.0
remaining    = budget_mib - worker_mib
hard_buffer  = hard_mib - worker_mib

print(f"   worker size:   {worker_mib:.3f} MiB ({worker_bytes:,} bytes gzip)")
print(f"   vs budget:     {budget_mib:.2f} MiB ({budget_used:.1f}% used, {remaining:+.3f} MiB remaining)")
print(f"   vs hard cap:   {hard_mib:.2f} MiB ({hard_buffer:+.3f} MiB buffer)")
print()

if worker_mib > hard_mib:
    over_hard = worker_mib - hard_mib
    print(f"🚨 CRITICAL: over Cloudflare hard limit by {over_hard:.3f} MiB")
    print(f"   Deploy WILL be rejected by Cloudflare. Reduce worker size immediately.")
    sys.exit(1)
elif worker_mib > budget_mib:
    over = worker_mib - budget_mib
    print(f"❌ OVER BUDGET by {over:.3f} MiB")
    print(f"   Within Cloudflare limit but exceeds self-imposed budget.")
    print(f"   Trim dependencies, split via Service Bindings, or raise SIZE_BUDGET_MIB if intentional.")
    sys.exit(1)
elif worker_mib > warn_mib:
    print(f"⚠️  WARN: over {warn_mib:.2f} MiB warning threshold")
    print(f"   Within budget but approaching the limit. Consider trimming dependencies soon.")
    sys.exit(0)
else:
    print(f"✅ Under budget")
    sys.exit(0)
PYEOF

echo ""
if [ "$exit_code" -eq 0 ]; then
    echo "✅ Size check passed"
else
    echo "❌ Size check FAILED"
fi
exit "$exit_code"
