"""Credit applied to a hard-deleted event (.issues/191).

Runs the worker's real SQL (extracted from the Rust sources, same harness as
`test_locked_credit`) on the production migrations:

- the repro: with the event row gone, the ended-event release returns nothing
  and the credit stays locked;
- the fix: the purge batch's first statement returns it, before the row goes;
- the nightly repair returns credit stranded by an earlier delete, but only
  when the audit log records that hard delete (a missing row alone can be a
  KV-only event whose D1 write failed);
- nothing is returned twice, and what is still stranded is counted.
"""

import unittest

from test_person_emails import (
    GMAIL,
    OTHER,
    SRC,
    CreditFixture,
    concat_after,
    expand_concat,
    plain_after,
    release_sql,
)

LEDGER = "db/credit_ledger.rs"
LOCKED_SQL = concat_after(LEDGER, "pub async fn locked_applies(")
POSITIVE_SQL = concat_after(LEDGER, "pub async fn positive_balances(")
RELEASE_SQL = release_sql("RELEASE_ENDED_APPLIES_SQL")
STRANDED_SQL = plain_after(LEDGER, "pub(crate) const STRANDED_APPLIES_SQL")
PURGE_SQL = (SRC / "db/sql/event_purge.sql").read_text()


def return_sql(const):
    """A constant built by `return_deleted_applies_sql!`, its `$where` expanded."""
    source = (SRC / LEDGER).read_text()
    body = source[source.index("macro_rules! return_deleted_applies_sql") :]
    template = body[body.index("concat!(") + len("concat!(") : body.index("\n    };")]
    where = concat_after(LEDGER, f"pub(crate) const {const}: &str =", opener="return_deleted_applies_sql!(")
    return expand_concat(template.replace("$($where)+", "$where"), where)


ON_DELETE_SQL = return_sql("RETURN_DELETED_EVENT_APPLIES_SQL")
REPAIR_SQL = return_sql("RETURN_HARD_DELETED_APPLIES_SQL")

PAST = 1_600_000_000_000
FUTURE = 4_100_000_000_000


class DeletedEventCreditTests(CreditFixture):
    def event(self, event_id, end_ms):
        self.db.execute(
            "INSERT INTO events (id, name, slug, event_start_ms, event_end_ms) VALUES (?, ?, ?, ?, ?)",
            (event_id, event_id.upper(), event_id, end_ms - 1, end_ms),
        )

    def pay_with_credit(self, email, event_id):
        self.ledger(email, 500, "hold", deposit_id=f"earlier:{email}")
        self.ledger(email, -500, "apply", event_id=event_id, deposit_id=f"apply:{event_id}:{email}")

    def spendable(self, email):
        return sum(r["balance"] for r in self.db.execute(POSITIVE_SQL, (email,)).fetchall())

    def locked(self, email):
        return [r["event_id"] for r in self.db.execute(LOCKED_SQL, (email,)).fetchall()]

    def hard_delete(self, event_id, with_return=True):
        """What `purge_event_rows` + the handler do: the batch, then the row."""
        if with_return:
            self.db.execute(ON_DELETE_SQL, (event_id,))
        self.db.executescript(PURGE_SQL.replace("?1", f"'{event_id}'"))
        self.db.execute("DELETE FROM events WHERE id = ?", (event_id,))

    def audit_delete(self, event_id, action="event_hard_deleted"):
        self.db.execute(
            "INSERT INTO audit_log (event_id, actor, action, target, description) VALUES ('__global__', 'admin@x', ?, ?, 'event permanently deleted')",
            (action, event_id),
        )

    def returns(self, email):
        return self.db.execute(
            "SELECT COUNT(*) FROM credit_ledger WHERE reason = 'return' AND email = ?", (email,)
        ).fetchone()[0]

    def test_repro_without_the_fix_the_credit_stays_locked(self):
        self.event("cancelled", FUTURE)
        self.pay_with_credit(GMAIL, "cancelled")
        self.hard_delete("cancelled", with_return=False)
        self.db.execute(RELEASE_SQL)
        self.assertEqual(self.spendable(GMAIL), 0)
        self.assertEqual(self.locked(GMAIL), ["cancelled"])
        self.assertEqual(self.db.execute(STRANDED_SQL).fetchone()[0], 1)

    def test_the_delete_batch_returns_it(self):
        self.event("cancelled", FUTURE)
        self.pay_with_credit(GMAIL, "cancelled")
        self.hard_delete("cancelled")
        self.assertEqual(self.spendable(GMAIL), 500)
        self.assertEqual(self.locked(GMAIL), [])
        self.assertEqual(self.db.execute(STRANDED_SQL).fetchone()[0], 0)
        note = self.db.execute("SELECT note FROM credit_ledger WHERE reason = 'return'").fetchone()[0]
        self.assertEqual(note, "event_deleted")

    def test_repair_needs_the_audited_delete_not_just_a_missing_row(self):
        self.event("deleted", FUTURE)
        self.pay_with_credit(GMAIL, "deleted")
        self.hard_delete("deleted", with_return=False)
        # a KV-only event: no D1 row, never deleted
        self.pay_with_credit(OTHER, "kv-only")
        self.db.execute(REPAIR_SQL)
        self.assertEqual(self.spendable(GMAIL), 0, "no audit row yet: not proof")
        self.audit_delete("deleted")
        self.audit_delete("other-event", action="event_created")
        self.db.execute(REPAIR_SQL)
        self.assertEqual(self.spendable(GMAIL), 500)
        self.assertEqual(self.spendable(OTHER), 0, "the KV-only event keeps its lock")
        self.assertEqual(self.db.execute(STRANDED_SQL).fetchone()[0], 1, "and is counted")

    def test_force_delete_counts_and_nothing_returns_twice(self):
        self.event("forced", PAST)
        self.pay_with_credit(GMAIL, "forced")
        # returned at check-in before the delete
        self.ledger(GMAIL, 500, "return", event_id="forced", deposit_id=f"return:forced:{GMAIL}")
        self.hard_delete("forced")
        self.audit_delete("forced", action="force_delete_used")
        self.db.execute(REPAIR_SQL)
        self.db.execute(RELEASE_SQL)
        self.assertEqual(self.returns(GMAIL), 1)
        self.assertEqual(self.spendable(GMAIL), 500)


if __name__ == "__main__":
    unittest.main()
