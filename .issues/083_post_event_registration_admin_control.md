# 083 — The post-event registration flag had no UI and could only be set by hand

**Status:** fixed 2026-09-13, **deployed** to prod `253717a5` — see `.issues/090`
**Found:** 2026-09-13, planning the DevRel phase-2 event flips
**Severity:** medium (a shipped feature was unreachable)

## What

`PUT /api/events/{id}/post-event-registration` has existed since Plan 008
Phase 3. Its frontend API client, `put_post_event_registration`
(`frontend-leptos/src/api/event/post_event.rs:65`), has existed just as long.

**Nothing ever called it.** The function is defined, re-exported through
`api/event/mod.rs`, and referenced from exactly zero call sites.

So `post_event_registration_open` could only be set by hand-crafting an
authenticated `PUT`. It is `0` on all 16 production events.

## Why it matters

DevRel have 13 recap posters whose QR codes point at completed events. The
standalone form at `/events/{slug}/post-event-register` is the page those codes
should land on — and `register_post_event` returns **409** until the flag is
open (`handlers/register/post_event.rs:75`).

So the feature was built, tested, shipped, and unreachable. Asking an operator
to hand-craft a `PUT` per event is not a workflow, and it bypasses the audit
trail's intent even though the handler writes one.

## What changed

`frontend-leptos/src/pages/post_event_panel.rs` — a `PostEventRegistrationPanel`
component, mounted in the event form beside the Status selector, because the two
are used together: an event is marked Completed and then opened for
retrospective sign-ups.

- Open/close button plus an optional closing date.
- Only enabled when status is Completed; otherwise it explains why, rather than
  offering a control the server would reject.
- Reads `status` reactively from the form signal, so it appears as soon as the
  organizer picks "Completed" — not only after a save.
- Validates the deadline client-side against the same rule the handler applies
  (must be in the future), so the organizer is told before the round trip.
- Date handling uses `js_sys::Date` like the rest of the crate — the crate has
  no `chrono`, and the organizer's local day is the day they mean.

`EventDetail` gained `post_event_registration_open` and
`post_event_registration_until_ms`. The Worker already serialized both (the
admin detail endpoint returns the whole `EventConfig`); the frontend mirror
simply did not declare them, which is why the panel had nothing to read.

## Deliberately not in `EventForm`

The flag is written **only** by the dedicated endpoint, which refuses to open it
on a non-Completed event. `UpdateEventRequest` has no such field, so the generic
form save cannot touch it. Routing the toggle through the form would have
created a second writer with no guard — the exact shape of
`duplicated-state-transition-paths`. The panel therefore owns its own state and
seeds it with its own `get_event_detail` call.

## Verification

- `cargo clippy` clean on **both** targets (`wasm32-unknown-unknown` and
  native `--all-targets`) plus the workspace.
- Frontend suite green, including the SSOT mirror audit that guards these
  mirror structs against the domain.
- **The CSS class audit caught a real mistake**: the first draft used
  `quiz-field-hint`, which does not exist in any stylesheet, so the hint text
  would have rendered unstyled. Corrected to `quiz-setting-hint`
  (`styles/style-06-admin.css:156`). That guard earned its keep.

Not exercised in a browser — this needs the deploy, and then the 12 event flips
it exists to enable.

## Noticed, not fixed

The frontend crate has accumulated `rustfmt` drift in three files
(`pages/deposit/types.rs`, `pages/landing/page.rs`, `utils/mod.rs`). CI's
`cargo fmt --check` runs at the workspace root and `frontend-leptos` is not a
workspace member, so nothing gates it. Reverted out of this change to keep the
diff honest; worth a separate pass, together with adding the crate to the CI
fmt step.

## Related

- DevRel `reports/phase-2/BETHERE-REPLY.md` items 1 and 2.
- `.issues/068_retrospective_learning_hub.md` — what the flag unlocks.
