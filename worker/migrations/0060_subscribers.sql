-- 0060_subscribers.sql — .plans/045 R4.12: "email me when the next event
-- opens", stored in D1 and sent from bethere.sol@gmail.com (owner, 2026-10-08:
-- the free Gmail API path; memory subscribe-email-sender).
--
-- subscribers: one row per address. `consent_at` is when they asked (the
-- form's consent line, shown next to the button); a later re-subscribe after
-- an unsubscribe moves it. `unsub_token` is the one-click unsubscribe secret
-- in every mail (random, 32 hex bytes). Unsubscribing keeps the row with
-- `unsubscribed_at` set, so a resend never mails them; PDPA erasure deletes it.
--
-- announced_events: when the announcer first saw each public event open.
-- A subscriber gets an event's mail only if they subscribed before that, so
-- signing up never mails about what was already open on the page.
--
-- event_announcements: one row per (event, subscriber), written BEFORE the
-- send (the idempotency key: one mail per event per subscriber). `sent_at`
-- stays NULL while a send is in flight or ended ambiguously; such a row is
-- never retried automatically (the notifications rule: no blind replay).
CREATE TABLE IF NOT EXISTS subscribers (
    email TEXT PRIMARY KEY,
    locale TEXT NOT NULL DEFAULT 'en' CHECK (locale IN ('en', 'th')),
    consent_at TEXT NOT NULL,
    unsub_token TEXT NOT NULL UNIQUE,
    unsubscribed_at TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS announced_events (
    event_id TEXT PRIMARY KEY,
    announced_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS event_announcements (
    event_id TEXT NOT NULL,
    email TEXT NOT NULL,
    claimed_at TEXT NOT NULL,
    sent_at TEXT,
    PRIMARY KEY (event_id, email)
);

-- What is open today was never news to a subscriber: mark it seen, so
-- turning the announcer on mails nobody.
INSERT OR IGNORE INTO announced_events (event_id, announced_at)
SELECT id, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM events
WHERE status = 'active' AND visibility = 'public';
