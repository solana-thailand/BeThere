#!/usr/bin/env bash
# Re-derive the escrow PDA golden vectors with the Solana CLI, an implementation
# independent of the worker, and compare them with the pinned fixture.
#
#   scripts/verify/golden_vectors_check.sh              # check the fixture
#   scripts/verify/golden_vectors_check.sh --self-test  # prove a wrong pin fails
#
# Exit: 0 all match · 1 mismatch · 2 tooling missing or zero cases checked.
set -euo pipefail

fixture="$(cd "$(dirname "$0")/../.." && pwd)/domain/tests/fixtures/golden_vectors.json"
token_program="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
ata_program="ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"

command -v solana >/dev/null || { echo "solana CLI not found" >&2; exit 2; }
command -v python3 >/dev/null || { echo "python3 not found" >&2; exit 2; }

pda() {
  solana find-program-derived-address "$@" --output json-compact |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["address"])'
}

# Emits: program mint organizer attendee, then one line per case.
# Python, not jq: the u64 ids exceed 2^53.
read_cases() {
  python3 - "$1" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))["escrow_pda"]
print(d["program_id"], d["usdc_mint"], d["organizer"], d["attendee"])
for c in d["cases"]:
    print(c["event_id"], c["escrow"], c["deposit"], c["vault"])
PY
}

check() {
  local file="$1" checked=0 failed=0 program mint organizer attendee
  local event_id escrow deposit vault got
  {
    read -r program mint organizer attendee
    while read -r event_id escrow deposit vault; do
      got="$(pda "$program" string:escrow "pubkey:$organizer" "u64le:$event_id")"
      [[ "$got" == "$escrow" ]] || { echo "❌ escrow $event_id: fixture $escrow, cli $got"; failed=1; }
      got="$(pda "$program" string:deposit "pubkey:$got" "pubkey:$attendee")"
      [[ "$got" == "$deposit" ]] || { echo "❌ deposit $event_id: fixture $deposit, cli $got"; failed=1; }
      got="$(pda "$ata_program" "pubkey:$escrow" "pubkey:$token_program" "pubkey:$mint")"
      [[ "$got" == "$vault" ]] || { echo "❌ vault $event_id: fixture $vault, cli $got"; failed=1; }
      checked=$((checked + 1))
    done
  } < <(read_cases "$file")
  if [[ "$checked" -eq 0 ]]; then
    echo "❌ zero cases checked" >&2
    return 2
  fi
  if [[ "$failed" -ne 0 ]]; then
    return 1
  fi
  echo "✅ $checked escrow cases match the Solana CLI"
}

if [[ "${1:-}" == "--self-test" ]]; then
  tmp="$(mktemp)"
  trap 'rm -f "$tmp"' EXIT
  # Flip the last character of the first escrow address.
  python3 - "$fixture" "$tmp" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
e = d["escrow_pda"]["cases"][0]["escrow"]
d["escrow_pda"]["cases"][0]["escrow"] = e[:-1] + ("A" if e[-1] != "A" else "B")
json.dump(d, open(sys.argv[2], "w"))
PY
  rc=0
  check "$tmp" >/dev/null || rc=$?
  if [[ "$rc" -ne 1 ]]; then
    echo "❌ self-test: a wrong pin exited $rc, expected 1" >&2
    exit 1
  fi
  echo "✅ self-test: a wrong pin fails"
  exit 0
fi

check "$fixture"
