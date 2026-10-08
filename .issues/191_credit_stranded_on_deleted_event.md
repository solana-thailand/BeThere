# 191 · Credit applied to a hard-deleted event stays locked forever

**Status:** open. Found 2026-10-08 (session `event-checkin-8a`) while building
`.issues/190`; read from the code on `develop` `4d9cf386` and reproduced on
the migrated SQLite schema with the worker's own release SQL (the repro
below: 0 rows released, balance 0, the lock still listed). Not checked
against prod data.

## What is wrong

An `apply` row in `credit_ledger` (−฿, registration paid with rolling credit)
is given back by a `return` row either at check-in or by
`credit_ledger::release_ended_applies` once the event has ended. The release
statement, `RELEASE_ENDED_APPLIES_SQL` (`worker/src/db/credit_ledger.rs:55`),
finds ended events with an **inner** join:

    FROM credit_ledger a JOIN events e ON e.id = a.event_id
    WHERE a.reason = 'apply' AND a.delta < 0 AND e.event_end_ms > 0
      AND e.event_end_ms <= now …

A hard delete (`DELETE /api/events/{id}/delete`, super-admin, archived events
only) removes the `events` row. `db::event_purge` deliberately **keeps**
`credit_ledger` (`EVENT_PURGE_KEEPS`, `worker/src/db/event_purge.rs:23` —
"a person's balance, not the event's"). After that, the apply row has no event
to join, so no `return` is ever written: the credit is locked forever. An
event can be archived (and then deleted) before it ends — a cancelled event is
exactly that case — so this is reachable for an attendee who paid with credit
and never checked in.

Downstream:

- `locked_applies` (LEFT JOIN) keeps listing it, named by the bare event id with
  `event_end_ms = 0`;
- the payout clear's `.issues/120` §3 guard then refuses a request whose whole
  balance is that lock ("returns when that event ends") — an event that will
  never end;
- the attendee's spendable balance is short by the amount for good, while
  owner rule D1 says nothing is forfeited.

## Repro (SQL, against the migrated schema)

```sql
INSERT INTO events (id, name, slug, event_start_ms, event_end_ms)
  VALUES ('gone', 'Gone', 'gone', 1600000000000, 1600000003600);  -- ended long ago
INSERT INTO credit_ledger (email, organization_id, currency, delta, reason, deposit_id)
  VALUES ('p@x.example', '', 'thb', 500, 'hold', 'e1:a');
INSERT INTO credit_ledger (email, organization_id, currency, delta, reason, event_id, deposit_id)
  VALUES ('p@x.example', '', 'thb', -500, 'apply', 'gone', 'apply:gone:p@x.example');
DELETE FROM events WHERE id = 'gone';        -- what the hard delete does
-- run RELEASE_ENDED_APPLIES_SQL: 0 rows; the balance stays 0 and the lock
-- stays in locked_applies, however long you wait.
```

## Proposed fix (not done)

Return the credit when the event is hard-deleted, in the same D1 batch as
`event_purge` (all or nothing), keyed exactly like the other two writers so the
three can never double-return:

```sql
INSERT INTO credit_ledger (email, organization_id, currency, delta, reason,
                           event_id, deposit_id, note)
SELECT a.email, a.organization_id, a.currency, -a.delta, 'return', a.event_id,
       'return:' || a.event_id || ':' || a.email, 'event_deleted'
FROM credit_ledger a
WHERE a.event_id = ?1 AND a.reason = 'apply' AND a.delta < 0
  AND NOT EXISTS (SELECT 1 FROM credit_ledger r WHERE r.reason = 'return'
                  AND r.event_id = a.event_id AND r.email = a.email)
ON CONFLICT (deposit_id, reason) WHERE deposit_id IS NOT NULL DO NOTHING
```

Plus a one-off repair for any apply already orphaned (same statement with
`NOT EXISTS (SELECT 1 FROM events e WHERE e.id = a.event_id)` instead of the
id), and a `reconcile` count of unreturned applies whose event is gone so a
new path that drops the events row shows up the next night. Test with the
`test_locked_credit.py` fixture: delete the event, run the purge batch, assert
the balance is back and `locked_applies` is empty.
