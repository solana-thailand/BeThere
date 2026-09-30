# Plan 032: Handle a postponed event without spreadsheets on the side

**Created:** 2026-09-26, session `event-checkin-b9`.
**Trigger:** RTM#6 was postponed from 27 Sep to 4 Oct 2026 because of
flooding. The organizer re-asks every in-person registrant whether they can
still come. The answers are coming, not sure yet, or can't come. People who
can't come are moved to online and refunded later.
**Owner picks (2026-09-26):** 1, 3, 4 now; 2 not chosen.

## 0. Found first

- [x] `.issues/152`: a date change never reached D1. The trigger was fixed
  (migration 0051) and applied to prod. The owner re-saved RTM#6 at 08:18 UTC,
  and D1 and the public list now read 4 Oct.

## 1. Record each registrant's answer (develop)

- [x] Migration `0052_attendance_answers.sql`, a table of its own, so no
  `attendees` writer can clobber it.
- [x] `AttendanceAnswer` (domain). `db::attendance_answers` (SQL in
  `db/sql/attendance_answer_{set,clear}.sql`). A write lands only on an
  attendee of the named event.
- [x] `PUT /api/attendee/{id}/attendance-answer` in the authed router, with
  the audit action `AttendanceAnswerRecorded`.
- [x] Roster: `attendance_answer` on `AttendeeListItem` (a 4th batch
  annotation), a per-row picker, an answer filter bar with counts, and a CSV
  column. UI in `pages/admin_attendance_answer.rs`.
- [x] Staging, 2026-09-26 (`3fa0eb4d`), in the browser:
  - picker and filter pills render;
  - saving "Can't come" updates the counts and survives a refresh;
  - the filters show the right rows;
  - clearing works;
  - a write for another event's attendee → 404, a bad value → 422.

## 3. Refund Queue filter (develop)

- [x] `RefundQueueResponse.context` gives participation, check-in and answer
  per queued attendee (`db/sql/refund_queue_context.sql`). Best-effort.
- [x] Filter pills: All · Moved online · Can't come · Not checked in. A row
  without context passes only "All". UI in
  `pages/admin_refund_queue_filter.rs`.
- [x] Staging: pills render with counts; "Can't come" shows the queued row;
  "Moved online" and "Not checked in" hide it (a checked-in walk-in). The
  "Moved online" positive case was not exercised in the browser, because the
  participation switch 500s on a fixture without a real Sheet (see Later);
  it is covered by the SQL and domain tests.

## 4. Postponed notice (develop, `58dc161c`)

- [x] `postponed_note` on the event (migration 0053). Empty means not
  postponed. The event stays `Active`, so registration, check-in and credit
  keep working. A new `EventStatus` was rejected because signup and the
  public list are gated on `Active`.
- [x] Event form field in the "Ticket Announcements" section. It is capped at
  500 characters by `normalize_postponed_note`, which shares its body with
  the ticket-note normalizer.
- [x] Banner on the public event page (live events) and above both ticket
  views. Badge on the landing list and `/discover`. The note is rendered as
  plain text only. `EventMeta` carries the note for the KV fallback of the
  list.
- [x] Duplicate does not carry the note forward.
- [x] Staging:
  - the form field saves (Thai text and a newline reach D1);
  - the banner shows on `/e/{slug}` and on the ticket;
  - `<b>` renders as literal text (no element);
  - registration stays open;
  - exactly one "Postponed" badge on the landing list.
- [x] Deploy order: migration 0053 **before** the code, because the D1-first
  public list selects `postponed_note` and fails without the column.
  Done (2026-09-29, `event-checkin-8c`): `wrangler d1 migrations list bethere-db --remote` has nothing to apply, the prod `events` schema has `postponed_note`, 0053 is in the prod tree `f02143d4`, and prod `/api/public/events` answers 200 `application/json`.

## Later / not chosen

- [ ] 2. Attendees answer on their own ticket page ("can't come" switches
  them to online and lists them for a refund).
  **Blocked:** owner; option 2 was not chosen, so it is a product decision.
  **Owner question (2026-09-30, `event-checkin-aa`):** "should attendees of a
  postponed event be able to answer "can't come" on their own ticket page,
  which switches them to online and puts them on the refund list, or does
  option 2 stay unchosen?"
- [x] A failed D1 dual-write on event save (`sync_event_to_d1`) should reach
  the admin who saved, not only a log line (`.issues/152`, "Not done").
  Done on develop 2026-09-26 as a response `warnings` entry and warning toast,
  then extended to the poster, escrow confirm-init and duplicate paths;
  not deployed.
- [x] "Batch THB refund" (cancel page) refunded every verified deposit with no
  proof and skipped D1 `attendees.mark_refund`. Guarded on develop 2026-09-26:
  the worker requires a receipt (same validator as the single refund) and
  writes D1 attendees per refund; the page needs the receipt link and a
  confirm. Guard: `worker/tests/refund_proof_link_guard.rs`. Not deployed.
  The batch now also writes refund_link to the Sheet (`c323440b`).
- Staging, 2026-09-27 (deploy `a941d6bb`, git `c323440b`, session
  `event-checkin-2e`):
  - cancel page in headless Chrome (`flow-test-event`): the batch button is
    disabled with an empty field and with a `javascript:` link, enabled for
    an https link; the confirm shows the pending count and the link;
    dismissing it refunds nothing;
  - API: batch with no proof → "refund_proof_url is required", with a
    `javascript:` proof → the https error;
  - create → `warnings: []`; duplicate keeps its sheet warning; archive and
    restore return `name`. Probe events deleted.
  - Not exercised: a real failed D1 write (no way to force one), and the
    toasts themselves (the chrome-devtools profile was held by a peer).
  - Found: duplicating an event with no Sheet ID returns 500 "internal error".
    `create_event` rejects it ("google sheet_id is required"), and the
    duplicate and create handlers map every error except "already exists" to
    Internal. Predates this work. Fixed on develop 2026-09-27: `create_event`
    returns `EventWriteError` (Invalid → 400 with the reason, Storage → 500);
    test `worker/tests/event_create_errors.rs`. Not deployed.

- [x] Participation switch (`PATCH /attendee/{id}/participation-type`) returned
  500 when the event's Sheet could not be read. Fixed in `.issues/153`
  (`445c1eb5`): an unreadable Sheet now degrades to a D1-only update. Staging
  verified on 2026-09-26 (`d1_updated: true, sheet_row_updated: false`).
  Reading it also turned up a cross-event write by id, fixed in the same issue.

## Deploy (owner-gated)

Apply migrations 0052 and 0053 before the code. Build the frontend. Then
`worker/deploy.sh` with an owner go.

- Staging done 2026-09-26: migrations 0052 and 0053 applied, deploy
  `20260926T101341Z` = git `3fa0eb4d`, Content-Type, security headers and
  write smoke all green.
- Prod done 2026-09-26 (session `event-checkin-3f`, owner go in session):
  D1 backup `backup-prod-20260926-1821.sql` (gitignored), migrations 0052 and
  0053 applied and read back, prod `f3b32edf`, git `70a36f3e`, deploy tag `20260926T122155Z`.
  Content-Type and security headers green; `postponed_note` is in
  `/api/public/events`. Preflight bypassed with `--force` (fixtures missing,
  `.issues/084`/141). The write smoke on prod is owed (no `SMOKE_TOKEN`).
- Frontend first load is +36213 B over the baseline: above the 25600 warn
  line, under the fail line.
- Worker is 51.75% of the 3 MiB free-plan ceiling (+53819 B since
  2026-09-22).
- Staging 2026-09-27 (session `event-checkin-d9`): version `33a0775e` = git
  `4ddb6439`, which carries the Later items and `a22f749d`. Verified: a
  create refused for an empty Sheet ID or a backwards date returns 400 with
  the reason (`validation error: google sheet_id is required`). Not on prod.
