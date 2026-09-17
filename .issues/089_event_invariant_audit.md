# 089 — An audit for event states that should never coexist

**Status:** implemented 2026-09-13
**Asked for by:** the repo owner — *"มีวิธีพัฒนาเพื่อไม่ให้เกิด wrong flow ได้ไหม"*
**Severity:** preventative

## Why

Nearly every defect found on 2026-09-13 had one shape: **two columns that
contradict each other, and nothing checking.**

| what | how it was found |
|---|---|
| past events still `active` | needed 12 manual status changes |
| `completed` but `post_event_registration_open = 0` | 13 recap QR codes led to a 409 |
| `draft` with an initialized escrow and a real deposit | spotted by eye, after the fact |
| `quiz_enabled = 1` with no quiz config, ×12 | **DevRel, reading our database from outside** |
| `on_chain_event_id` not matching its escrow | only when a deposit failed on-chain (`.issues/085`) |
| `id` ≠ `slug` after duplication | DevRel again, via poster filenames (`.issues/079`) |

Two of those were found by people outside the team, and one only surfaced
because a transaction failed. None of them had to be discovered that way — each
is a single `SELECT`.

## What

`scripts/verify/event_invariants_audit.py` — 11 rules, each one a bug that
actually happened. Read-only; it never writes. Exits non-zero on a violation.

Rules carry a `consequence` rather than a rule name alone, because the useful
question is not "which invariant broke" but "what does a person see". The
output says *"recap QR codes return 409 — the link exists and leads nowhere"*,
not *"rule 2 failed"*.

Rules are **blocking** or **advisory**. Advisory covers judgement calls — an
organizer may legitimately not want post-event registration open, and the ten
`id ≠ slug` rows are history that will not be rewritten. `--strict` promotes
them for a clean-slate check.

## First run against production

```
✅ past event still accepting registration
⚠️  completed event with post-event registration closed   (1)  islanddao-v4-demo
❌ quiz enabled with no quiz configured                   (12)
⚠️  event id disagrees with its slug                      (10)
✅ …7 others
```

It found the one remaining event whose post-event registration was still shut,
without anyone looking for it — the other eleven had been fixed minutes
earlier. That is the behaviour this is for.

The 12 quiz-enabled-but-configless events are DevRel's finding from
`BETHERE-ASKS.md` item 5, now automated instead of depending on someone reading
the database.

## CI verifies the auditor, humans run the audit

Pointing the gate at a live database was the obvious design and the wrong one.
Staging is **deliberately** full of harness fixtures in contradictory states —
that is what a fixture is — so the audit reports 3 blocking violations there,
all of them `flow-test-event`, `escrow-e2e-*` and `test`. Production findings
are frequently intentional organizer choices. Either target makes a permanently
red gate, and a permanently red gate is not read.

So CI runs `--self-test`: every rule is exercised **both ways** against an
in-memory SQLite — it must fire on a violating row and stay silent on a clean
one. That needs no database, no credentials, and no network.

It also closes the failure mode of `.issues/072`: a rule whose SQL can never
match would pass a one-directional check forever. Verified by breaking one
rule's SQL to `WHERE 1 = 0`; the self-test reports
*"did not fire on a violating row"* and exits 1.

The audit against real data is a manual command:

```sh
python3 scripts/verify/event_invariants_audit.py --db bethere-db
python3 scripts/verify/event_invariants_audit.py --db bethere-db --strict
```

## Not covered

Rules are per-event and cross-table only where a `JOIN` is cheap. It does not
check on-chain state (that is `onchain_event_id_audit.py`), and it does not
check attendee-level invariants — a natural next step if this proves useful.

## Related

- `scripts/verify/onchain_event_id_audit.py` — the same shape, for `.issues/085`.
- `.issues/079`, `.issues/083`, `.issues/085`, `.issues/086` — the bugs that
  motivated each rule.
