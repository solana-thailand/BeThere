# 047 — Past-event materials and one-click registration

**Status:** Phase 1 deployed to prod (2026-10-17, session `event-checkin-ef`). Phase 2 deployed to prod 2026-10-25: live-form prefill (pull 286) and one click on the live form (pull 296), release pull 297.

## Why

Owner 2026-10-17: someone catching up on past events from YouTube asks for
each event's slides. Point them at the event on BeThere instead. The owner
also wants one central profile, so registering for an event is a click.

## Decisions (owner: "continue from your suggestion", 2026-10-17)

- **Slides stay public.** They already are (the public event payload and the
  recap carry `community_links`), and gating them would need the recap and the
  public payload changed. Registering is "keep it in your record", not a
  paywall.
- **First registration asks for name + contact only.** Experience, stack,
  interests and the satisfaction questions stay optional on the full form.

## Flow

```
YouTube description → /e/{slug} (ended page: recording + "watch" button)
  → Google sign-in
  → name on file?  yes → one button "Agree and watch"   (Phase 1)
                   no  → the form, name + contact required
  → /ticket/{attendee_id}?event_id=… : recording (VideoSection) + slides
    (resources_section), which already render for `retrospective` attendees
```

## Phase 1 (this branch)

- [x] `GET /api/my-profile` returns `contact_name`, `contact_channel`,
      `contact_handle` from `contacts`, read by the caller's own email only
      (`db::developers::get_contact_card`; a read error means no prefill).
- [x] `POST …/register-post-event` answers with `event_id`.
- [x] Post-event page: prefills name and contact from the server profile
      (not stack/interests: arrays on the profile, strings on this form); a
      known name gets the one-button card, "edit my details first" opens the
      form; success hard-navigates to the ticket.
- [x] Tests: `frontend-leptos/tests/post_event_one_click.rs` (7),
      `worker/tests/profile_prefill_guards.rs` (4).
- [x] The watch-back ticket: post-event registrants (`retrospective`) get a
      "Watching back / Recording and materials" hero, no timeline, no claim or
      calendar cards (they can never check in or claim).
- [x] Verified by opening the page on a local fixture worker (EN + TH):
      ended page → one-button card → ticket with the recording and both
      resource links; with no profile on file the form shows instead; a
      second visit lands straight on the same ticket.
- [x] Staging (pull 283, `bb072770`), smoke passed.
- [x] Ticket order on ended events (owner "continue from your suggestion",
      2026-10-17): what you can still do, then the recording and slides, then
      history. In person was already right (the claim card leads the main
      card). Online: the timeline moves below the materials once the event is
      over. One `materials()` block renders the recording and resources in both
      views (`tests/ticket_materials_order.rs`). Checked on the fixture worker:
      in-person and online ended tickets, claim card first.
- [x] Prod (owner go 2026-10-17): release pull 285, `aec56c53`, version
      `241e02a8`, 16:36 UTC, after backup `backup-prod-20261017-2331.sql`.
      Checked read-only: content types, headers, `/api/my-profile` 401
      unauthenticated, RTM #5 ended page shows the watch button and sends a
      guest to sign-in. The write smoke was skipped: the smoke token had
      expired.
- [x] Slides added to RTM #1–#5 on prod via the admin API (2026-10-18): 7 decks
      (Katopz #1–#5, Chaerin #1, Golf #4). Rollback snapshot kept in
      `~/bethere-backups/`.
- [ ] Owner: still missing Golf (#1 NFT Engine, #2 pay.sh x402), Luke (#3),
      Gui Bibeau (#5), Andy (#1); link each event page from its YouTube video.

## Phase 2 (not started)

- [x] Live-event registration form prefills name and contact from the server
  profile (pull 286, prod 2026-10-25).
- [x] One click on the live form (owner rule 2026-10-25): offered only when the
  event has no deposit, no required custom field and one way to attend; never
  for wallet-only sessions or when photo consent is required; only with a name
  on file (and the contact when the event requires one). `one_click_eligible`
  in `public_event/types.rs`; both buttons share one submit closure; the full
  form stays in the DOM, prefilled, hidden with `display:none` (`contents`
  otherwise, so the form's layout is unchanged) and one bot-check slot.
  Tests `tests/live_one_click.rs` (5). Checked on a local fixture worker: a
  deposit event shows the form; a free event shows the card (EN + TH); "edit my
  details first" shows the prefilled form; "agree and register" lands on the
  ticket; 0 page errors. `consent_given` goes in the same request body as the
  form's checkbox (it is recorded on the Sheet, not in D1).
- [x] Prod (owner go 2026-10-25): release pull 297, `4e222285`, version
  `22f7689a`, 11:54 UTC, after backup `backup-prod-20261025-1836.sql`. First
  release with the health build stamp: `/api/health` reports
  `git:4e222285…`. Checked: staging parity, content types, security headers,
  RTM #5 ended page sends a guest to sign-in. Write smoke skipped: the smoke
  token had expired (`.issues/195`).
- Organizer side: a "slides" field per past event is already in the event form
  (Community links → type `slides`); fill it for RTM #1–#6.

- [x] Prod (owner go 2026-10-25): release pull 288, `01f7effa`, version `4359e027`, 05:47 UTC, after backup `backup-prod-20261025-1241.sql`; same release carried the tokio RUSTSEC-2026-0386 fix (pull 290). Checked: staging parity, content types, security headers, the shipped wasm carries the new text, RTM #5 ended page sends a guest to sign-in. Write smoke skipped: the smoke token had expired.

## Notes

- Opening post-event registration fires `notification_post_event_survey` (a
  survey row for checked-in attendees). Prod notifications are off, but it
  shows in the in-app inbox. An event with a `video_url` accepts post-event
  registration even with the toggle off.
- Re-registering is safe: `upsert_post_event_attendee` keeps an existing live
  registration's type and status.
