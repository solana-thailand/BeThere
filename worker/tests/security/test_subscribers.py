"""The subscriber list and the announcer's queue (.plans/045 R4.12).

Runs the worker's real SQL (`src/subscribers/db.rs`) against the production
migrations: who is owed a mail, that turning the announcer on mails nobody,
that a claim is one mail per event per subscriber, and that unsubscribing and
re-subscribing do what the form promises.
"""

from pathlib import Path
import sqlite3
import unittest

from test_person_emails import plain_after

WORKER = Path(__file__).resolve().parents[2]
DB = "subscribers/db.rs"

UPSERT = plain_after(DB, "pub const UPSERT_SQL")
UNSUBSCRIBE = plain_after(DB, "pub const UNSUBSCRIBE_SQL")
MARK_OPEN = plain_after(DB, "pub const MARK_OPEN_SQL")
DUE = plain_after(DB, "pub const DUE_SQL")
CLAIM = plain_after(DB, "pub const CLAIM_SQL")
SENT = plain_after(DB, "pub const SENT_SQL")
RELEASE = plain_after(DB, "pub const RELEASE_SQL")
ERASE_SOURCE = (WORKER / "src" / DB).read_text()
ERASE = [
    line.strip().strip('",')
    for line in ERASE_SOURCE[ERASE_SOURCE.index("pub const ERASE_SQL") :].split("];")[0].splitlines()[1:]
    if line.strip().startswith('"')
]

DAY_MS = 86_400_000
NOW_MS = 1_800_000_000_000  # 2027-01-15
TOKENS = iter(f"{n:064x}" for n in range(1, 1000))


def iso(minute):
    """A fixed clock: minute `minute` of one day, in the tables' form."""
    return f"2027-01-15T10:{minute:02d}:00Z"


class SubscriberTests(unittest.TestCase):
    def migrate(self, since="", until="~"):
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            if since <= migration.name < until:
                self.db.executescript(migration.read_text())

    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.row_factory = sqlite3.Row

    def tearDown(self):
        self.db.close()

    def event(self, event_id, status="active", visibility="public", ends=NOW_MS + DAY_MS):
        self.db.execute(
            """INSERT INTO events (id, name, slug, status, visibility, event_start_ms, event_end_ms)
               VALUES (?, ?, ?, ?, ?, ?, ?)""",
            (event_id, f"Event {event_id}", event_id, status, visibility, ends - 3600_000, ends),
        )

    def subscribe(self, email, minute, locale="en"):
        self.db.execute(UPSERT, (email, locale, iso(minute), next(TOKENS)))

    def mark_open(self, minute):
        return self.db.execute(MARK_OPEN, (iso(minute), NOW_MS)).rowcount

    def due(self):
        return [(r["event_id"], r["email"]) for r in self.db.execute(DUE, (NOW_MS, 20))]

    def token(self, email):
        return self.db.execute(
            "SELECT unsub_token FROM subscribers WHERE email = ?", (email,)
        ).fetchone()[0]

    def test_only_who_asked_before_it_opened_is_owed(self):
        self.migrate()
        self.subscribe("early@example.com", 1)
        self.event("rtm7")
        self.event("draft", status="draft")
        self.event("private", visibility="private")
        self.event("over", ends=NOW_MS - 1)
        self.assertEqual(self.mark_open(2), 1)
        self.subscribe("late@example.com", 3)
        self.assertEqual(self.due(), [("rtm7", "early@example.com")])
        # seeing it open again does not move the time it opened
        self.assertEqual(self.mark_open(9), 0)
        self.assertEqual(self.due(), [("rtm7", "early@example.com")])

    def test_turning_it_on_mails_nobody(self):
        self.migrate(until="0060")
        self.event("already-open")
        self.migrate(since="0060")  # 0060 marks what is open today as seen
        self.subscribe("early@example.com", 1)
        self.mark_open(2)
        self.assertEqual(self.due(), [])

    def test_one_mail_per_event_per_subscriber(self):
        self.migrate()
        self.subscribe("a@example.com", 1)
        self.event("rtm7")
        self.mark_open(2)
        claim = (lambda: self.db.execute(CLAIM, ("rtm7", "a@example.com", iso(3))).rowcount)
        self.assertEqual(claim(), 1)
        self.assertEqual(claim(), 0, "a second run must not claim it again")
        self.assertEqual(self.due(), [])
        # a refusal for the account frees an unsent claim, never a sent one
        self.db.execute(RELEASE, ("rtm7", "a@example.com"))
        self.assertEqual(self.due(), [("rtm7", "a@example.com")])
        claim()
        self.db.execute(SENT, ("rtm7", "a@example.com", iso(4)))
        self.db.execute(RELEASE, ("rtm7", "a@example.com"))
        self.assertEqual(self.due(), [])

    def test_unsubscribe_stops_and_resubscribe_restarts_the_clock(self):
        self.migrate()
        self.subscribe("a@example.com", 1)
        token = self.token("a@example.com")
        self.assertEqual(self.db.execute(UNSUBSCRIBE, (token, iso(2))).rowcount, 1)
        self.assertEqual(self.db.execute(UNSUBSCRIBE, (token, iso(3))).rowcount, 0)
        self.assertEqual(self.db.execute(UNSUBSCRIBE, ("f" * 64, iso(3))).rowcount, 0)
        self.event("rtm7")
        self.mark_open(4)
        self.assertEqual(self.due(), [], "unsubscribed")
        # back after it opened: consent moves to now, so rtm7 is not owed
        self.subscribe("a@example.com", 5, locale="th")
        row = self.db.execute("SELECT * FROM subscribers WHERE email = 'a@example.com'").fetchone()
        self.assertIsNone(row["unsubscribed_at"])
        self.assertEqual((row["consent_at"], row["locale"]), (iso(5), "th"))
        self.assertEqual(row["unsub_token"], token, "the link in old mails keeps working")
        self.assertEqual(self.due(), [])

    def test_repeat_while_subscribed_keeps_consent(self):
        self.migrate()
        self.subscribe("a@example.com", 1)
        self.subscribe("a@example.com", 7, locale="th")
        row = self.db.execute("SELECT * FROM subscribers").fetchone()
        self.assertEqual((row["consent_at"], row["locale"]), (iso(1), "th"))
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM subscribers").fetchone()[0], 1)

    def test_erasure_leaves_nothing(self):
        self.migrate()
        self.subscribe("a@example.com", 1)
        self.subscribe("b@example.com", 1)
        self.event("rtm7")
        self.mark_open(2)
        self.db.execute(CLAIM, ("rtm7", "a@example.com", iso(3)))
        self.assertEqual(len(ERASE), 2)
        for sql in ERASE:
            self.db.execute(sql, ("a@example.com",))
        for table in ("subscribers", "event_announcements"):
            n = self.db.execute(f"SELECT COUNT(*) FROM {table} WHERE email = 'a@example.com'").fetchone()[0]
            self.assertEqual(n, 0, table)
        self.assertEqual(self.due(), [("rtm7", "b@example.com")])


if __name__ == "__main__":
    unittest.main()
