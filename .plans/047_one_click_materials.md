# 047 — Past-event materials and one-click registration

**Status:** Phase 1 deployed to prod (2026-10-17, session `event-checkin-ef`). Phase 2 not started.

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
- [ ] Owner: add slides to RTM #1–#5 (all have a recording; only RTM #6 has
      materials, 6 links) and link each event page from its YouTube video.

## Phase 2 (not started)

- Live-event registration form: prefill from the server profile, not only
  localStorage `bt_dev_profile`; one click when the event asks nothing the
  profile lacks (participation type, deposit and custom `form_config` fields
  make this a larger change).
- Organizer side: a "slides" field per past event is already in the event form
  (Community links → type `slides`); fill it for RTM #1–#6.

## Notes

- Opening post-event registration fires `notification_post_event_survey` (a
  survey row for checked-in attendees). Prod notifications are off, but it
  shows in the in-app inbox. An event with a `video_url` accepts post-event
  registration even with the toggle off.
- Re-registering is safe: `upsert_post_event_attendee` keeps an existing live
  registration's type and status.
