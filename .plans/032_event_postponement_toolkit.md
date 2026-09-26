# Plan 032: Handle a postponed event without spreadsheets on the side

**Created:** 2026-09-26, session `event-checkin-b9`.
**Trigger:** RTM#6 was postponed from 27 Sep to 4 Oct 2026 because of
flooding. The organizer re-asks every in-person registrant whether they can
still come. The answers are coming, not sure yet, or can't come. People who
can't come are moved to online and refunded later.
**Owner picks (2026-09-26):** 1, 3, 4 now; 2 not chosen.

## 0. Found first

- [x] `.issues/152`: a date change never reached D1. The trigger was fixed
  (migration 0051) and applied to prod. RTM#6's D1 row needs one more save.

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
- [ ] Open the admin page on staging and record, change and clear an answer.

## 3. Refund Queue filter (develop)

- [x] `RefundQueueResponse.context` gives participation, check-in and answer
  per queued attendee (`db/sql/refund_queue_context.sql`). Best-effort.
- [x] Filter pills: All · Moved online · Can't come · Not checked in. A row
  without context passes only "All". UI in
  `pages/admin_refund_queue_filter.rs`.
- [ ] Verify on staging with a switched attendee who has a cash deposit.

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
- [ ] Open on staging: form, public page, ticket, landing badge.
- [ ] Deploy order: migration 0053 **before** the code, because the D1-first
  public list selects `postponed_note` and fails without the column.

## Later / not chosen

- [ ] 2. Attendees answer on their own ticket page ("can't come" switches
  them to online and lists them for a refund).
- [ ] A failed D1 dual-write on event save (`sync_event_to_d1`) should reach
  the admin who saved, not only a log line (`.issues/152`, "Not done").
- [ ] "Batch THB refund" (cancel page) refunds every verified deposit with no
  proof and skips D1 `attendees.mark_refund`. Guard it or retire it.

## Deploy (owner-gated)

Apply migrations 0052 and 0053 before the code. Build the frontend. Then
`worker/deploy.sh` with an owner go.
