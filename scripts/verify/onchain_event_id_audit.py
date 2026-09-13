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
# float64 holds integers exactly only up to 2**53; above it the JSON read path
# has already rounded the value before Rust ever sees it.
MAX_SAFE_INTEGER = 2**53
I64_MAX = 2**63 - 1
# float64 has a 53-bit significand; searching 4 ULPs either side is far more
# than the rounding can move a value, and costs only a few thousand derivations.
ULP_SEARCH_RADIUS = 4


def derive_on_chain_event_id(event_id: str) -> int:
    """FNV-1a 64-bit, mirroring `worker/src/handlers/deposit/mod.rs`.

    The id is a pure function of the event's string id, so the true value is
    always recomputable — the column is only a cache of this.
    """
    h = 0xCBF29CE484222325
    for byte in event_id.encode():
        h ^= byte
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h or 1


def derives_to(event_id_u64: int, organizer: str, escrow_address: str) -> bool:
    """Whether `event_id_u64` produces the escrow PDA we already know about."""
    from solders.pubkey import Pubkey

    pda, _ = Pubkey.find_program_address(
        [b"escrow", bytes(Pubkey.from_string(organizer)), struct.pack("<Q", event_id_u64)],
        Pubkey.from_string(ESCROW_PROGRAM),
    )
    return str(pda) == escrow_address


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


def execute(db: str, sql: str) -> None:
    """Run a write against a remote D1 database."""
    subprocess.run(
        ["npx", "wrangler", "d1", "execute", db, "--remote", "--command", sql],
        capture_output=True,
        text=True,
        check=True,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", required=True, help="D1 database name")
    parser.add_argument(
        "--recover",
        action="store_true",
        help="Report the true id of each corrupted row without writing",
    )
    parser.add_argument(
        "--repair",
        action="store_true",
        help="Write the recovered id into on_chain_event_id_text (migration "
        "0034). Refuses any value that does not derive to the stored "
        "escrow_address.",
    )
    args = parser.parse_args()

    # Migration 0034 may not have run yet. Detect it rather than failing with
    # a SQL error the operator has to decode.
    columns = {c["name"] for c in query(args.db, "PRAGMA table_info(events);")}
    has_exact = "on_chain_event_id_text" in columns
    if not has_exact:
        print(
            f"⚠️  {args.db}: migration 0034 has not been applied — the exact "
            "column does not exist yet.\n"
            "   Reporting what is stored; --repair needs the migration first.\n"
        )

    exact_select = (
        "COALESCE(on_chain_event_id_text, '')" if has_exact else "''"
    )
    rows = query(
        args.db,
        "SELECT id, typeof(on_chain_event_id) AS kind, "
        "CAST(on_chain_event_id AS TEXT) AS value, organizer_wallet, "
        "escrow_address, escrow_status, "
        f"{exact_select} AS exact FROM events "
        "WHERE on_chain_event_id IS NOT NULL AND on_chain_event_id != 0;",
    )

    # Two independent defects, so two independent verdicts per row:
    #   stored as REAL      → the write destroyed it (above i64::MAX)
    #   above 2**53         → the read rounds it, even if the write was fine
    # A row is healthy only when the exact TEXT column holds the true value.
    print(f"{args.db}: {len(rows)} event(s) with an on-chain id\n")

    unhealthy = 0
    for row in rows:
        truth = derive_on_chain_event_id(row["id"])
        stored_real = row["kind"] != "integer"
        unsafe_read = truth > MAX_SAFE_INTEGER
        exact_ok = row["exact"] == str(truth)

        verified = None
        if row["escrow_address"] and row["organizer_wallet"]:
            verified = derives_to(truth, row["organizer_wallet"], row["escrow_address"])

        healthy = exact_ok and verified is not False
        if not healthy:
            unhealthy += 1

        mark = "✅" if healthy else "❌"
        print(f"  {mark} {row['id']}  ({row['escrow_status']})")
        print(f"       stored numeric  {row['value']}"
              f"{'   [REAL — destroyed on write]' if stored_real else ''}")
        print(f"       true (FNV-1a)   {truth}"
              f"{'   [above 2**53 — rounded on read]' if unsafe_read else ''}")
        print(f"       exact column    {row['exact'] or '(empty — run migration 0034)'}")
        if verified is None:
            print("       PDA check       skipped — no escrow address on record")
        elif verified:
            print("       PDA check       derives to the stored escrow_address ✓")
        else:
            print("       PDA check       ⚠️  does NOT derive to the stored address —"
                  " the id was overridden, not derived; do NOT auto-repair")

        if args.repair and not has_exact:
            print("       repair          BLOCKED — apply migration 0034 first")
        elif args.repair and not exact_ok:
            if verified is False:
                print("       repair          REFUSED — would not match the live escrow")
                continue
            execute(
                args.db,
                f"UPDATE events SET on_chain_event_id_text = '{truth}' "
                f"WHERE id = '{row['id']}';",
            )
            print(f"       repair          ✅ wrote {truth}")
            unhealthy -= 1
        print()

    if unhealthy == 0:
        print("✅ Every id is exact and derives to its escrow.")
        return 0

    print(
        f"❌ {unhealthy} event(s) unhealthy. Every escrow tx builder derives the "
        "PDA from this value, so deposit/refund/close are broken for them.\n"
        "   Re-run with --repair once migration 0034 has been applied."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
