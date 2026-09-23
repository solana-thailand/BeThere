#!/usr/bin/env bash
# Worker upload-size budget gate.
#
# Cloudflare enforces the Worker size limit on the COMPRESSED bundle, and
# BeThere is on the free plan: 3 MiB after gzip. Until 2026-09-22 nothing in
# this repo measured it. `deploy.sh` had no size check, and CI's `wasm-build`
# job built the wasm and threw the number away — so "will this still fit?" was
# answered by guessing, and the answer would have arrived as a failed deploy.
#
# What is measured is wrangler's OWN bundler output (`wrangler deploy
# --dry-run --outdir`), not the raw cargo artifact: wrangler runs esbuild over
# the shim and the wasm-bindgen glue, and that bundle — shim.js plus the wasm —
# is literally what gets uploaded. Source maps and wrangler's outdir README are
# excluded because they are not part of the upload.
#
# Thresholds live in worker/.size-budget, not here, so a budget change is a
# one-line diff that shows up in review.
#
# Usage:
#   bash scripts/verify/worker_size_budget.sh                  # build + measure
#   bash scripts/verify/worker_size_budget.sh --dir <outdir>   # measure an existing bundle
#   bash scripts/verify/worker_size_budget.sh --keep <outdir>  # build into <outdir>, measure, keep it
#   bash scripts/verify/worker_size_budget.sh --update-baseline
#
# Exit: 0 green (or warn), 1 over the fail line or broken invocation.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

BUDGET_FILE="worker/.size-budget"
BUNDLE_DIR=""
KEEP_DIR=""
UPDATE_BASELINE=false
TMP_DIR=""

usage() {
  sed -n '3,25p' "${BASH_SOURCE[0]}" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --dir)
      [ $# -ge 2 ] || { echo "❌ --dir needs a path" >&2; usage; }
      BUNDLE_DIR="$2"
      shift 2
      ;;
    --keep)
      [ $# -ge 2 ] || { echo "❌ --keep needs a path" >&2; usage; }
      KEEP_DIR="$2"
      shift 2
      ;;
    --update-baseline)
      UPDATE_BASELINE=true
      shift
      ;;
    -h|--help) usage ;;
    *) echo "❌ Unknown argument: $1" >&2; usage ;;
  esac
done

[ -z "$BUNDLE_DIR" ] || [ -z "$KEEP_DIR" ] || { echo "❌ --dir and --keep are exclusive" >&2; usage; }

cleanup() {
  [ -n "$TMP_DIR" ] && rm -rf "$TMP_DIR"
  return 0
}
trap cleanup EXIT

# ── Budget ──────────────────────────────────────────────────────────────────
# Parsed, not sourced: the file is data, and `source` would execute anything a
# bad merge dropped into it.
read_budget() {
  local key="$1" value
  value=$(awk -F= -v k="$key" '$1==k {print $2; exit}' "$BUDGET_FILE" | tr -d ' \r')
  [ -n "$value" ] || { echo "❌ $BUDGET_FILE is missing $key" >&2; exit 1; }
  printf '%s' "$value"
}

[ -f "$BUDGET_FILE" ] || { echo "❌ Budget file not found: $BUDGET_FILE" >&2; exit 1; }

CEILING_BYTES=$(read_budget CEILING_BYTES)
FAIL_PCT=$(read_budget FAIL_PCT)
WARN_PCT=$(read_budget WARN_PCT)
BASELINE_BYTES=$(read_budget BASELINE_BYTES)
BASELINE_DATE=$(read_budget BASELINE_DATE)

FAIL_BYTES=$(( CEILING_BYTES * FAIL_PCT / 100 ))
WARN_BYTES=$(( CEILING_BYTES * WARN_PCT / 100 ))

# ── Bundle ──────────────────────────────────────────────────────────────────
if [ -z "$BUNDLE_DIR" ]; then
  command -v npx >/dev/null 2>&1 || {
    echo "❌ npx not found, and no --dir given. Either install node deps or pass" >&2
    echo "   --dir pointing at an already-built bundle." >&2
    exit 1
  }
  # --keep hands the bundle to a later step (the worker leak scan in CI);
  # otherwise it is a temp dir removed on exit.
  if [ -n "$KEEP_DIR" ]; then
    mkdir -p "$KEEP_DIR"
    BUNDLE_DIR="$KEEP_DIR"
  else
    TMP_DIR=$(mktemp -d)
    BUNDLE_DIR="$TMP_DIR"
  fi
  echo "📦 Bundling worker (wrangler deploy --dry-run) ..."
  if ! (cd worker && CI=true npx wrangler deploy --env="" --dry-run --outdir "$BUNDLE_DIR" >/dev/null 2>&1); then
    echo "❌ Dry-run bundling failed — cannot measure. Re-run the command by hand" >&2
    echo "   to see the build error:" >&2
    echo "     cd worker && npx wrangler deploy --env=\"\" --dry-run --outdir /tmp/x" >&2
    exit 1
  fi
fi

[ -d "$BUNDLE_DIR" ] || { echo "❌ Bundle directory not found: $BUNDLE_DIR" >&2; exit 1; }

# Everything wrangler uploads: the wasm module and the JS entry/glue. Source
# maps are not uploaded (`upload_source_maps` is unset) and neither is the
# README wrangler writes into an outdir.
files=()
while IFS= read -r f; do
  files+=("$f")
done < <(find "$BUNDLE_DIR" -type f \
  \( -name '*.wasm' -o -name '*.js' -o -name '*.mjs' \) \
  ! -name '*.map' | sort)

if [ "${#files[@]}" -eq 0 ]; then
  echo "❌ No uploadable files in $BUNDLE_DIR — the gate would pass vacuously." >&2
  exit 1
fi

# ── Measure ─────────────────────────────────────────────────────────────────
# `wc -c` rather than `stat`: BSD stat (-f%z) and GNU stat (-c%s) disagree, and
# this runs on both macOS and the CI runner.
total_gzip=0
total_raw=0
echo ""
printf '  %-52s %12s %12s\n' "file" "raw" "gzip"
printf '  %-52s %12s %12s\n' "----" "---" "----"
for f in "${files[@]}"; do
  raw=$(wc -c < "$f" | tr -d ' ')
  gzip_size=$(gzip -9 -c "$f" | wc -c | tr -d ' ')
  total_raw=$(( total_raw + raw ))
  total_gzip=$(( total_gzip + gzip_size ))
  printf '  %-52s %12s %12s\n' "$(basename "$f" | cut -c1-52)" "$raw" "$gzip_size"
done
printf '  %-52s %12s %12s\n' "TOTAL" "$total_raw" "$total_gzip"
echo ""

pct_x100=$(( total_gzip * 10000 / CEILING_BYTES ))
delta=$(( total_gzip - BASELINE_BYTES ))

printf '  gzip total : %s bytes (%d.%02d%% of the %s-byte free-plan ceiling)\n' \
  "$total_gzip" "$(( pct_x100 / 100 ))" "$(( pct_x100 % 100 ))" "$CEILING_BYTES"
printf '  baseline   : %s bytes (recorded %s), delta %+d bytes\n' \
  "$BASELINE_BYTES" "$BASELINE_DATE" "$delta"
printf '  warn / fail: %s / %s bytes (%s%% / %s%%)\n' \
  "$WARN_BYTES" "$FAIL_BYTES" "$WARN_PCT" "$FAIL_PCT"
echo ""

if [ "$UPDATE_BASELINE" = true ]; then
  today=$(date +%Y-%m-%d)
  tmp_budget="${BUDGET_FILE}.tmp"
  sed -e "s/^BASELINE_BYTES=.*/BASELINE_BYTES=${total_gzip}/" \
      -e "s/^BASELINE_DATE=.*/BASELINE_DATE=${today}/" \
      "$BUDGET_FILE" > "$tmp_budget"
  mv "$tmp_budget" "$BUDGET_FILE"
  echo "📝 Baseline updated to ${total_gzip} bytes (${today}) in ${BUDGET_FILE}."
  echo "   Commit this in the SAME commit as the change that grew the bundle."
  echo ""
fi

if [ "$total_gzip" -gt "$FAIL_BYTES" ]; then
  echo "❌ Worker bundle is OVER BUDGET: ${total_gzip} > ${FAIL_BYTES} bytes gzip."
  echo "   The Cloudflare free-plan hard limit is ${CEILING_BYTES} bytes after gzip;"
  echo "   this gate stops ${FAIL_PCT}% of the way there so the wall is hit in CI"
  echo "   and not mid-deploy."
  echo ""
  echo "   The per-file table above names the culprit. Usual causes:"
  echo "     • a new crate pulled into the worker (check the diff in Cargo.lock)"
  echo "     • a dependency added WITHOUT default-features = false"
  echo "     • an image/QR/crypto decoder — these are the big ones"
  echo ""
  echo "   If the growth is intentional and unavoidable, raise FAIL_PCT in"
  echo "   ${BUDGET_FILE} in its own commit that says why — do not edit it"
  echo "   inside a feature commit."
  exit 1
fi

if [ "$total_gzip" -gt "$WARN_BYTES" ]; then
  echo "⚠️  Worker bundle is past the warn line: ${total_gzip} > ${WARN_BYTES} bytes gzip."
  echo "   Still deployable, but stop adding dependencies to the worker and start"
  echo "   asking what can move out of it."
  exit 0
fi

echo "✅ Worker bundle fits the free plan with room to spare."
printf '   Headroom to the hard limit: %s bytes gzip.\n' "$(( CEILING_BYTES - total_gzip ))"
