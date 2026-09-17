-- Linked emails: several emails, one person (plan 025, issue #122).
--
-- Email is the identity key everywhere, so one attendee who registers with a
-- work email cannot see or spend the rolling credit held under their personal
-- email. A row here links an email to a person. An email with no row is
-- implicitly its own person, so this table changes nothing until someone links.
--
-- Only credit reads and the spend guard resolve over the linked set (owner
-- choice 2026-09-18: shared credit only). Sessions keep the email used to log
-- in, and ledger rows stay per email, so no history is rewritten.
--
-- `proof` records how ownership of the email was established: `google` means a
-- Google sign-in for that email inside a session already signed in as the
-- person. Nothing writes `admin` yet: an admin override is an open owner
-- decision (plan 025 §6.1). It is allowed by the CHECK now because widening a
-- CHECK later means rebuilding the table in SQLite.
CREATE TABLE IF NOT EXISTS person_emails (
    email       TEXT PRIMARY KEY,           -- lowercased
    person_id   TEXT NOT NULL,              -- UUIDv7
    is_primary  INTEGER NOT NULL DEFAULT 0, -- the email the person linked from
    proof       TEXT NOT NULL CHECK (proof IN ('google', 'admin')),
    linked_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_person_emails_person ON person_emails(person_id);

-- Exactly one primary per person.
CREATE UNIQUE INDEX IF NOT EXISTS idx_person_emails_one_primary
    ON person_emails(person_id) WHERE is_primary = 1;
