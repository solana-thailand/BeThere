"""Manual participation override (PATCH /attendee/{id}/participation-type).

Runs the worker's real SQL file (`worker/src/db/sql/`) against every
production migration in SQLite. `attendees.id` is global, so the write must be
scoped to the event: staff of one event must not be able to move another
event's registrant between tracks by id (Issue 153).
"""

from pathlib import Path
import sqlite3
import unittest

WORKER = Path(__file__).resolve().parents[2]
SET = (WORKER / 'src/db/sql/attendee_participation_set.sql').read_text()


class ParticipationOverrideTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(':memory:')
        for migration in sorted((WORKER / 'migrations').glob('*.sql')):
            self.db.executescript(migration.read_text())
        for event in ('rtm6', 'other'):
            self.db.execute(
                "INSERT INTO events (id, name, slug, event_start_ms, event_end_ms) VALUES (?, ?, ?, 1, 2)",
                (event, event, event))
        for attendee_id, event_id in (('a', 'rtm6'), ('x', 'other')):
            self.db.execute(
                "INSERT INTO attendees (id, event_id, email, name, participation_type) "
                "VALUES (?, ?, ?, 'Name', 'in_person')",
                (attendee_id, event_id, attendee_id + '@example.com'))

    def tearDown(self):
        self.db.close()

    def set(self, event_id, attendee_id, value):
        return self.db.execute(SET, (event_id, attendee_id, value)).rowcount

    def track(self, attendee_id):
        return self.db.execute(
            'SELECT participation_type FROM attendees WHERE id = ?', (attendee_id,)).fetchone()[0]

    def test_attendee_of_the_event_is_moved(self):
        self.assertEqual(self.set('rtm6', 'a', 'online'), 1)
        self.assertEqual(self.track('a'), 'online')

    def test_attendee_of_another_event_is_not_written(self):
        self.assertEqual(self.set('rtm6', 'x', 'online'), 0)
        self.assertEqual(self.track('x'), 'in_person')

    def test_unknown_attendee_changes_nothing(self):
        self.assertEqual(self.set('rtm6', 'nobody', 'online'), 0)


if __name__ == '__main__':
    unittest.main()
