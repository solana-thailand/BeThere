#!/usr/bin/env python3
"""THB deposits still owed back, and whether the 7-day promise holds.

Owner decision D3 (2026-09-28, `docs/deposit-commitment-model.md` §5): THB
comes back within 7 days after the event. The listing and the event page say
so. This report lists what is still owed and flags anything past the window.

"Owed" is exactly the admin refund queue (`slip_list.rs::refund_queue_handler`):
verified, not refunded, not held as credit, and cash. Cash uses the same
classifier as `ThbDeposit::source()` for rows whose `deposit_source` is NULL
(the CASE is migration 0047's backfill, pinned to `source()` by
`worker/tests/deposit_source_guards.rs`). The clock starts at the event's
`event_end_ms`. Nothing is forfeited (D1), so attendance does not matter.

Events that ended before the promise (PROMISE_FROM) are reported as backlog
and do not fail the run; an overdue refund on a later event exits 1.

Read-only: one SELECT through `wrangler d1 execute`. Prints attendee ids
(internal, Issue 070) and amounts, never names, emails or bank details.

Usage:
    python3 scripts/verify/refund_window_report.py              # prod
    python3 scripts/verify/refund_window_report.py --staging
    python3 scripts/verify/refund_window_report.py --detail     # one line per deposit
    python3 scripts/verify/refund_window_report.py --self-test

Exit: 0 nothing overdue since the promise, 1 overdue (or self-test failed), 2 error.
"""

import argparse
import sqlite3
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import d1_remote

WORKER = Path(__file__).resolve().parents[2] / "worker"
WINDOW_DAYS = 7
DAY_MS = 86_400_000
# D3 was decided on this day; events that ended earlier are backlog.
PROMISE_FROM_MS = int(datetime(2026, 9, 28, tzinfo=timezone.utc).timestamp() * 1000)

# `?1` = now in epoch ms.
QUERY = """
SELECT d.event_id AS event_id,
       COALESCE(e.slug, d.event_id) AS slug,
       d.attendee_id AS attendee_id,
       d.amount_thb AS amount_thb,
       e.event_end_ms AS event_end_ms,
       (?1 - e.event_end_ms) AS ms_since_end
FROM thb_deposits d
LEFT JOIN events e ON e.id = d.event_id
WHERE d.verified = 1
  AND COALESCE(d.refunded, 0) = 0
  AND COALESCE(d.held_as_credit, 0) = 0
  AND COALESCE(d.deposit_source, CASE
        WHEN d.verified_by = 'SYSTEM_ROLLING_CREDIT' OR d.slip_url = 'ROLLING_CREDIT_AUTO_APPLIED' THEN 'credit'
        WHEN d.verified_by = 'SYSTEM_STAFF_WAIVE' OR d.slip_url = 'STAFF_COMP_WAIVED' OR d.amount_thb = 0 THEN 'comp'
        ELSE 'cash'
      END) = 'cash'
ORDER BY e.event_end_ms, d.event_id, d.attendee_id
"""


def classify(row: dict) -> str:
    end, since = row["event_end_ms"], row["ms_since_end"]
    match (end, since):
        case (None, _) | (0, _):
            return "unknown_end"
        case (_, s) if s < 0:
            return "not_due"
        case (_, s) if s <= WINDOW_DAYS * DAY_MS:
            return "within_window"
        case _ if end < PROMISE_FROM_MS:
            return "backlog"
        case _:
            return "overdue"


def summarize(rows: list[dict]) -> dict:
    by_event: dict[str, dict] = {}
    for r in rows:
        e = by_event.setdefault(r["slug"], {"owed": 0, "thb": 0, "overdue": 0, "backlog": 0, "max_days": 0.0, "state": set()})
        state = classify(r)
        e["owed"] += 1
        e["thb"] += r["amount_thb"] or 0
        e["state"].add(state)
        e["overdue"] += state == "overdue"
        e["backlog"] += state == "backlog"
        if r["ms_since_end"] is not None:
            e["max_days"] = max(e["max_days"], r["ms_since_end"] / DAY_MS)
    return {
        "owed": len(rows),
        "thb": sum(r["amount_thb"] or 0 for r in rows),
        "overdue": sum(e["overdue"] for e in by_event.values()),
        "backlog": sum(e["backlog"] for e in by_event.values()),
        "by_event": by_event,
    }


def render(s: dict, rows: list[dict], detail: bool, label: str) -> str:
    lines = [
        f"THB refunds owed — {label} (window {WINDOW_DAYS} days after the event, D3)",
        f"  owed: {s['owed']} deposits, ฿{s['thb']:,}",
        f"  {'❌' if s['overdue'] else '✅'} overdue since the promise (events ended on/after 2026-09-28): {s['overdue']}",
        f"  backlog from before the promise (not failing): {s['backlog']}",
    ]
    # Slugs, not ids: an id is kept when an event is copied, so `…-5-bangkok-copy` can be RTM #6.
    for event_id, e in sorted(s["by_event"].items(), key=lambda kv: -kv[1]["max_days"]):
        states = ",".join(sorted(e["state"]))
        lines.append(f"    {event_id}: {e['owed']} owed, ฿{e['thb']:,}, oldest {e['max_days']:.0f} days after the end [{states}]")
    if detail:
        lines.append("  detail:")
        lines += [
            f"    {r['slug']} {r['attendee_id']} ฿{r['amount_thb']} "
            f"{'' if r['ms_since_end'] is None else f'{r['ms_since_end'] / DAY_MS:.1f}d'} {classify(r)}"
            for r in rows
        ]
    return "\n".join(lines)


def self_test() -> int:
    db = sqlite3.connect(":memory:")
    db.row_factory = sqlite3.Row
    for migration in sorted((WORKER / "migrations").glob("*.sql")):
        db.executescript(migration.read_text())
    now = PROMISE_FROM_MS + 30 * DAY_MS
    events = {
        "old": PROMISE_FROM_MS - 20 * DAY_MS,   # ended before the promise
        "late": now - 10 * DAY_MS,              # ended after it, 10 days ago
        "recent": now - 3 * DAY_MS,             # inside the window
        "future": now + 5 * DAY_MS,             # not ended
    }
    for eid, end in events.items():
        db.execute(
            "INSERT INTO events (id, name, slug, event_start_ms, event_end_ms, created_at) VALUES (?,?,?,?,?,?)",
            (eid, eid, eid, end - DAY_MS, end, "2026"),
        )
    # (event, attendee, amount, verified, refunded, held, source, verified_by)
    deposits = [
        ("old", "o1", 500, 1, 0, 0, "cash", None),        # backlog
        ("late", "l1", 500, 1, 0, 0, "cash", None),       # OVERDUE
        ("late", "l2", 500, 1, 1, 0, "cash", None),       # refunded: not owed
        ("late", "l3", 500, 1, 0, 1, "cash", None),       # held as credit: not owed
        ("late", "l4", 500, 1, 0, 0, None, "SYSTEM_ROLLING_CREDIT"),  # legacy credit: not owed
        ("late", "l5", 0, 1, 0, 0, None, "admin@x"),       # legacy ฿0 comp: not owed
        ("late", "l6", 500, 0, 0, 0, "cash", None),       # never verified: not owed
        ("late", "l7", 700, 1, 0, 0, None, "admin@x"),     # legacy NULL source, cash: OVERDUE
        ("recent", "r1", 500, 1, 0, 0, "cash", None),     # within window
        ("future", "f1", 500, 1, 0, 0, "cash", None),     # not due
        ("ghost", "g1", 500, 1, 0, 0, "cash", None),      # event row missing: unknown end
    ]
    for eid, aid, amt, ver, ref, held, src, vby in deposits:
        db.execute(
            "INSERT INTO thb_deposits (attendee_id, event_id, amount_thb, uploaded_at, verified, verified_by, refunded, held_as_credit, deposit_source)"
            " VALUES (?,?,?,?,?,?,?,?,?)",
            (aid, eid, amt, "2026", ver, vby, ref, held, src),
        )
    rows = [dict(r) for r in db.execute(QUERY, (now,))]
    s = summarize(rows)
    got = {r["attendee_id"]: classify(r) for r in rows}
    want = {"o1": "backlog", "l1": "overdue", "l7": "overdue", "r1": "within_window", "f1": "not_due", "g1": "unknown_end"}
    failures = [f"owed set/classes: got {got}, want {want}"] if got != want else []
    if (s["overdue"], s["backlog"], s["thb"]) != (2, 1, 3200):
        failures.append(f"totals: got overdue={s['overdue']} backlog={s['backlog']} thb={s['thb']}, want 2, 1, 3200")
    print(render(s, rows, True, "self-test fixture"))
    for f in failures:
        print(f"❌ {f}")
    print(f"\n{'❌' if failures else '✅'} self-test: {2 - len(failures)}/2 checks")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--staging", action="store_true", help="read the staging D1 instead of prod")
    parser.add_argument("--detail", action="store_true", help="one line per owed deposit")
    parser.add_argument("--self-test", action="store_true", help="prove the owed set and the overdue exit on fixtures")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    now_ms = int(time.time() * 1000)
    try:
        rows = d1_remote.select(QUERY.replace("?1", str(now_ms)), args.staging)
    except (RuntimeError, ValueError, KeyError, IndexError) as e:
        print(f"❌ {e}", file=sys.stderr)
        return 2
    s = summarize(rows)
    print(render(s, rows, args.detail, "staging" if args.staging else "production"))
    return 1 if s["overdue"] else 0


if __name__ == "__main__":
    sys.exit(main())
