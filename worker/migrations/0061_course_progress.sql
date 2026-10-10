-- 0061_course_progress.sql — .plans/045 R4.4: courses ("register once, watch
-- episode by episode").
--
-- A course is a series of past events (domain::models::catalogue: course
-- slug + episode = event slug). Registering is a sign-in and one click; the
-- row is the enrolment. Progress is what the person marked as watched.
--
-- .issues/068: watching a recording is never check-in, attendance, a deposit
-- or badge eligibility. These two tables are read and written only by
-- worker/src/courses.rs, and nothing there touches attendees, deposits,
-- check-ins or mints (guarded by worker/tests/courses_guard.rs).
--
-- Keyed by the signed-in email (lowercased). PDPA erasure deletes both.
CREATE TABLE IF NOT EXISTS course_enrolments (
    email TEXT NOT NULL,
    course TEXT NOT NULL,
    enrolled_at TEXT NOT NULL,
    PRIMARY KEY (email, course)
);

CREATE TABLE IF NOT EXISTS course_progress (
    email TEXT NOT NULL,
    course TEXT NOT NULL,
    episode TEXT NOT NULL,
    watched_at TEXT NOT NULL,
    PRIMARY KEY (email, course, episode)
);
