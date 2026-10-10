"""Courses (.plans/045 R4.4): the worker's real SQL (src/courses.rs) on the
production migrations. Registering is idempotent, progress needs a
registration, and erasure leaves nothing."""

from pathlib import Path
import sqlite3
import unittest

from test_person_emails import plain_after

WORKER = Path(__file__).resolve().parents[2]
SRC = "courses.rs"
ENROL = plain_after(SRC, "pub const ENROL_SQL")
WATCHED = plain_after(SRC, "pub const WATCHED_SQL")
ENROLLED = plain_after(SRC, "pub const ENROLLED_SQL")
PROGRESS = plain_after(SRC, "pub const PROGRESS_SQL")
_src = (WORKER / "src" / SRC).read_text()
ERASE = [
    line.strip().strip('",')
    for line in _src[_src.index("pub const ERASE_SQL") :].split("];")[0].splitlines()[1:]
    if line.strip().startswith('"')
]
RTM = "road-to-mainnet"
EP1 = "solana-x-ai-builders-the-road-to-mainnet-1-bangkok"
EP2 = "solana-x-ai-builders-the-road-to-mainnet-2-bangkok"


class CourseTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())

    def tearDown(self):
        self.db.close()

    def watched(self, email):
        return [r[0] for r in self.db.execute(PROGRESS, (email, RTM))]

    def test_progress_needs_a_registration(self):
        n = self.db.execute(WATCHED, ("a@example.com", RTM, EP1, "2026-10-10T10:00:00Z")).rowcount
        self.assertEqual(n, 0, "not registered: nothing recorded")
        self.assertEqual(self.db.execute(ENROL, ("a@example.com", RTM, "2026-10-10T10:01:00Z")).rowcount, 1)
        self.assertEqual(self.db.execute(ENROL, ("a@example.com", RTM, "2026-10-10T10:02:00Z")).rowcount, 0)
        self.assertEqual(self.db.execute(ENROLLED, ("a@example.com", RTM)).fetchall(), [(1,)])
        for ep, minute in [(EP2, 3), (EP1, 4), (EP1, 5)]:
            self.db.execute(WATCHED, ("a@example.com", RTM, ep, f"2026-10-10T10:0{minute}:00Z"))
        self.assertEqual(self.watched("a@example.com"), [EP2, EP1])
        # another course's registration does not count here
        self.db.execute(ENROL, ("b@example.com", "solana-in-latent-space", "2026-10-10T10:00:00Z"))
        self.db.execute(WATCHED, ("b@example.com", RTM, EP1, "2026-10-10T10:06:00Z"))
        self.assertEqual(self.watched("b@example.com"), [])

    def test_erasure_leaves_nothing(self):
        self.db.execute(ENROL, ("a@example.com", RTM, "2026-10-10T10:00:00Z"))
        self.db.execute(WATCHED, ("a@example.com", RTM, EP1, "2026-10-10T10:01:00Z"))
        self.db.execute(ENROL, ("b@example.com", RTM, "2026-10-10T10:00:00Z"))
        self.assertEqual(len(ERASE), 2)
        for sql in ERASE:
            self.db.execute(sql, ("a@example.com",))
        for table in ("course_enrolments", "course_progress"):
            n = self.db.execute(f"SELECT COUNT(*) FROM {table} WHERE email = 'a@example.com'").fetchone()[0]
            self.assertEqual(n, 0, table)
        self.assertEqual(self.db.execute(ENROLLED, ("b@example.com", RTM)).fetchall(), [(1,)])


if __name__ == "__main__":
    unittest.main()
