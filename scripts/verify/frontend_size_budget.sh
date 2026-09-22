#!/usr/bin/env bash
# Frontend first-load size budget gate.
#
# The sibling of scripts/verify/worker_size_budget.sh, for the half of the
# product that nobody was measuring. Until 2026-09-22 the worker had a gate and
# the frontend had none — and the frontend is the bigger of the two: the
# deployed wasm alone is 1.68 MB brotli against a 1.57 MB whole-worker bundle.
# Every attendee downloads it, on venue mobile data, before they can see their
# ticket.
#
# Two deliberate differences from the worker gate:
#
#   1. It measures BROTLI AT QUALITY 4, not gzip and not brotli -11.
#      Cloudflare serves these assets `content-encoding: br`, so brotli is what
#      an attendee waits for — but it compresses them on the fly at a low
#      quality, not at 11. Measured against the deployed site on 2026-09-22:
#      the wasm arrives as 1,675,993 bytes, and the same artifact compresses
#      locally to 1,646,742 at q4 and 1,288,797 at q11. q4 is within 2% of what
#      is served; q11 would have understated every number here by ~28% and
#      called it a measurement. q11 is printed anyway, as `br11`, because the
#      gap between the two columns is worth something concrete: it is what
#      precompressing the assets at build time would save, per first load, for
#      no code change at all.
#      The worker gate measures gzip because Cloudflare defines the WORKER SIZE
#      LIMIT on gzip — different question, different artifact, different
#      compressor. gzip is printed here for continuity, not judged.
#
#   2. The primary threshold is GROWTH against a committed baseline, not a
#      percentage of a ceiling. There is no platform limit here to anchor to;
#      inventing one and calling it a limit would be dishonest. The failure
#      this gate exists to catch is not "we hit a wall", it is "nobody noticed
#      the bundle grew". CEILING_BYTES is a product budget and is documented as
#      such in the budget file; it is a backstop, not the main mechanism.
#
# What counts as "first load": everything dist/index.html references, plus
# index.html itself. All 22 stylesheets are <link rel=stylesheet> in the
# document head, so they are all render-blocking and all first-load, whatever
# page the visitor asked for.
#
# Usage:
#   bash scripts/verify/frontend_size_budget.sh                 # measure dist/
#   bash scripts/verify/frontend_size_budget.sh --build         # build first
#   bash scripts/verify/frontend_size_budget.sh --dir <dist>    # measure a dist
#   bash scripts/verify/frontend_size_budget.sh --update-baseline
#
# Exit: 0 green (or warn), 1 over budget or broken invocation.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

BUDGET_FILE="frontend-leptos/.size-budget"
DIST_DIR="frontend-leptos/dist"
DIST_GIVEN=false
DO_BUILD=false
UPDATE_BASELINE=false

usage() {
  sed -n '3,37p' "${BASH_SOURCE[0]}" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --dir)
      [ $# -ge 2 ] || { echo "❌ --dir needs a path" >&2; usage; }
      DIST_DIR="$2"
      DIST_GIVEN=true
      shift 2
      ;;
    --build) DO_BUILD=true; shift ;;
    --update-baseline) UPDATE_BASELINE=true; shift ;;
    -h|--help) usage ;;
    *) echo "❌ Unknown argument: $1" >&2; usage ;;
  esac
done

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
MAX_GROWTH_BYTES=$(read_budget MAX_GROWTH_BYTES)
WARN_GROWTH_BYTES=$(read_budget WARN_GROWTH_BYTES)

FAIL_BYTES=$(( CEILING_BYTES * FAIL_PCT / 100 ))
WARN_BYTES=$(( CEILING_BYTES * WARN_PCT / 100 ))

# ── Build ───────────────────────────────────────────────────────────────────
if [ "$DO_BUILD" = true ]; then
  echo "🏗️  Building frontend (frontend-leptos/build.sh) ..."
  (cd frontend-leptos && bash build.sh >/dev/null) || {
    echo "❌ Frontend build failed — cannot measure. Re-run it by hand to see why:" >&2
    echo "     cd frontend-leptos && bash build.sh" >&2
    exit 1
  }
fi

INDEX="$DIST_DIR/index.html"
[ -d "$DIST_DIR" ] || { echo "❌ dist not found: $DIST_DIR (try --build)" >&2; exit 1; }
[ -f "$INDEX" ] || { echo "❌ No index.html in $DIST_DIR — nothing to measure." >&2; exit 1; }

# ── Staleness ───────────────────────────────────────────────────────────────
# A gate that measures last week's build is a gate that reports last week's
# number, and it reports it in green. This is not artifact-picking by mtime
# (which is its own trap, and a different one) — it is a single question about
# one pair of paths: is the thing we are about to measure older than its input?
# Skipped for --dir, where the caller built the bundle and knows what it is.
if [ "$DO_BUILD" = false ] && [ "$DIST_GIVEN" = false ] && [ -d frontend-leptos/src ]; then
  newer=$(find frontend-leptos/src frontend-leptos/styles frontend-leptos/index.html \
            -newer "$INDEX" -type f -print -quit 2>/dev/null || true)
  if [ -n "$newer" ]; then
    echo "❌ $DIST_DIR is older than the sources that produce it." >&2
    echo "   Newer than $INDEX: $newer" >&2
    echo "   Measuring it would report the previous build's size, in green." >&2
    echo "   Re-run with --build, or pass --dir for a bundle you built yourself." >&2
    exit 1
  fi
fi

# ── Which files are "first load" ────────────────────────────────────────────
# Every local path index.html references, plus index.html itself. Paths that do
# not resolve to a file in dist are reported, not skipped: a reference that
# silently measures as zero is how a gate stops seeing a whole asset. The known
# non-dist references are worker API routes, which are listed rather than
# pattern-matched away so that a NEW unresolvable path is still noticed.
files=("$INDEX")
missing=()
while IFS= read -r ref; do
  [ -n "$ref" ] || continue
  case "$ref" in
    /api/*) continue ;;  # served by the worker, not shipped in dist
  esac
  if [ -f "$DIST_DIR$ref" ]; then
    files+=("$DIST_DIR$ref")
  else
    missing+=("$ref")
  fi
done < <(grep -oE '(href|src)="/[^"]+"' "$INDEX" | sed 's/.*="//; s/"$//' | sort -u)

if [ "${#missing[@]}" -gt 0 ]; then
  echo "❌ index.html references files that are not in $DIST_DIR:" >&2
  printf '     %s\n' "${missing[@]}" >&2
  echo "   Either the build is incomplete or a new non-dist route needs adding" >&2
  echo "   to the skip list in this script. Refusing to measure around it." >&2
  exit 1
fi

# A dist with only an index.html is not a frontend; without this the gate
# reports a few KiB as comfortably under budget and passes vacuously.
if [ "${#files[@]}" -lt 3 ]; then
  echo "❌ Only ${#files[@]} first-load file(s) in $DIST_DIR — the gate would" >&2
  echo "   pass vacuously on an empty or half-built bundle." >&2
  exit 1
fi

# ── Measure ─────────────────────────────────────────────────────────────────
# Node rather than a `brotli` CLI: node is already required by this repo
# (wrangler), whereas the brotli binary is not installed on macOS by default
# and would make the gate silently unavailable on a developer's machine.
command -v node >/dev/null 2>&1 || {
  echo "❌ node not found. It is needed for brotli, which is what Cloudflare" >&2
  echo "   actually serves these assets as." >&2
  exit 1
}

measured=$(node -e '
const z = require("zlib"), fs = require("fs");
const br = (b, q) => z.brotliCompressSync(b, {
  params: { [z.constants.BROTLI_PARAM_QUALITY]: q,
            [z.constants.BROTLI_PARAM_SIZE_HINT]: b.length },
}).length;
let out = [];
for (const p of process.argv.slice(1)) {
  const b = fs.readFileSync(p);
  // q4 is judged (what Cloudflare serves); q11 is shown (what precompressing
  // at build time would get instead).
  out.push([p, b.length, z.gzipSync(b, { level: 9 }).length, br(b, 4), br(b, 11)].join("\t"));
}
process.stdout.write(out.join("\n"));
' "${files[@]}")

total_raw=0; total_gzip=0; total_br=0; total_br11=0
echo ""
printf '  %-44s %10s %10s %10s %10s\n' "file" "raw" "gzip" "br4" "br11"
printf '  %-44s %10s %10s %10s %10s\n' "----" "---" "----" "---" "----"
while IFS=$'\t' read -r path raw gz br br11; do
  [ -n "$path" ] || continue
  total_raw=$(( total_raw + raw ))
  total_gzip=$(( total_gzip + gz ))
  total_br=$(( total_br + br ))
  total_br11=$(( total_br11 + br11 ))
  printf '  %-44s %10s %10s %10s %10s\n' "$(basename "$path" | cut -c1-44)" "$raw" "$gz" "$br" "$br11"
done <<< "$measured"
printf '  %-44s %10s %10s %10s %10s\n' \
  "TOTAL (${#files[@]} files)" "$total_raw" "$total_gzip" "$total_br" "$total_br11"
echo ""

pct_x100=$(( total_br * 10000 / CEILING_BYTES ))
delta=$(( total_br - BASELINE_BYTES ))

printf '  first load : %s bytes br4 (%d.%02d%% of the %s-byte product budget)\n' \
  "$total_br" "$(( pct_x100 / 100 ))" "$(( pct_x100 % 100 ))" "$CEILING_BYTES"
printf '  precompress: %s bytes at br11 — %s bytes an attendee pays for nothing,\n' \
  "$total_br11" "$(( total_br - total_br11 ))"
printf '               because Cloudflare compresses on the fly at ~q4.\n'
printf '  baseline   : %s bytes (recorded %s), delta %+d bytes\n' \
  "$BASELINE_BYTES" "$BASELINE_DATE" "$delta"
printf '  growth     : warn %+d / fail %+d bytes\n' \
  "$WARN_GROWTH_BYTES" "$MAX_GROWTH_BYTES"
printf '  budget     : warn %s / fail %s bytes (%s%% / %s%%)\n' \
  "$WARN_BYTES" "$FAIL_BYTES" "$WARN_PCT" "$FAIL_PCT"
echo ""

if [ "$UPDATE_BASELINE" = true ]; then
  today=$(date +%Y-%m-%d)
  tmp_budget="${BUDGET_FILE}.tmp"
  sed -e "s/^BASELINE_BYTES=.*/BASELINE_BYTES=${total_br}/" \
      -e "s/^BASELINE_DATE=.*/BASELINE_DATE=${today}/" \
      "$BUDGET_FILE" > "$tmp_budget"
  mv "$tmp_budget" "$BUDGET_FILE"
  echo "📝 Baseline updated to ${total_br} bytes br4 (${today}) in ${BUDGET_FILE}."
  echo "   Commit this in the SAME commit as the change that grew the bundle,"
  echo "   with a line saying what grew. An unexplained baseline bump in a diff"
  echo "   is the thing this line exists to make visible."
  echo ""
fi

fail=false

if [ "$total_br" -gt "$FAIL_BYTES" ]; then
  echo "❌ First load is over the product budget: ${total_br} > ${FAIL_BYTES} bytes br4."
  echo "   There is no Cloudflare limit here — the limit is an attendee on venue"
  echo "   mobile data who cannot see their ticket. ${CEILING_BYTES} bytes is"
  echo "   roughly 5.6 s of download on a 3 Mbps connection before anything renders."
  fail=true
elif [ "$total_br" -gt "$WARN_BYTES" ]; then
  echo "⚠️  First load is past the warn line: ${total_br} > ${WARN_BYTES} bytes br4."
  echo "   Still shippable. Stop adding to the frontend and start asking what can"
  echo "   be split out of the first load — all 22 stylesheets are render-blocking"
  echo "   today, including the admin, scanner and dashboard sheets that an"
  echo "   attendee viewing a ticket never uses."
fi

if [ "$delta" -gt "$MAX_GROWTH_BYTES" ]; then
  echo "❌ First load grew ${delta} bytes over the baseline (limit ${MAX_GROWTH_BYTES})."
  echo "   The per-file table above names the culprit. Usual causes:"
  echo "     • a new crate in frontend-leptos (check the diff in its Cargo.lock)"
  echo "     • a dependency added WITHOUT default-features = false"
  echo "     • an image/QR/crypto decoder — these are the big ones (.issues/134)"
  echo ""
  echo "   If the growth is intentional, re-run with --update-baseline and commit"
  echo "   the new baseline in the SAME commit, with a line saying what grew."
  fail=true
elif [ "$delta" -gt "$WARN_GROWTH_BYTES" ]; then
  echo "⚠️  First load grew ${delta} bytes over the baseline (warn at ${WARN_GROWTH_BYTES})."
  echo "   Under the fail line, but large enough to be deliberate rather than drift."
fi

if [ "$fail" = true ]; then
  exit 1
fi

if [ "$delta" -le "$WARN_GROWTH_BYTES" ] && [ "$total_br" -le "$WARN_BYTES" ]; then
  echo "✅ Frontend first load is within budget."
  printf '   Headroom to the product budget: %s bytes br4.\n' \
    "$(( CEILING_BYTES - total_br ))"
fi
exit 0
