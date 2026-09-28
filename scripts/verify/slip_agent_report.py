#!/usr/bin/env python3
"""How the slip agent's proposals compare with the organizer's decisions.

Plan 033 W1/W4. The slip agent runs in shadow mode: every uploaded THB slip
gets a `slip_proposals` row (verdict `accepted` / `needs_review` / `rejected`),
and the organizer's approve or reject in `slip_verify.rs` stays the only
decision. This report is the number the plan publishes after RTM #6.

The organizer's decision is read from `thb_deposits` (rejects keep the row):
  approved   verified = 1
  rejected   verified = 0 and verified_at set
  undecided  verified_at NULL
A decision whose `verified_at` is within 60 s of `uploaded_at` is flagged as
made at upload: the admin upload with `auto_verify` records a slip the
organizer already checked, before any proposal could be read. It is still a
human decision, and it is counted separately so the reader can tell.

A **false accept** (the agent said `accepted`, the organizer rejected) must be
0. Exit 1 if there is any, so the number cannot be published without seeing it.

Read-only: one SELECT through `wrangler d1 execute`.

Usage:
    python3 scripts/verify/slip_agent_report.py                     # prod, all events
    python3 scripts/verify/slip_agent_report.py --event <event_id>
    python3 scripts/verify/slip_agent_report.py --staging
    python3 scripts/verify/slip_agent_report.py --self-test

Exit: 0 no false accepts, 1 false accepts found (or self-test failed), 2 error.
"""

import argparse
import json
import sqlite3
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKER = ROOT / "worker"

# `?1` is the optional event filter; NULL means every event.
QUERY = """
SELECT p.event_id AS event_id,
       p.source AS source,
       p.verdict AS verdict,
       CASE
         WHEN d.attendee_id IS NULL THEN 'no_deposit'
         WHEN d.verified = 1 THEN 'approved'
         WHEN d.verified_at IS NOT NULL THEN 'rejected'
         ELSE 'undecided'
       END AS decision,
       CASE
         WHEN d.verified_at IS NOT NULL AND d.uploaded_at IS NOT NULL
          AND abs(julianday(d.verified_at) - julianday(d.uploaded_at)) * 86400 < 60
         THEN 1 ELSE 0
       END AS decided_at_upload
FROM slip_proposals p
LEFT JOIN thb_deposits d
       ON d.event_id = p.event_id AND d.attendee_id = p.attendee_id
WHERE ?1 IS NULL OR p.event_id = ?1
ORDER BY p.event_id, p.created_at
"""


def summarize(rows: list[dict]) -> dict:
    """Counts the report prints. Pure, so the self-test can pin it."""
    matrix = Counter((r["verdict"], r["decision"]) for r in rows)
    decided = [r for r in rows if r["decision"] in ("approved", "rejected")]
    committed = [r for r in decided if r["verdict"] in ("accepted", "rejected")]
    agree = sum(
        1
        for r in committed
        if (r["verdict"], r["decision"]) in (("accepted", "approved"), ("rejected", "rejected"))
    )
    return {
        "proposals": len(rows),
        "decided": len(decided),
        "decided_at_upload": sum(r["decided_at_upload"] for r in decided),
        "undecided": sum(1 for r in rows if r["decision"] == "undecided"),
        "no_deposit": sum(1 for r in rows if r["decision"] == "no_deposit"),
        "by_source": dict(Counter(r["source"] for r in rows)),
        "matrix": {f"{v} / {d}": n for (v, d), n in sorted(matrix.items())},
        # Decided slips the agent committed on (not needs_review), and how many it got right.
        "committed": len(committed),
        "agree": agree,
        "deferred_to_human": sum(1 for r in decided if r["verdict"] == "needs_review"),
        "false_accepts": matrix[("accepted", "rejected")],
        "false_rejects": matrix[("rejected", "approved")],
    }


def render(summary: dict, label: str) -> str:
    s = summary
    rate = f"{s['agree']}/{s['committed']}" + (f" ({100 * s['agree'] / s['committed']:.0f}%)" if s["committed"] else "")
    lines = [
        f"slip agent vs organizer — {label}",
        f"  proposals: {s['proposals']}  (by source: {s['by_source']})",
        f"  organizer decided: {s['decided']}  (of which at upload: {s['decided_at_upload']}),"
        f" undecided: {s['undecided']}, deposit row gone: {s['no_deposit']}",
        f"  agent committed on {s['committed']} decided slips; agreed with the organizer on {rate}",
        f"  deferred to the organizer (needs_review): {s['deferred_to_human']}",
        f"  false rejects (agent rejected, organizer approved): {s['false_rejects']}",
        f"  {'❌' if s['false_accepts'] else '✅'} false accepts (agent accepted, organizer rejected): {s['false_accepts']}",
        "  verdict / decision:",
    ]
    lines += [f"    {k}: {n}" for k, n in s["matrix"].items()] or ["    (none)"]
    return "\n".join(lines)


def fetch_remote(staging: bool, event: str | None) -> list[dict]:
    sql = QUERY.replace("?1", "NULL" if event is None else "'" + event.replace("'", "''") + "'")
    cmd = ["npx", "wrangler", "d1", "execute", "bethere-db-staging" if staging else "bethere-db", "--remote", "--json", "--command", " ".join(sql.split())]
    if staging:
        cmd += ["--env", "staging"]
    out = subprocess.run(cmd, cwd=WORKER, capture_output=True, text=True)
    text = out.stdout
    start = text.find("[")
    if out.returncode != 0 or start < 0:
        raise RuntimeError(f"wrangler d1 execute failed: {out.stderr.strip()[-300:]}")
    return json.loads(text[start:])[0]["results"]


def self_test() -> int:
    """Run QUERY on SQLite built from the migrations, with a known set of rows."""
    db = sqlite3.connect(":memory:")
    db.row_factory = sqlite3.Row
    for migration in sorted((WORKER / "migrations").glob("*.sql")):
        db.executescript(migration.read_text())
    t0, t_late, t_now = "2026-10-04T06:00:00.000+00:00", "2026-10-04T09:00:00.000+00:00", "2026-10-04T06:00:30.000+00:00"
    # (attendee, verdict, source, verified, verified_at)
    fixtures = [
        ("a1", "accepted", "qr", 1, t_late),        # agree
        ("a2", "rejected", "qr", 0, t_late),        # agree (reject kept the row)
        ("a3", "needs_review", "qr", 1, t_late),    # deferred to the organizer
        ("a4", "accepted", "vision", 0, t_late),    # FALSE ACCEPT
        ("a5", "rejected", "qr", 1, t_late),        # false reject
        ("a6", "accepted", "qr", 1, t_now),         # agree, decided at upload
        ("a7", "needs_review", "qr", 0, None),      # undecided
    ]
    for aid, verdict, source, verified, verified_at in fixtures:
        db.execute(
            "INSERT INTO thb_deposits (attendee_id, event_id, amount_thb, uploaded_at, verified, verified_at) VALUES (?,?,?,?,?,?)",
            (aid, "ev", 500, t0, verified, verified_at),
        )
        db.execute(
            "INSERT INTO slip_proposals (event_id, attendee_id, source, checks, verdict, created_at) VALUES (?,?,?,?,?,?)",
            ("ev", aid, source, "[]", verdict, t0),
        )
    # A proposal whose deposit row is gone, and one on another event.
    db.execute("INSERT INTO slip_proposals (event_id, attendee_id, source, checks, verdict, created_at) VALUES ('ev','a8','qr','[]','accepted',?)", (t0,))
    db.execute("INSERT INTO slip_proposals (event_id, attendee_id, source, checks, verdict, created_at) VALUES ('other','b1','qr','[]','accepted',?)", (t0,))

    rows = [dict(r) for r in db.execute(QUERY, ("ev",))]
    got = summarize(rows)
    want = {
        "proposals": 8, "decided": 6, "decided_at_upload": 1, "undecided": 1, "no_deposit": 1,
        "committed": 5, "agree": 3, "deferred_to_human": 1, "false_accepts": 1, "false_rejects": 1,
    }
    failures = [f"{k}: got {got[k]}, want {v}" for k, v in want.items() if got[k] != v]
    all_events = summarize([dict(r) for r in db.execute(QUERY, (None,))])
    if all_events["proposals"] != 9:
        failures.append(f"unfiltered proposals: got {all_events['proposals']}, want 9")
    print(render(got, "self-test fixture"))
    for f in failures:
        print(f"❌ {f}")
    print(f"\n{'❌' if failures else '✅'} self-test: {len(want) + 1 - len(failures)}/{len(want) + 1} checks")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--staging", action="store_true", help="read the staging D1 instead of prod")
    parser.add_argument("--event", help="only this event id")
    parser.add_argument("--self-test", action="store_true", help="prove the query and the false-accept exit on fixtures")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    try:
        rows = fetch_remote(args.staging, args.event)
    except (RuntimeError, json.JSONDecodeError, KeyError, IndexError) as e:
        print(f"❌ {e}", file=sys.stderr)
        return 2
    summary = summarize(rows)
    print(render(summary, f"{'staging' if args.staging else 'production'}{' / ' + args.event if args.event else ''}"))
    return 1 if summary["false_accepts"] else 0


if __name__ == "__main__":
    sys.exit(main())
