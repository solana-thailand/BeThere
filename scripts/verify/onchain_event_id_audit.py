#!/usr/bin/env python3
"""Audit — and recover — `events.on_chain_event_id` values corrupted by SQLite.

Issue 085. `on_chain_event_id` is a **u64** on-chain, but SQLite integers are
signed 64-bit. `db/events.rs` interpolates the value into the SQL literal, so a
value above `i64::MAX` (9223372036854775807) is not an integer literal SQLite
can store — it silently falls back to REAL and the low digits are gone.

Roughly half of all randomly generated u64 ids exceed `i64::MAX`, so roughly
half of all escrows are affected.

This matters because **every** escrow transaction builder re-derives the PDA
from this id (`solana_escrow/tx_builders/mod.rs::EscrowCtx::resolve`), never
from the stored `escrow_address`. A corrupted id means deposit, refund, close
and rollover all address a PDA that does not exist.

Recovery is possible because `escrow_address` is stored separately as TEXT and
is correct. The true id is within a few float64 ULPs of the stored value, so a
short search around it — checking which candidate derives to the known address
— recovers it exactly.

Usage:
    python3 scripts/verify/onchain_event_id_audit.py --db bethere-db
    python3 scripts/verify/onchain_event_id_audit.py --db bethere-db --recover

Requires `solders` (pip install solders) and a wrangler login.
"""

from __future__ import annotations

import argparse
import json
import math
import struct
import subprocess
import sys

ESCROW_PROGRAM = "C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T"
# float64 has a 53-bit significand; searching 4 ULPs either side is far more
# than the rounding can move a value, and costs only a few thousand derivations.
ULP_SEARCH_RADIUS = 4


def query(db: str, sql: str) -> list[dict]:
    """Run a read-only statement against a remote D1 database."""
    out = subprocess.run(
        ["npx", "wrangler", "d1", "execute", db, "--remote", "--json", "--command", sql],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return json.loads(out[out.index("[") :])[0]["results"]


def recover(stored: float, organizer: str, escrow_address: str) -> int | None:
    """Find the u64 that derives to `escrow_address`, near the corrupted float."""
    from solders.pubkey import Pubkey

    program = Pubkey.from_string(ESCROW_PROGRAM)
    org = Pubkey.from_string(organizer)
    centre = int(stored)
    spacing = 2 ** (math.frexp(stored)[1] - 53)

    for delta in range(-ULP_SEARCH_RADIUS * spacing, ULP_SEARCH_RADIUS * spacing + 1):
        candidate = centre + delta
        if not 0 <= candidate <= 2**64 - 1:
            continue
        pda, _ = Pubkey.find_program_address(
            [b"escrow", bytes(org), struct.pack("<Q", candidate)], program
        )
        if str(pda) == escrow_address:
            return candidate
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", required=True, help="D1 database name")
    parser.add_argument(
        "--recover",
        action="store_true",
        help="Search for the true id of each corrupted row (slower)",
    )
    args = parser.parse_args()

    rows = query(
        args.db,
        "SELECT id, typeof(on_chain_event_id) AS kind, "
        "CAST(on_chain_event_id AS TEXT) AS value, organizer_wallet, "
        "escrow_address, escrow_status FROM events "
        "WHERE on_chain_event_id IS NOT NULL AND on_chain_event_id != 0;",
    )

    corrupted = [r for r in rows if r["kind"] != "integer"]
    print(f"{args.db}: {len(rows)} event(s) with an on-chain id, "
          f"{len(corrupted)} stored as REAL (corrupted)")

    if not corrupted:
        print("✅ No corruption found.")
        return 0

    exit_code = 1
    for row in corrupted:
        print(f"\n  event          {row['id']}")
        print(f"  escrow_status  {row['escrow_status']}")
        print(f"  stored value   {row['value']}  (REAL — low digits lost)")
        print(f"  escrow_address {row['escrow_address']}")
        if not args.recover:
            continue
        if not row["escrow_address"] or not row["organizer_wallet"]:
            print("  recovery       IMPOSSIBLE — no escrow address to match against")
            continue
        found = recover(float(row["value"]), row["organizer_wallet"], row["escrow_address"])
        if found is None:
            print(f"  recovery       NOT FOUND within {ULP_SEARCH_RADIUS} ULP")
        else:
            print(f"  recovery       ✅ {found}  (derives to the stored address)")

    print(
        "\n❌ Corrupted ids found. Every escrow tx builder derives the PDA from "
        "this value, so deposit/refund/close are broken for these events."
    )
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
