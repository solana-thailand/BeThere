"""Linked emails share one credit balance (plan 025, issue #122).

Runs the worker's real SQL, extracted from the Rust sources, against the
production migrations. The source-scan guards in `credit_ledger_guards.rs`
prove every reader uses the person fragment; this proves the fragment and the
queries built from it do the right thing with money.
"""

from pathlib import Path
import re
import sqlite3
import unittest


WORKER = Path(__file__).resolve().parents[2]
SRC = WORKER / "src"

LITERAL = r'"((?:[^"\\]|\\[\s\S])*)"'


def unescape(body):
    """Rust string literal body -> text (line continuations, escaped quotes)."""
    return re.sub(r"\\\n\s*", "", body).replace('\\"', '"')


# Every `concat!`-only SQL macro the worker builds its person-scoped queries
# from, and the file that defines it. They nest (`unreturned_apply_of!` expands
# `person_emails_of!`), so the expander below is recursive.
MACRO_SOURCES = {
    "person_emails_of": "db/person.rs",
    "unreturned_apply_of": "db/credit_ledger.rs",
}

# `name!("literal")` or `name!($param)`, with any path prefix (`crate::db::x::`,
# `$crate::db::x::`) — group 1 name, group 2 literal argument, group 3 param.
MACRO_CALL = (
    r"(?:\$?crate::)?(?:\w+::)*(" + "|".join(MACRO_SOURCES) + r")!\(\s*(?:"
    + LITERAL + r"|\$(\w+))\s*\)"
)


def macro_body(name):
    """The body of a `macro_rules! name` whose single arm is one `concat!`."""
    source = (SRC / MACRO_SOURCES[name]).read_text()
    body = source[source.index(f"macro_rules! {name}") :]
    return body[body.index("concat!(") + len("concat!(") : body.index("\n    };")]


def expand_concat(args, arg=None):
    """Evaluate a `concat!(...)` argument list: literals, `$param`
    placeholders bound to `arg`, and nested macro calls (recursively)."""
    parts = []
    token = MACRO_CALL + "|" + LITERAL + r"|\$(\w+)"
    for match in re.finditer(token, args):
        if match.group(1) is not None:
            inner = unescape(match.group(2)) if match.group(2) is not None else arg
            parts.append(expand_concat(macro_body(match.group(1)), inner))
        elif match.group(4) is not None:
            parts.append(unescape(match.group(4)))
        else:
            parts.append(arg)
    return "".join(parts)


def person_fragment(email_expr):
    """Expand `person_emails_of!(email_expr)` from its macro definition."""
    return expand_concat(macro_body("person_emails_of"), email_expr)


def concat_after(path, anchor, opener="concat!("):
    """The SQL of the first `concat!(...)` after `anchor` in `path`."""
    source = (SRC / path).read_text()
    rest = source[source.index(anchor) :]
    rest = rest[rest.index(opener) + len(opener) :]
    depth, end = 1, 0
    in_string = False
    i = 0
    while i < len(rest):
        ch = rest[i]
        if in_string:
            if ch == "\\":
                i += 2
                continue
            if ch == '"':
                in_string = False
        elif ch == '"':
            in_string = True
        elif ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                end = i
                break
        i += 1
    return expand_concat(rest[:end])


def plain_after(path, anchor):
    """The first plain string literal after `anchor` in `path`."""
    source = (SRC / path).read_text()
    rest = source[source.index(anchor) :]
    return unescape(re.search(LITERAL, rest).group(1))


def const_sql(name):
    source = (SRC / "db/person.rs").read_text()
    rest = source[source.index(f"const {name}: &str = ") :]
    return unescape(re.search(LITERAL, rest).group(1))


LINK_SQL = [
    const_sql("LINK_FIRST_JOINS_SECOND_SQL"),
    const_sql("LINK_NEW_PERSON_SQL"),
    const_sql("LINK_SECOND_JOINS_FIRST_SQL"),
]
LINK_STATE_SQL = const_sql("LINK_STATE_SQL")
BALANCE_SQL = concat_after("db/credit_ledger.rs", "pub async fn balance(")
POSITIVE_SQL = concat_after("db/credit_ledger.rs", "pub async fn positive_balances(")
TRY_SPEND_SQL = concat_after("db/credit_ledger.rs", "pub async fn try_spend(")
THB_BALANCES_SQL = plain_after(
    "db/credit_ledger.rs", "pub async fn thb_balances_by_email("
)
LIABILITY_SQL = plain_after("db/credit_ledger.rs", "pub async fn liability(")
NEGATIVE_SQL = plain_after("db/credit_ledger.rs", "let negative_balances = count_query(")
APPLY_SPEND_SQL = concat_after(
    "db/credit_coverage.rs", "let spend = db", opener=".prepare(concat!("
)
QUEUE_SQL = concat_after("db/contacts.rs", "pub async fn credit_refund_requests(")
CLEAR_FLAG_SQL = concat_after(
    "db/contacts.rs", "pub(crate) async fn clear_credit_refund_requested(",
    opener="db.prepare(concat!(",
)
CLAIMED_ELSEWHERE_SQL = concat_after(
    "db/person.rs", "const CLAIMED_ELSEWHERE_SQL: &str = "
)

GMAIL = "person@gmail.com"
WORK = "person@work.example"
OTHER = "stranger@gmail.com"


class CreditFixture(unittest.TestCase):
    """Migrations + the ledger/link/refund-request helpers, no tests of its own.

    `test_locked_credit` builds on this; inheriting `PersonEmailsTests`
    instead would silently re-run every test in this module.
    """

    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.row_factory = sqlite3.Row
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        self.person_ids = iter(f"person-{n}" for n in range(100))

    def tearDown(self):
        self.db.close()

    # -- helpers ---------------------------------------------------------

    def link(self, session_email, added_email):
        """Mirror `person::link_google`'s batch; returns (written, same_person)."""
        binds = (session_email, next(self.person_ids), added_email)
        written = 0
        with self.db:
            for sql in LINK_SQL:
                written += self.db.execute(sql, binds).rowcount
            same = self.db.execute(LINK_STATE_SQL, binds).fetchone()["same_person"]
        return written, same

    def ledger(self, email, delta, reason, event_id=None, deposit_id=None):
        self.db.execute(
            """INSERT INTO credit_ledger
                   (email, organization_id, currency, delta, reason, event_id, deposit_id)
               VALUES (?, '', 'thb', ?, ?, ?, ?)""",
            (email, delta, reason, event_id, deposit_id),
        )

    def balance(self, email):
        return self.db.execute(BALANCE_SQL, (email, "", "thb")).fetchone()["bal"]

    def person_of(self, email):
        row = self.db.execute(
            "SELECT person_id FROM person_emails WHERE email=?", (email,)
        ).fetchone()
        return None if row is None else row["person_id"]

    def request_refund(self, email, requested_at):
        self.db.execute(
            """INSERT INTO contacts (email, name, credit_refund_requested,
                   credit_refund_requested_at) VALUES (?, 'P', 1, ?)""",
            (email, requested_at),
        )


class PersonEmailsTests(CreditFixture):
    # -- linking ---------------------------------------------------------

    def test_first_link_creates_one_person_with_the_session_email_primary(self):
        written, same = self.link(GMAIL, WORK)
        self.assertEqual((written, same), (2, 1))
        rows = self.db.execute(
            "SELECT email, is_primary, proof FROM person_emails ORDER BY email"
        ).fetchall()
        self.assertEqual(
            [tuple(r) for r in rows],
            [(GMAIL, 1, "google"), (WORK, 0, "google")],
        )

    def test_relinking_is_a_no_op(self):
        self.link(GMAIL, WORK)
        self.assertEqual(self.link(GMAIL, WORK), (0, 1))
        self.assertEqual(self.link(WORK, GMAIL), (0, 1))

    def test_an_unlinked_email_joins_whichever_side_is_already_linked(self):
        self.link(GMAIL, WORK)
        self.assertEqual(self.link("third@x.example", WORK), (1, 1))
        self.assertEqual(self.person_of("third@x.example"), self.person_of(GMAIL))
        primaries = self.db.execute(
            "SELECT COUNT(*) AS n FROM person_emails WHERE is_primary=1"
        ).fetchone()["n"]
        self.assertEqual(primaries, 1)

    def test_two_linked_people_are_never_merged(self):
        self.link(GMAIL, WORK)
        self.link(OTHER, "stranger@work.example")
        before = self.db.execute("SELECT * FROM person_emails").fetchall()
        self.assertEqual(self.link(GMAIL, OTHER), (0, 0))
        after = self.db.execute("SELECT * FROM person_emails").fetchall()
        self.assertEqual([tuple(r) for r in before], [tuple(r) for r in after])

    def test_second_primary_for_a_person_is_rejected(self):
        self.link(GMAIL, WORK)
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(
                "UPDATE person_emails SET is_primary=1 WHERE email=?", (WORK,)
            )

    # -- balances --------------------------------------------------------

    def test_unlinked_emails_keep_their_own_balance(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="e1:a")
        self.assertEqual(self.balance(GMAIL), 500)
        self.assertEqual(self.balance(WORK), 0)

    def test_linked_emails_share_one_balance(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="e1:a")
        self.link(GMAIL, WORK)
        self.assertEqual(self.balance(WORK), 500)
        self.assertEqual(self.balance(GMAIL), 500)
        self.assertEqual(self.balance(OTHER), 0)
        buckets = self.db.execute(POSITIVE_SQL, (WORK,)).fetchall()
        self.assertEqual([tuple(b) for b in buckets], [("", "thb", 500)])

    def test_roster_map_covers_a_linked_email_with_no_ledger_rows(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="e1:a")
        self.ledger(OTHER, 500, "hold", deposit_id="e1:b")
        self.link(GMAIL, WORK)
        rows = self.db.execute(THB_BALANCES_SQL, ("",)).fetchall()
        self.assertEqual(
            sorted((r["email"], r["bal"]) for r in rows),
            [(GMAIL, 500), (WORK, 500), (OTHER, 500)],
        )
        liability = self.db.execute(LIABILITY_SQL).fetchall()
        self.assertEqual([tuple(r) for r in liability], [("", "thb", 1000, 2)])

    # -- spending --------------------------------------------------------

    def spend(self, email, event_id, attendee_id):
        cur = self.db.execute(
            APPLY_SPEND_SQL,
            (
                email, "", "thb", 500, event_id,
                f"apply:{event_id}:{email}", attendee_id, "credit_thb",
            ),
        )
        return cur.rowcount

    def test_linked_email_spends_the_person_credit_exactly_once(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="e1:a")
        self.link(GMAIL, WORK)
        self.assertEqual(self.spend(WORK, "e2", "att-work"), 1)
        self.assertEqual(self.balance(WORK), 0)
        self.assertEqual(self.balance(GMAIL), 0)
        # The apply row keeps the registered email, so its return pairs with it.
        row = self.db.execute(
            "SELECT email, delta FROM credit_ledger WHERE reason='apply'"
        ).fetchone()
        self.assertEqual(tuple(row), (WORK, -500))
        # Neither email can spend it again anywhere.
        self.assertEqual(self.spend(GMAIL, "e3", "att-gmail"), 0)
        self.assertEqual(self.spend(WORK, "e3", "att-work3"), 0)

    def test_unlinked_email_still_cannot_spend_someone_elses_credit(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="e1:a")
        self.assertEqual(self.spend(WORK, "e2", "att-work"), 0)
        self.assertEqual(
            self.db.execute(
                TRY_SPEND_SQL, (WORK, "", "thb", 500, "e2", "apply:e2:w")
            ).rowcount,
            0,
        )

    def test_cross_email_spend_is_not_a_negative_balance_alarm(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="e1:a")
        self.link(GMAIL, WORK)
        self.spend(WORK, "e2", "att-work")
        self.assertEqual(self.db.execute(NEGATIVE_SQL).fetchone()["n"], 0)
        # Per email it IS negative — the alarm would have fired before plan 025.
        per_email = self.db.execute(
            """SELECT COUNT(*) AS n FROM (SELECT SUM(delta) FROM credit_ledger
               GROUP BY email, organization_id, currency HAVING SUM(delta) < 0)"""
        ).fetchone()["n"]
        self.assertEqual(per_email, 1)

    def test_payout_queue_shows_the_person_total(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="e1:a")
        self.ledger(WORK, 500, "hold", deposit_id="e1:b")
        self.link(GMAIL, WORK)
        self.db.execute(
            """INSERT INTO contacts (email, name, credit_refund_requested,
                   credit_refund_requested_at) VALUES (?, 'P', 1, 'now')""",
            (WORK,),
        )
        row = self.db.execute(QUEUE_SQL).fetchone()
        self.assertEqual((row["email"], row["credit_thb"]), (WORK, 1000))


    # -- one badge per person per event ----------------------------------

    def attendee(
        self,
        attendee_id,
        email,
        claimed_at=None,
        claim_token=None,
        participation_type="in_person",
    ):
        self.db.execute(
            """INSERT INTO attendees (id, event_id, email, claimed_at, claim_token,
                   participation_type)
               VALUES (?, 'e1', ?, ?, ?, ?)""",
            (
                attendee_id,
                email,
                claimed_at,
                claim_token or f"tok-{attendee_id}",
                participation_type,
            ),
        )

    def claimed_elsewhere(self, email, attendee_id="", claim_token=""):
        row = self.db.execute(
            CLAIMED_ELSEWHERE_SQL, ("e1", email, attendee_id, claim_token)
        ).fetchone()
        return None if row is None else row["claimed_at"]

    def test_linked_second_row_cannot_claim_a_second_badge(self):
        self.attendee("a-gmail", GMAIL, claimed_at="2026-09-18T00:00:00Z")
        self.attendee("a-work", WORK)
        # Unlinked: two emails are two people, so nothing blocks the second row.
        self.assertIsNone(self.claimed_elsewhere(WORK, "a-work"))
        self.link(GMAIL, WORK)
        self.assertEqual(
            self.claimed_elsewhere(WORK, "a-work"), "2026-09-18T00:00:00Z"
        )

    def test_a_row_never_blocks_itself_and_an_unclaimed_sibling_is_fine(self):
        self.attendee("a-gmail", GMAIL)
        self.attendee("a-work", WORK, claimed_at="2026-09-18T00:00:00Z")
        self.link(GMAIL, WORK)
        # The claiming row itself is excluded...
        self.assertIsNone(self.claimed_elsewhere(WORK, "a-work"))
        # ...but its linked sibling sees the claim.
        self.assertEqual(
            self.claimed_elsewhere(GMAIL, "a-gmail"), "2026-09-18T00:00:00Z"
        )
        # An empty string is "not claimed", not a claim.
        self.db.execute("UPDATE attendees SET claimed_at='' WHERE id='a-work'")
        self.assertIsNone(self.claimed_elsewhere(GMAIL, "a-gmail"))

    def test_walkin_row_is_excluded_by_its_claim_token(self):
        """The walk-in path has no row id, so it excludes itself by token."""
        self.attendee("a-registered", GMAIL, claimed_at="2026-09-18T00:00:00Z")
        # Only a walk-in may repeat an email in one event
        # (idx_attendees_unique_event_email), which is exactly the path that
        # skipped the check before.
        self.attendee(
            "a-walkin", GMAIL, claim_token="tok-walkin", participation_type="walkin"
        )
        # The walk-in row does not block itself...
        self.assertEqual(
            self.claimed_elsewhere(GMAIL, "", "tok-walkin"), "2026-09-18T00:00:00Z"
        )
        # ...and when it is the only row that claimed, its own token excludes it.
        self.db.execute("DELETE FROM attendees WHERE id='a-registered'")
        self.db.execute(
            "UPDATE attendees SET claimed_at='2026-09-18T01:00:00Z' WHERE id='a-walkin'"
        )
        self.assertIsNone(self.claimed_elsewhere(GMAIL, "", "tok-walkin"))

    def test_a_different_person_in_the_same_event_never_blocks(self):
        self.attendee("a-other", OTHER, claimed_at="2026-09-18T00:00:00Z")
        self.attendee("a-work", WORK)
        self.link(GMAIL, WORK)
        self.assertIsNone(self.claimed_elsewhere(WORK, "a-work"))


    # -- payout queue collapses to one row per person ---------------------

    def test_two_linked_emails_requesting_a_refund_show_as_one_row(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="e1:a")
        self.ledger(WORK, 500, "hold", deposit_id="e1:b")
        self.link(GMAIL, WORK)
        self.request_refund(GMAIL, "2026-09-18T00:00:00Z")
        self.request_refund(WORK, "2026-09-18T01:00:00Z")
        rows = self.db.execute(QUEUE_SQL).fetchall()
        # One row, the latest request, showing the person's whole balance once —
        # paying both rows would double the payout.
        self.assertEqual(
            [(r["email"], r["credit_thb"]) for r in rows], [(WORK, 1000)]
        )

    def test_unlinked_requesters_each_keep_their_own_row(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="e1:a")
        self.ledger(OTHER, 700, "hold", deposit_id="e1:b")
        self.request_refund(GMAIL, "2026-09-18T00:00:00Z")
        self.request_refund(OTHER, "2026-09-18T01:00:00Z")
        rows = self.db.execute(QUEUE_SQL).fetchall()
        self.assertEqual(
            sorted((r["email"], r["credit_thb"]) for r in rows),
            [(GMAIL, 500), (OTHER, 700)],
        )

    def test_clearing_the_flag_clears_the_whole_person(self):
        self.link(GMAIL, WORK)
        self.request_refund(GMAIL, "2026-09-18T00:00:00Z")
        self.request_refund(WORK, "2026-09-18T01:00:00Z")
        self.db.execute(CLEAR_FLAG_SQL, (WORK,))
        still_flagged = self.db.execute(
            "SELECT COUNT(*) AS n FROM contacts WHERE credit_refund_requested=1"
        ).fetchone()["n"]
        self.assertEqual(still_flagged, 0)


if __name__ == "__main__":
    unittest.main()
