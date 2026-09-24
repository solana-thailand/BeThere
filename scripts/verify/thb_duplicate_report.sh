#!/usr/bin/env bash
# Which THB deposits would collide under a UNIQUE (event_id, attendee_id).
#
# `.issues/127`: `thb_deposits` has no such constraint, so one attendee can hold
# two deposit rows for one event. The refund path pays per row, which is how
# ฿700 went out twice. The fix is a table rebuild (SQLite cannot add a
# constraint in place), and a rebuild of the live deposits table days before an
# event is the wrong trade — so migration 0048 is written and deliberately NOT
# applied until after RTM #6 on 2026-09-27.
#
# This is the read-only half: it answers "which rows, exactly" so the dedupe
# decision is made against the real list rather than in the abstract. It runs
# SELECTs only, and never writes.
#
# Usage:
#   bash scripts/verify/thb_duplicate_report.sh              # production
#   bash scripts/verify/thb_duplicate_report.sh --staging
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../../worker"

ENV_ARGS=()
LABEL="production"
if [ "${1:-}" = "--staging" ]; then
  ENV_ARGS=("--env" "staging")
  LABEL="staging"
fi

# Yarn PnP breaks wrangler's esbuild; deploy.sh does this itself, a direct
# wrangler call has to.
PNP_MOVED=false
if [ -f "$HOME/.pnp.cjs" ]; then
  mv "$HOME/.pnp.cjs" "$HOME/.pnp.cjs.bak"
  PNP_MOVED=true
fi
restore_pnp() {
  if [ "$PNP_MOVED" = true ] && [ -f "$HOME/.pnp.cjs.bak" ]; then
    mv "$HOME/.pnp.cjs.bak" "$HOME/.pnp.cjs"
  fi
}
trap restore_pnp EXIT

echo "🔍 THB deposit duplicates on ${LABEL} (read-only)"
echo ""

# No names or emails: the answer is "which (event, attendee) pairs and how much
# money", and pulling PII out of production to answer it would be gratuitous.
QUERY="SELECT event_id, attendee_id, COUNT(*) AS rows_held, \
       SUM(amount_thb) AS total_thb, \
       SUM(CASE WHEN refunded = 1 THEN 1 ELSE 0 END) AS refunded_rows, \
       SUM(CASE WHEN held_as_credit = 1 THEN 1 ELSE 0 END) AS held_rows \
FROM thb_deposits \
GROUP BY event_id, attendee_id \
HAVING COUNT(*) > 1 \
ORDER BY rows_held DESC, total_thb DESC"

npx wrangler d1 execute DB "${ENV_ARGS[@]+"${ENV_ARGS[@]}"}" --remote --json --command "$QUERY" \
  | python3 -c '
import json, sys

payload = json.load(sys.stdin)
rows = payload[0]["results"]

if not rows:
    print("  No (event_id, attendee_id) pair holds more than one deposit row.")
    print("  The UNIQUE in migration 0048 would apply cleanly, with no dedupe.")
    sys.exit(0)

head = "  {:<26} {:<28} {:>5} {:>8} {:>9} {:>5}"
print("  {} colliding pair(s):".format(len(rows)))
print()
print(head.format("event_id", "attendee_id", "rows", "total", "refunded", "held"))
print(head.format("-" * 26, "-" * 28, "-" * 5, "-" * 8, "-" * 9, "-" * 5))

extra_money = 0
for r in rows:
    print(head.format(r["event_id"], r["attendee_id"], r["rows_held"],
                      r["total_thb"], r["refunded_rows"], r["held_rows"]))
    if r["refunded_rows"] > 1:
        per_row = r["total_thb"] // r["rows_held"]
        extra_money += per_row * (r["refunded_rows"] - 1)

print()
print("  Rows to remove before 0048 can apply: {}".format(
    sum(r["rows_held"] - 1 for r in rows)))
if extra_money:
    print("  Money already paid out more than once: {} THB".format(extra_money))
else:
    print("  No pair has been refunded more than once.")
print()
print("  Deciding which row to keep is NOT mechanical: keep the one whose slip")
print("  the organizer actually accepted. Do it by hand, with the list above.")
'
