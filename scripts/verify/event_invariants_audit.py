#!/usr/bin/env python3
"""Audit event rows for field combinations that should never coexist.

Issue 089. Almost every defect found on 2026-09-13 had the same shape: two
columns that contradict each other, and nothing checking. They were found by a
person noticing something looked wrong — sometimes months later, once by the
DevRel team reading the database from outside, once by the repo owner seeing
their own registration history go blank.

Each rule below is a bug that actually happened. The point is that the next one
in this family goes red in CI instead of waiting to be noticed.

Usage:
    python3 scripts/verify/event_invariants_audit.py --db bethere-db
    python3 scripts/verify/event_invariants_audit.py --db bethere-db-staging

Exits non-zero if any rule is violated. Read-only — it never writes.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from dataclasses import dataclass


@dataclass(frozen=True)
class Rule:
    """One thing that must never be true of an event row."""

    name: str
    #: Why it matters, in terms of what a human sees when it is violated.
    consequence: str
    #: SQL returning the offending rows. Must select `id`.
    sql: str
    #: False for rules that describe a judgement call rather than a defect.
    blocking: bool = True


RULES: list[Rule] = [
    Rule(
        name="past event still accepting registration",
        consequence=(
            "a finished event keeps taking sign-ups, and post-event "
            "registration cannot be opened for it (it requires 'completed')"
        ),
        sql=(
            "SELECT id FROM events WHERE status = 'active' "
            "AND event_end_ms > 0 AND event_end_ms < {now};"
        ),
    ),
    Rule(
        name="completed event with post-event registration closed",
        consequence=(
            "recap QR codes pointing at /events/{slug}/post-event-register "
            "return 409 — the link exists and leads nowhere"
        ),
        sql=(
            "SELECT id FROM events WHERE status = 'completed' "
            "AND post_event_registration_open = 0;"
        ),
        blocking=False,
    ),
    Rule(
        name="draft event with an on-chain escrow",
        consequence=(
            "the public page 404s while real money sits in an initialized "
            "escrow — how islanddao-v4-demo ended up invisible"
        ),
        sql=(
            "SELECT id FROM events WHERE status = 'draft' "
            "AND escrow_status IN ('initialized','deactivated');"
        ),
    ),
    Rule(
        name="draft event with registered attendees",
        consequence=(
            "people registered for an event the product now says is unpublished"
        ),
        sql=(
            "SELECT e.id FROM events e WHERE e.status = 'draft' "
            "AND EXISTS (SELECT 1 FROM attendees a WHERE a.event_id = e.id);"
        ),
    ),
    Rule(
        name="quiz enabled with no quiz configured",
        consequence=(
            "the event advertises a quiz that does not exist; found by DevRel "
            "reading the database from outside, on 12 events"
        ),
        sql=(
            "SELECT e.id FROM events e WHERE e.quiz_enabled = 1 "
            "AND NOT EXISTS (SELECT 1 FROM quiz_configs q WHERE q.event_id = e.id);"
        ),
    ),
    Rule(
        name="escrow initialized without an organizer wallet",
        consequence=(
            "the escrow PDA cannot be re-derived — every deposit, refund and "
            "close for the event fails"
        ),
        sql=(
            "SELECT id FROM events WHERE escrow_status = 'initialized' "
            "AND (organizer_wallet IS NULL OR organizer_wallet = '');"
        ),
    ),
    Rule(
        name="escrow address recorded without an on-chain id",
        consequence=(
            "same as above — the address is stored but the seed that derives "
            "it is missing (.issues/085)"
        ),
        sql=(
            "SELECT id FROM events WHERE escrow_address <> '' "
            "AND (on_chain_event_id IS NULL OR on_chain_event_id = 0);"
        ),
    ),
    Rule(
        name="deposits enabled with no amount set",
        consequence="attendees are asked for a deposit of zero",
        sql=(
            "SELECT id FROM events WHERE deposit_enabled = 1 "
            "AND deposit_amount_usdc = 0 AND deposit_amount_thb = 0;"
        ),
    ),
    Rule(
        name="recap published with no summary row",
        consequence=(
            "the public recap endpoint 404s despite the flag claiming it is live"
        ),
        sql=(
            "SELECT e.id FROM events e WHERE e.recap_published = 1 "
            "AND NOT EXISTS (SELECT 1 FROM event_summaries s WHERE s.event_id = e.id);"
        ),
    ),
    Rule(
        name="post-event registration open on a non-completed event",
        consequence=(
            "the flag is set but the endpoint rejects every caller, because it "
            "requires 'completed'"
        ),
        sql=(
            "SELECT id FROM events WHERE post_event_registration_open = 1 "
            "AND status <> 'completed';"
        ),
    ),
    Rule(
        name="event id disagrees with its slug",
        consequence=(
            "the join key names a different event than the slug does, so every "
            "human reading an id attributes rows to the wrong event "
            "(.issues/079)"
        ),
        sql="SELECT id FROM events WHERE id <> slug;",
        blocking=False,
    ),
]


def query(db: str, sql: str) -> list[dict]:
    out = subprocess.run(
        ["npx", "wrangler", "d1", "execute", db, "--remote", "--json", "--command", sql],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return json.loads(out[out.index("[") :])[0]["results"]


# ── Self-test ────────────────────────────────────────────────────────────────
#
# CI has no useful database to point this at: staging is deliberately full of
# harness fixtures in contradictory states (that is what a fixture is), and
# production findings are often intentional organizer choices. Pointing the
# audit at either would make a permanently red gate that nobody reads.
#
# So CI verifies the *auditor* instead, and humans run the *audit*. Each rule
# is exercised in both directions against an in-memory SQLite: it must fire on
# a row that violates it, and stay silent on one that does not. A rule that can
# only ever pass is not a rule.

SCHEMA = """
CREATE TABLE events (
    id TEXT PRIMARY KEY, slug TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'draft',
    event_end_ms INTEGER NOT NULL DEFAULT 0, post_event_registration_open INTEGER NOT NULL DEFAULT 0,
    escrow_status TEXT NOT NULL DEFAULT 'none', escrow_address TEXT NOT NULL DEFAULT '',
    organizer_wallet TEXT NOT NULL DEFAULT '', on_chain_event_id INTEGER NOT NULL DEFAULT 0,
    quiz_enabled INTEGER NOT NULL DEFAULT 0, deposit_enabled INTEGER NOT NULL DEFAULT 0,
    deposit_amount_usdc INTEGER NOT NULL DEFAULT 0, deposit_amount_thb INTEGER NOT NULL DEFAULT 0,
    recap_published INTEGER NOT NULL DEFAULT 0);
CREATE TABLE attendees (id TEXT PRIMARY KEY, event_id TEXT NOT NULL);
CREATE TABLE quiz_configs (event_id TEXT PRIMARY KEY);
CREATE TABLE event_summaries (event_id TEXT PRIMARY KEY);
"""

#: rule name → (columns that violate it, columns that satisfy it)
SELF_TEST_CASES: dict[str, tuple[dict, dict]] = {
    "past event still accepting registration": (
        {"status": "active", "event_end_ms": 1},
        {"status": "completed", "event_end_ms": 1},
    ),
    "completed event with post-event registration closed": (
        {"status": "completed", "post_event_registration_open": 0},
        {"status": "completed", "post_event_registration_open": 1},
    ),
    "draft event with an on-chain escrow": (
        {"status": "draft", "escrow_status": "initialized"},
        {"status": "completed", "escrow_status": "initialized"},
    ),
    "draft event with registered attendees": (
        {"status": "draft", "_attendee": True},
        {"status": "draft"},
    ),
    "quiz enabled with no quiz configured": (
        {"quiz_enabled": 1},
        {"quiz_enabled": 1, "_quiz": True},
    ),
    "escrow initialized without an organizer wallet": (
        {"escrow_status": "initialized", "organizer_wallet": ""},
        {"escrow_status": "initialized", "organizer_wallet": "Aqdr"},
    ),
    "escrow address recorded without an on-chain id": (
        {"escrow_address": "H8iH", "on_chain_event_id": 0},
        {"escrow_address": "H8iH", "on_chain_event_id": 42},
    ),
    "deposits enabled with no amount set": (
        {"deposit_enabled": 1},
        {"deposit_enabled": 1, "deposit_amount_usdc": 1},
    ),
    "recap published with no summary row": (
        {"recap_published": 1},
        {"recap_published": 1, "_summary": True},
    ),
    "post-event registration open on a non-completed event": (
        {"post_event_registration_open": 1, "status": "active"},
        {"post_event_registration_open": 1, "status": "completed"},
    ),
    "event id disagrees with its slug": (
        {"slug": "something-else"},
        {},
    ),
}


def self_test() -> int:
    """Prove every rule fires on a bad row and stays silent on a good one."""
    import sqlite3

    missing = {r.name for r in RULES} - set(SELF_TEST_CASES)
    if missing:
        print(f"❌ rules with no self-test case: {sorted(missing)}")
        return 1

    now_ms = int(time.time() * 1000)
    failures = 0

    for rule in RULES:
        bad, good = SELF_TEST_CASES[rule.name]
        for label, columns, should_fire in (("violating", bad, True), ("clean", good, False)):
            conn = sqlite3.connect(":memory:")
            conn.executescript(SCHEMA)
            row = dict(columns)
            extras = {k: row.pop(k) for k in list(row) if k.startswith("_")}
            row.setdefault("slug", "ev")
            names = ["id"] + list(row)
            values = ["ev"] + list(row.values())
            conn.execute(
                f"INSERT INTO events ({','.join(names)}) "
                f"VALUES ({','.join('?' * len(names))})",
                values,
            )
            if extras.get("_attendee"):
                conn.execute("INSERT INTO attendees (id,event_id) VALUES ('a','ev')")
            if extras.get("_quiz"):
                conn.execute("INSERT INTO quiz_configs (event_id) VALUES ('ev')")
            if extras.get("_summary"):
                conn.execute("INSERT INTO event_summaries (event_id) VALUES ('ev')")

            fired = bool(conn.execute(rule.sql.format(now=now_ms)).fetchall())
            conn.close()

            if fired != should_fire:
                verb = "did not fire on a violating row" if should_fire else "fired on a clean row"
                print(f"  ❌ {rule.name}: {verb} ({label})")
                failures += 1

    if failures:
        print(f"\n❌ {failures} self-test failure(s) across {len(RULES)} rule(s).")
        return 1
    print(f"✅ All {len(RULES)} rules fire on a violating row and stay silent on a clean one.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", help="D1 database name (omit with --self-test)")
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Verify every rule against an in-memory fixture; needs no database",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Treat advisory rules as failures too",
    )
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if not args.db:
        parser.error("--db is required unless --self-test is given")

    now_ms = int(time.time() * 1000)
    failures = 0
    advisories = 0

    print(f"{args.db}: {len(RULES)} invariant(s)\n")
    for rule in RULES:
        rows = query(args.db, rule.sql.format(now=now_ms))
        if not rows:
            print(f"  ✅ {rule.name}")
            continue

        blocking = rule.blocking or args.strict
        mark = "❌" if blocking else "⚠️ "
        print(f"  {mark} {rule.name}  ({len(rows)} event(s))")
        print(f"       → {rule.consequence}")
        for row in rows[:8]:
            print(f"       · {row['id']}")
        if len(rows) > 8:
            print(f"       · … and {len(rows) - 8} more")

        if blocking:
            failures += 1
        else:
            advisories += 1

    print()
    if failures:
        print(f"❌ {failures} invariant(s) violated, {advisories} advisory.")
        return 1
    if advisories:
        print(f"⚠️  {advisories} advisory finding(s); no blocking violation.")
        return 0
    print("✅ Every invariant holds.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
