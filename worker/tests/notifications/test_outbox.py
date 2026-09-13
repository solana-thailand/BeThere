"""Execute production migrations and dispatcher SQL against SQLite (no email/network).
Run: python3 -m unittest discover -s worker/tests/notifications -v
"""
from pathlib import Path
import sqlite3
import tempfile
import threading
import time
import unittest

WORKER = Path(__file__).resolve().parents[2]
SQL = WORKER / 'src/notifications/sql'
HANDLER_SQL = WORKER / 'src/handlers/sql'
DB_SQL = WORKER / 'src/db/sql'

def query(name):
    return (SQL / (name + '.sql')).read_text()

class OutboxTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(':memory:')
        self.db.row_factory = sqlite3.Row
        for migration in sorted((WORKER / 'migrations').glob('*.sql')):
            self.db.executescript(migration.read_text())
        self.now = int(time.time())
        self.event('event-a')
        self.event('event-b')

    def tearDown(self):
        self.db.close()

    def event(self, event):
        self.db.execute("INSERT INTO events(id,name,slug,status,event_start_ms,event_end_ms) VALUES (?,?,?,'active',?,?)",
                        (event, 'Builder night', event, (self.now+90000)*1000, (self.now+100000)*1000))

    def register(self, attendee='a', event='event-a', enroll=True):
        with self.db:
            self.db.execute("INSERT INTO attendees(id,event_id,email,name) VALUES (?,?,?,'Attendee')",
                            (attendee, event, attendee+'@example.com'))
            if enroll:
                self.db.execute('INSERT INTO notification_enrollments(attendee_id,event_id) VALUES (?,?)',(attendee,event))

    def jobs(self):
        return list(self.db.execute('SELECT * FROM notification_outbox ORDER BY id'))

    def deposit(self, verified=0, rejected=0, version='upload-1'):
        self.db.execute("INSERT INTO deposit_statuses(attendee_id,event_id,method,amount,currency,verified,rejected,deposited_at) VALUES ('a','event-a','thb',100,'THB',?,?,?) ON CONFLICT(event_id,attendee_id) DO UPDATE SET verified=excluded.verified,rejected=excluded.rejected,deposited_at=excluded.deposited_at", (verified,rejected,version))

    def test_registration_is_atomic_and_duplicate_enrollment_is_noop(self):
        self.register()
        self.db.execute("INSERT OR IGNORE INTO notification_enrollments VALUES ('a','event-a',unixepoch())")
        self.assertEqual([j['kind'] for j in self.jobs()], ['registration','reminder'])
        self.assertEqual(self.jobs()[1]['due_at'], self.now+3600)
        self.db.commit()
        with self.assertRaises(sqlite3.IntegrityError):
            with self.db:
                self.db.execute("INSERT INTO attendees(id,event_id,email) VALUES ('b','event-a','b@example.com')")
                self.db.execute("INSERT INTO notification_enrollments(attendee_id,event_id) VALUES ('b','event-a')")
                self.db.execute("INSERT INTO attendees(id,event_id,email) VALUES ('a','event-a','a@example.com')")
        self.assertIsNone(self.db.execute("SELECT id FROM attendees WHERE id='b'").fetchone())
        self.assertEqual(len(self.jobs()),2)

    def test_unverified_or_imported_attendee_has_no_jobs(self):
        self.register(enroll=False)
        self.deposit(verified=1)
        self.assertEqual(self.jobs(),[])

    def test_deposit_replays_do_not_duplicate_and_reupload_gets_new_notice(self):
        self.register()
        self.deposit(rejected=1)
        self.deposit(rejected=1)
        self.assertEqual(len(self.jobs()),3)
        self.deposit(version='upload-2')
        self.deposit(rejected=1,version='upload-2')
        self.assertEqual(len(self.jobs()),4)
        self.deposit(verified=1,version='upload-2')
        self.deposit(verified=1,version='upload-2')
        self.assertEqual(len(self.jobs()),5)

    def test_only_one_claim_and_no_resend_of_accepted(self):
        self.register()
        first=self.db.execute(query('claim')).fetchall()
        self.assertEqual(len(first),1)
        self.assertEqual(self.db.execute(query('claim')).fetchall(),[])
        self.db.execute(query('settle'),('accepted','provider-id','',0,first[0]['id'])).fetchall()
        self.assertEqual(self.db.execute(query('claim')).fetchall(),[])
        self.assertEqual(self.db.execute(query('retry'),('event-a',first[0]['id'])).fetchall(),[])

    def test_interrupted_send_is_uncertain_and_cannot_retry(self):
        self.register()
        job=self.db.execute(query('claim')).fetchone()
        self.db.execute("UPDATE notification_outbox SET attempted_at=unixepoch()-1000 WHERE id=?",(job['id'],))
        self.db.execute(query('recover'))
        self.assertEqual(self.jobs()[0]['status'],'uncertain')
        self.assertEqual(self.db.execute(query('retry'),('event-a',job['id'])).fetchall(),[])
        self.assertEqual(self.db.execute(query('claim')).fetchall(),[])

    def test_retry_is_event_scoped_and_only_once(self):
        self.register()
        job=self.jobs()[0]
        self.db.execute("UPDATE notification_outbox SET status='failed',attempts=5 WHERE id=?",(job['id'],))
        self.assertEqual(self.db.execute(query('retry'),('event-b',job['id'])).fetchall(),[])
        self.assertEqual(len(self.db.execute(query('retry'),('event-a',job['id'])).fetchall()),1)
        self.assertEqual(self.db.execute(query('retry'),('event-a',job['id'])).fetchall(),[])

    def test_cancelled_events_and_removed_attendees_are_not_sent(self):
        self.register()
        self.db.execute("UPDATE events SET status='cancelled' WHERE id='event-a'")
        self.db.execute(query('cancel'))
        self.assertTrue(all(j['status']=='cancelled' for j in self.jobs()))
        self.assertEqual(self.db.execute(query('claim')).fetchall(),[])

    def test_reschedule_and_tba(self):
        self.register()
        self.db.execute("UPDATE events SET event_start_ms=? WHERE id='event-a'",((self.now+172800)*1000,))
        self.assertEqual(self.jobs()[1]['due_at'],self.now+86400)
        self.db.execute("UPDATE events SET time_tba=1,event_start_ms=? WHERE id='event-a'",((self.now+3600)*1000,))
        self.db.execute(query('claim')).fetchall() # registration
        self.assertEqual(self.db.execute(query('claim')).fetchall(),[])
        self.db.execute("UPDATE events SET time_tba=0 WHERE id='event-a'")
        self.assertEqual(self.db.execute(query('claim')).fetchone()['kind'],'reminder')

    def test_tba_registration_later_gets_reminder(self):
        self.db.execute("UPDATE events SET time_tba=1 WHERE id='event-a'")
        self.register()
        self.assertEqual(len(self.jobs()),1)
        self.db.execute("UPDATE events SET time_tba=0 WHERE id='event-a'")
        self.assertEqual(len(self.jobs()),2)

    def test_privacy_erasure_removes_notification_data(self):
        self.register()
        self.db.execute("UPDATE attendees SET email='deleted' WHERE id='a'")
        self.assertEqual(self.jobs(),[])
        self.assertEqual(self.db.execute('SELECT * FROM notification_enrollments').fetchall(),[])

    def test_delete_attendee_or_event_removes_jobs(self):
        self.register()
        self.db.execute("DELETE FROM attendees WHERE id='a'")
        self.assertEqual(self.jobs(),[])
        self.register('b')
        self.db.execute("DELETE FROM events WHERE id='event-a'")
        self.assertEqual(self.jobs(),[])

    def test_unrelated_event_save_preserves_notification_backoff(self):
        self.register()
        future = self.now + 20000
        self.db.execute("UPDATE notification_outbox SET due_at=? WHERE kind='reminder'", (future,))
        self.db.execute("UPDATE events SET name='New title',event_start_ms=event_start_ms,time_tba=time_tba WHERE id='event-a'")
        self.assertEqual(self.jobs()[1]['due_at'], future)

    def test_claimed_reminder_can_be_deferred_without_spending_attempt(self):
        self.register()
        self.db.execute("UPDATE notification_outbox SET status='accepted' WHERE kind='registration'")
        self.db.execute("UPDATE events SET event_start_ms=? WHERE id='event-a'", ((self.now+3600)*1000,))
        job = self.db.execute(query('claim')).fetchone()
        self.assertEqual(job['attempts'], 1)
        self.db.execute("UPDATE events SET event_start_ms=? WHERE id='event-a'", ((self.now+172800)*1000,))
        self.db.execute(query('defer'), (self.now+86400, job['id']))
        reminder = self.jobs()[1]
        self.assertEqual(reminder['status'], 'pending')
        self.assertEqual(reminder['attempts'], 0)
        self.assertIsNone(reminder['attempted_at'])
        self.assertEqual(self.db.execute(query('claim')).fetchall(), [])

    def test_late_completion_cannot_overwrite_uncertain_result(self):
        self.register()
        job = self.db.execute(query('claim')).fetchone()
        self.db.execute("UPDATE notification_outbox SET status='uncertain' WHERE id=?", (job['id'],))
        self.assertEqual(self.db.execute(query('settle'), ('accepted','provider-id','',0,job['id'])).fetchall(), [])
        self.assertEqual(self.jobs()[0]['status'], 'uncertain')

    def test_identity_erasure_and_reschedule_use_indexes(self):
        cleanup = self.db.execute("EXPLAIN QUERY PLAN DELETE FROM notification_outbox WHERE attendee_id='a' AND event_id='event-a'").fetchall()
        self.assertTrue(any('notification_attendee' in r['detail'] for r in cleanup))
        enrollment = self.db.execute("EXPLAIN QUERY PLAN SELECT attendee_id FROM notification_enrollments WHERE event_id='event-a'").fetchall()
        self.assertTrue(any('notification_enrollment_event' in r['detail'] for r in enrollment))

    def test_inbox_read_is_due_and_identity_scoped(self):
        self.register()
        self.register('b', 'event-b')
        a_registration = self.db.execute(
            "SELECT id FROM notification_outbox WHERE attendee_id='a' AND kind='registration'"
        ).fetchone()['id']
        b_registration = self.db.execute(
            "SELECT id FROM notification_outbox WHERE attendee_id='b' AND kind='registration'"
        ).fetchone()['id']
        self.assertEqual(len(self.db.execute(query('inbox_list'), ('a@example.com', 2**53-1)).fetchall()), 1)
        self.assertEqual(self.db.execute(query('inbox_read'), (b_registration, 'a@example.com')).fetchall(), [])
        self.assertEqual(len(self.db.execute(query('inbox_read'), (a_registration, 'A@EXAMPLE.COM')).fetchall()), 1)
        self.assertEqual(self.db.execute(query('inbox_unread'), ('a@example.com',)).fetchone()['count'], 0)
        self.assertIsNone(self.db.execute("SELECT read_at FROM notification_outbox WHERE id=?", (b_registration,)).fetchone()['read_at'])

    def test_inbox_visibility_and_read_mutations_share_eligibility(self):
        self.register()
        registration = self.db.execute(
            "SELECT id FROM notification_outbox WHERE attendee_id='a' AND kind='registration'"
        ).fetchone()['id']
        reminder = self.db.execute(
            "SELECT id FROM notification_outbox WHERE attendee_id='a' AND kind='reminder'"
        ).fetchone()['id']
        self.db.execute("UPDATE notification_outbox SET due_at=unixepoch() WHERE id=?", (reminder,))
        self.assertEqual(len(self.db.execute(query('inbox_list'), ('a@example.com', 2**53-1)).fetchall()), 2)

        self.db.execute("UPDATE attendees SET checked_in_at='now' WHERE id='a'")
        visible = self.db.execute(query('inbox_list'), ('a@example.com', 2**53-1)).fetchall()
        self.assertEqual([row['id'] for row in visible], [registration])
        self.assertEqual(self.db.execute(query('inbox_read'), (reminder, 'a@example.com')).fetchall(), [])

        self.db.execute("UPDATE events SET status='cancelled' WHERE id='event-a'")
        self.assertEqual(self.db.execute(query('inbox_list'), ('a@example.com', 2**53-1)).fetchall(), [])
        self.db.execute(query('inbox_read_all'), ('a@example.com',))
        self.assertIsNone(self.db.execute("SELECT read_at FROM notification_outbox WHERE id=?", (registration,)).fetchone()['read_at'])

    def test_form_stays_per_event_while_the_message_is_per_person(self):
        """The collapse in 0038 must not collapse the form.

        `/feedback` used to enumerate from the notification inbox. One row per
        person is right for sending and wrong for a form whose unit is the
        event — a person with two sessions was offered one (.issues/102).
        """
        self.db.execute("UPDATE events SET status='completed' WHERE id IN ('event-a','event-b')")
        with self.db:
            for attendee, event in (('reg-a', 'event-a'), ('reg-b', 'event-b')):
                self.db.execute(
                    "INSERT INTO attendees(id,event_id,email,name,participation_type)"
                    " VALUES (?,?,'regular@example.com','Regular','online')",
                    (attendee, event))
                self.db.execute(
                    'INSERT INTO notification_enrollments(attendee_id,event_id) VALUES (?,?)',
                    (attendee, event))
        for event in ('event-a', 'event-b'):
            self.db.execute(
                'UPDATE events SET post_event_registration_open=1 WHERE id=?', (event,))

        messages = self.db.execute(
            "SELECT COUNT(*) FROM notification_outbox n JOIN attendees a ON a.id=n.attendee_id"
            " WHERE n.kind='survey' AND n.status='pending' AND lower(a.email)='regular@example.com'"
        ).fetchone()[0]
        blocks = self.db.execute(
            "SELECT COUNT(*) FROM feedback_eligible_events WHERE lower(email)='regular@example.com'"
        ).fetchone()[0]
        self.assertEqual(messages, 1, 'one message for the person')
        self.assertEqual(blocks, 2, 'one question block per session they attended')

    def test_one_survey_per_person_not_one_per_event(self):
        """`dedup_key` is `[event, attendee, kind]`, which is right for every
        kind whose destination is one event and wrong for `survey`, whose
        destination is `/feedback` — a page not scoped to an event.

        `dispatch()` sends one message per row, so before migration 0038 prod
        held 425 rows for 206 people and one person would have received twelve
        identical mails (.issues/101).
        """
        self.db.execute("UPDATE events SET status='completed' WHERE id IN ('event-a','event-b')")
        # One human, two events, two attendee rows — which is how the schema
        # represents a repeat attendee.
        #
        # The shared email is set at INSERT, not by a later UPDATE:
        # `notification_attendee_email` erases the outbox and the enrolments
        # whenever an address changes, which is correct PDPA behaviour and would
        # quietly empty the fixture.
        with self.db:
            for attendee, event in (('dup-a', 'event-a'), ('dup-b', 'event-b')):
                self.db.execute(
                    "INSERT INTO attendees(id,event_id,email,name,participation_type)"
                    " VALUES (?,?,'regular@example.com','Regular','online')",
                    (attendee, event))
                self.db.execute(
                    'INSERT INTO notification_enrollments(attendee_id,event_id) VALUES (?,?)',
                    (attendee, event))

        for event in ('event-a', 'event-b'):
            self.db.execute(
                'UPDATE events SET post_event_registration_open=1 WHERE id=?', (event,))

        rows = self.db.execute(
            "SELECT COUNT(*) FROM notification_outbox n JOIN attendees a ON a.id=n.attendee_id"
            " WHERE n.kind='survey' AND n.status='pending' AND lower(a.email)='regular@example.com'"
        ).fetchone()[0]
        self.assertEqual(rows, 1, 'two events, one person, one survey message')

    def test_survey_counts_online_by_registration_and_onsite_by_check_in(self):
        """DevRel's own counting rule, and the one their Phase 1 figures use.

        Nobody watching a livestream is scanned at a door. Gating the survey on
        `checked_in_at` excluded 337 online registrations against 2 check-ins
        (.issues/097). An in-person no-show still stays out: "How was the
        event?" is the wrong question for someone who did not come.
        """
        self.db.execute("UPDATE events SET status='completed', post_event_registration_open=1 WHERE id='event-a'")

        cases = [
            ('online-noshow', 'online', None, True),
            ('online-watched', 'online', '2026-01-01T00:00:00Z', True),
            ('onsite-came', 'in_person', '2026-01-01T00:00:00Z', True),
            ('onsite-noshow', 'in_person', None, False),
            ('retro', 'retrospective', None, False),
        ]
        for attendee, participation, checked_in, expected in cases:
            with self.subTest(participation=participation, checked_in=bool(checked_in)):
                self.register(attendee=attendee)
                self.db.execute(
                    'UPDATE attendees SET participation_type=?, checked_in_at=? WHERE id=?',
                    (participation, checked_in, attendee))
                self.db.execute(
                    "INSERT OR IGNORE INTO notification_outbox(dedup_key,event_id,attendee_id,kind,due_at)"
                    " VALUES (?, 'event-a', ?, 'survey', unixepoch())",
                    (f'["event-a","{attendee}","survey"]', attendee))
                seen = self.db.execute(
                    "SELECT COUNT(*) FROM notification_inbox_visible WHERE attendee_id=? AND kind='survey'",
                    (attendee,)).fetchone()[0]
                self.assertEqual(bool(seen), expected)

    def test_inbox_is_visible_to_every_status_past_pending(self):
        """`approval_status` is a progression, not a set of alternatives.

        PendingApproval -> Approved -> Invited -> CheckedIn. The view used to
        pin `= 'approved'`, so anyone scanned at the door — the furthest-along
        state there is — saw an empty inbox. In production that was exactly half
        the eligible rows (.issues/095).
        """
        for status in ('approved', 'invited', 'checked_in'):
            with self.subTest(status=status):
                attendee = 'att-' + status
                self.register(attendee=attendee)
                self.db.execute('UPDATE attendees SET approval_status=? WHERE id=?', (status, attendee))
                visible = self.db.execute(
                    'SELECT COUNT(*) FROM notification_inbox_visible WHERE attendee_id=?',
                    (attendee,)).fetchone()[0]
                self.assertGreater(visible, 0, f'{status} must see their inbox')

        self.register(attendee='att-pending')
        self.db.execute("UPDATE attendees SET approval_status='pending_approval' WHERE id='att-pending'")
        pending = self.db.execute(
            "SELECT COUNT(*) FROM notification_inbox_visible WHERE attendee_id='att-pending'").fetchone()[0]
        self.assertEqual(pending, 0, 'a pending registration is the one state that stays out')

    def test_inbox_uses_current_deposit_and_participation_state(self):
        self.register()
        self.db.execute("UPDATE events SET deposit_enabled=1 WHERE id='event-a'")
        row = self.db.execute(query('inbox_list'), ('a@example.com', 2**53-1)).fetchone()
        self.assertEqual(row['deposit_verified'], 0)
        self.assertEqual(row['participation_type'], 'in_person')

        self.deposit(verified=1)
        row = self.db.execute(query('inbox_list'), ('a@example.com', 2**53-1)).fetchone()
        self.assertEqual(row['deposit_verified'], 1)
        self.db.execute("UPDATE attendees SET participation_type='online' WHERE id='a'")
        row = self.db.execute(query('inbox_list'), ('a@example.com', 2**53-1)).fetchone()
        self.assertEqual(row['participation_type'], 'online')

    def complete(self, event='event-a', until=None):
        """Finish the event and open the post-event form the survey asks through."""
        self.db.execute("UPDATE events SET status='completed',event_end_ms=? WHERE id=?",
                        ((self.now-3600)*1000, event))
        self.db.execute("UPDATE events SET post_event_registration_open=1,post_event_registration_until_ms=? WHERE id=?",
                        (until, event))

    def visible_kinds(self, email='a@example.com'):
        return [row['kind'] for row in self.db.execute(query('inbox_list'), (email, 2**53-1)).fetchall()]

    def test_survey_is_only_enqueued_for_people_who_were_there(self):
        self.register()                    # enrolled and checked in
        self.register('b')                 # enrolled, never turned up
        self.register('c', enroll=False)   # checked in but never enrolled
        self.db.execute("UPDATE attendees SET checked_in_at='now' WHERE id IN ('a','c')")
        self.complete()
        self.assertEqual([j['attendee_id'] for j in self.jobs() if j['kind']=='survey'], ['a'])
        # Re-opening a form that is already open must not ask the same person twice.
        self.db.execute("UPDATE events SET post_event_registration_open=1 WHERE id='event-a'")
        self.assertEqual(len([j for j in self.jobs() if j['kind']=='survey']), 1)

    def test_finishing_an_event_retires_pre_event_jobs_but_not_the_survey(self):
        """.issues/080 — the cancel sweep used to retire every kind once the
        event was over, which is why no message could ever be sent afterwards."""
        self.register()
        self.db.execute("UPDATE attendees SET checked_in_at='now' WHERE id='a'")
        self.complete()
        self.db.execute(query('cancel'))
        states = {j['kind']: j['status'] for j in self.jobs()}
        self.assertEqual(states['registration'], 'cancelled')
        self.assertEqual(states['reminder'], 'cancelled')
        self.assertEqual(states['survey'], 'pending')
        self.assertEqual(self.db.execute(query('claim')).fetchone()['kind'], 'survey')

    def test_cancel_retires_a_survey_once_its_event_is_no_longer_addressable(self):
        self.register()
        self.db.execute("UPDATE attendees SET checked_in_at='now' WHERE id='a'")
        self.complete()
        self.db.execute("UPDATE events SET status='archived' WHERE id='event-a'")
        self.db.execute(query('cancel'))
        self.assertTrue(all(j['status']=='cancelled' for j in self.jobs()))

    def test_survey_leaves_the_inbox_when_the_form_stops_accepting(self):
        self.register()
        self.db.execute("UPDATE attendees SET checked_in_at='now' WHERE id='a'")
        self.complete()
        self.assertIn('survey', self.visible_kinds())
        self.db.execute("UPDATE events SET post_event_registration_until_ms=? WHERE id='event-a'",
                        ((self.now-60)*1000,))
        self.assertNotIn('survey', self.visible_kinds())
        self.db.execute("UPDATE events SET post_event_registration_until_ms=NULL,post_event_registration_open=0 WHERE id='event-a'")
        self.assertNotIn('survey', self.visible_kinds())

    def test_my_registrations_is_one_indexed_user_scoped_query(self):
        self.register()
        self.register('b', 'event-b')
        sql = (HANDLER_SQL / 'my_registrations.sql').read_text()
        rows = self.db.execute(sql, ('A@EXAMPLE.COM',)).fetchall()
        self.assertEqual([row['attendee_id'] for row in rows], ['a'])
        plan = self.db.execute('EXPLAIN QUERY PLAN ' + sql, ('a@example.com',)).fetchall()
        self.assertTrue(any('idx_attendees_email_nocase' in row['detail'] for row in plan))

    def test_live_dashboard_metrics_share_one_statement(self):
        self.register()
        self.db.execute("UPDATE attendees SET checked_in_at='now',claim_asset_id='asset' WHERE id='a'")
        self.deposit(verified=1)
        self.db.execute("UPDATE deposit_statuses SET method='usdc',currency='USDC' WHERE attendee_id='a'")
        sql = (DB_SQL / 'dashboard_live_metrics.sql').read_text()
        row = self.db.execute(sql, ('event-a',)).fetchone()
        self.assertEqual(dict(row), {
            'registered': 1,
            'checked_in': 1,
            'claims_minted': 1,
            'deposits_verified': 1,
            'usdc_locked_total': 100,
        })

    def test_concurrent_dispatchers_claim_different_jobs(self):
        self.register()
        self.db.commit()
        with tempfile.TemporaryDirectory() as directory:
            filename=str(Path(directory)/'outbox.sqlite')
            disk=sqlite3.connect(filename)
            self.db.backup(disk)
            disk.close()
            barrier=threading.Barrier(2)
            results=[]
            errors=[]
            def claim():
                try:
                    conn=sqlite3.connect(filename,timeout=10)
                    barrier.wait()
                    with conn:
                        results.extend(conn.execute(query('claim')).fetchall())
                    conn.close()
                except Exception as error:
                    errors.append(error)
            threads=[threading.Thread(target=claim) for _ in range(2)]
            for thread in threads:thread.start()
            for thread in threads:thread.join()
            self.assertEqual(errors,[])
            self.assertEqual(len(results),1)

if __name__=='__main__':unittest.main()
