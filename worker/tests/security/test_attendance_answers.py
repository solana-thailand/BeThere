"""Attendance answers (migration 0052) and the refund-queue context.

Runs the worker's real SQL files (`worker/src/db/sql/`) against every
production migration in SQLite. The write is scoped through `attendees`, so an
answer can only land on an attendee of the named event: staff of one event
must not be able to write answers onto another event's registrant by id.
"""

from pathlib import Path
import sqlite3
import unittest

WORKER = Path(__file__).resolve().parents[2]
SQL = WORKER / 'src/db/sql'
SET = (SQL / 'attendance_answer_set.sql').read_text()
CLEAR = (SQL / 'attendance_answer_clear.sql').read_text()
CONTEXT = (SQL / 'refund_queue_context.sql').read_text()


class AttendanceAnswerTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(':memory:')
        self.db.row_factory = sqlite3.Row
        for migration in sorted((WORKER / 'migrations').glob('*.sql')):
            self.db.executescript(migration.read_text())
        for event in ('rtm6', 'other'):
            self.db.execute(
                "INSERT INTO events (id, name, slug, event_start_ms, event_end_ms) VALUES (?, ?, ?, 1, 2)",
                (event, event, event))
        self.attendee('a', 'rtm6', 'in_person', '')
        self.attendee('b', 'rtm6', 'online', '2026-09-27T10:00:00+07:00')
        self.attendee('x', 'other', 'in_person', '')

    def tearDown(self):
        self.db.close()

    def attendee(self, attendee_id, event_id, participation, checked_in_at):
        self.db.execute(
            "INSERT INTO attendees (id, event_id, email, name, participation_type, checked_in_at) "
            "VALUES (?, ?, ?, 'Name', ?, ?)",
            (attendee_id, event_id, attendee_id + '@example.com', participation, checked_in_at))

    def set(self, event_id, attendee_id, answer):
        return self.db.execute(SET, (event_id, attendee_id, answer, 'staff@example.com')).rowcount

    def answers(self):
        return {r['attendee_id']: r['answer']
                for r in self.db.execute('SELECT attendee_id, answer FROM attendance_answers')}

    def test_set_then_change_keeps_one_row(self):
        self.assertEqual(self.set('rtm6', 'a', 'undecided'), 1)
        self.assertEqual(self.set('rtm6', 'a', 'not_coming'), 1)
        self.assertEqual(self.answers(), {'a': 'not_coming'})

    def test_attendee_of_another_event_is_not_written(self):
        self.assertEqual(self.set('rtm6', 'x', 'coming'), 0)
        self.assertEqual(self.set('rtm6', 'nobody', 'coming'), 0)
        self.assertEqual(self.answers(), {})

    def test_unknown_answer_is_refused_by_the_check(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.set('rtm6', 'a', 'maybe')

    def test_clear_removes_only_that_answer(self):
        self.set('rtm6', 'a', 'coming')
        self.set('rtm6', 'b', 'coming')
        self.assertEqual(self.db.execute(CLEAR, ('rtm6', 'a')).rowcount, 1)
        self.assertEqual(self.db.execute(CLEAR, ('rtm6', 'a')).rowcount, 0)
        self.assertEqual(self.answers(), {'b': 'coming'})

    def test_refund_context_joins_participation_checkin_and_answer(self):
        self.set('rtm6', 'a', 'not_coming')
        rows = {r['attendee_id']: dict(r) for r in self.db.execute(CONTEXT, ('rtm6',))}
        self.assertEqual(set(rows), {'a', 'b'}, 'scoped to the event')
        self.assertEqual(rows['a']['participation_type'], 'in_person')
        self.assertEqual(rows['a']['checked_in'], 0)
        self.assertEqual(rows['a']['answer'], 'not_coming')
        self.assertEqual(rows['b']['participation_type'], 'online')
        self.assertEqual(rows['b']['checked_in'], 1)
        self.assertIsNone(rows['b']['answer'])


if __name__ == '__main__':
    unittest.main()
