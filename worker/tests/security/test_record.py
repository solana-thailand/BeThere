"""Your record (Super GOAT SG1): the worker's real SQL (src/record.rs) on the
production migrations. Only the person's own linked emails, only events that
ran and belong to a campaign (smoke and draft events out), no staff
invitations; staff, track and virtual check-ins reach Rust as columns."""

import sqlite3
import unittest

from test_person_emails import WORKER, concat_after

DOOR = concat_after("record.rs", "pub const DOOR_ROWS_SQL")
BADGES = concat_after("record.rs", "pub const BADGES_SQL")


class RecordTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        self.n = 0
        self.db.execute("INSERT OR IGNORE INTO campaigns (id, title, status) VALUES ('series', 'Series', 'active')")

    def tearDown(self):
        self.db.close()

    def event(self, slug, start, status="completed", campaign=True, staff=""):
        self.db.execute(
            "INSERT INTO events (id, name, slug, status, visibility, event_start_ms, event_end_ms, staff_emails) "
            "VALUES (?, ?, ?, ?, 'public', ?, ?, ?)",
            (f"id-{slug}", slug.upper(), slug, status, start, start + 1, staff),
        )
        if campaign:
            self.db.execute("INSERT INTO campaign_events (campaign_id, event_id) VALUES ('series', ?)", (f"id-{slug}",))

    def row(self, slug, email, came=True, by="staff@x.example", pt="in_person", approval="approved", asset=None):
        self.n += 1
        self.db.execute(
            "INSERT INTO attendees (id, event_id, email, participation_type, checked_in_at, checked_in_by, approval_status, claim_asset_id) "
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            (f"a{self.n}", f"id-{slug}", email, pt, "2026-10-01T10:00:00Z" if came else None, by if came else None, approval, asset),
        )

    def link(self, *emails):
        for i, e in enumerate(emails):
            self.db.execute(
                "INSERT INTO person_emails (email, person_id, is_primary, proof) VALUES (?, 'p1', ?, 'google')",
                (e, int(i == 0)),
            )

    def door(self, email):
        return [tuple(r) for r in self.db.execute(DOOR, (email,))]

    def test_only_campaign_events_that_ran(self):
        self.event("rtm1", 1)
        self.event("smoke", 2, campaign=False)
        self.event("draft", 3, status="draft")
        for slug in ("rtm1", "smoke", "draft"):
            self.row(slug, "me@x.example")
        self.assertEqual([r[0] for r in self.door("me@x.example")], ["rtm1"])

    def test_linked_emails_in_other_people_out_invited_out(self):
        self.event("rtm1", 1)
        self.event("rtm2", 2)
        self.link("me@x.example", "old@x.example")
        self.row("rtm1", "Old@X.example")
        self.row("rtm2", "me@x.example", came=False)
        self.row("rtm2", "other@x.example")
        self.row("rtm1", "me@x.example", approval="invited")
        rows = self.door("me@x.example")
        self.assertEqual([(r[0], r[5]) for r in rows], [("rtm1", 1), ("rtm2", 0)])
        self.assertEqual([r[0] for r in self.door("other@x.example")], ["rtm2"])

    def test_columns_carry_track_virtual_and_crew(self):
        self.event("rtm1", 1, staff="Crew@x.example, lead@x.example")
        self.row("rtm1", "crew@x.example")
        self.row("rtm1", "v@x.example", by="virtual", pt="online")
        crew = self.door("crew@x.example")[0]
        self.assertEqual((crew[3], crew[7]), (2, 1))
        virt = self.door("v@x.example")[0]
        self.assertEqual((virt[4], virt[6], virt[7]), ("online", "virtual", 0))

    def test_badges_any_event_own_only(self):
        self.event("rtm1", 1)
        self.event("oneoff", 2, campaign=False)
        self.row("oneoff", "me@x.example", asset="Asset111")
        self.row("rtm1", "me@x.example")
        self.row("rtm1", "other@x.example", asset="Asset222")
        self.assertEqual(
            [tuple(r) for r in self.db.execute(BADGES, ("me@x.example",))],
            [("oneoff", "ONEOFF", "Asset111")],
        )


if __name__ == "__main__":
    unittest.main()
