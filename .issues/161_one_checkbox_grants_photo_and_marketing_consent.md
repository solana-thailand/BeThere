# 161: One registration checkbox also grants photo and marketing consent

**Status:** deployed to prod `f02143d4` (2026-09-29, `deploy/production/20260929T050434Z`, session `event-checkin-1b`; staging runs the same tree as `bb8ac906`). Was: fixed on develop — `0d230002`, merged via `604cd993` on 2026-09-29 (owner chose plan 028 option A) and on staging `53de706d`. Checked there (session `event-checkin-f6`, headless Chrome, `/e/slipdemo-1790494035` signed in): the form shows three separate boxes (privacy + deposit, photo, marketing email), all unticked. Prod ships with the next owner-gated deploy. Still the owner's call: clearing the marketing consent already recorded (see "Old consent records"). Found by session `event-checkin-af` while correcting `/privacy` for `.issues/158`. Seen alongside: `.issues/166`.

## What happens

The public registration form has a single checkbox: "I agree to the Privacy
Policy and authorize the … commitment deposit". Its `on:change` sets four
signals at once (`frontend-leptos/src/pages/public_event/registration_form.rs:406-410`):
`consent_given`, `deposit_agreed`, `photo_consent_given` and
`consent_marketing`. So ticking the one box required to register also records
photo consent and marketing consent, and the label mentions neither.

`/privacy` said "a separate photo consent checkbox will appear". No such
checkbox exists. The page was corrected on 2026-09-28 (`.issues/158`) to say
the registration checkbox includes photo consent.

## Why it matters

PDPA s.19 asks for consent that is separate from other terms, specific, and
not a condition of the service unless necessary. Marketing consent is the
clearest problem: it is not needed to register, yet it is recorded for
everyone who registers. Email sending is off today (`NOTIFICATIONS_ENABLED = "0"`),
so nothing is sent on it yet, but the consent record itself is wrong.

## Fix

Separate, unticked, optional checkboxes: marketing always optional; photo
optional unless the event sets `require_photo_consent`, and then labelled as
required. Re-check `signup.rs:174` and the contacts and Sheet consent columns.
Decide what to do with the marketing consent already recorded (treat it as not
given until re-asked). After RTM #6, with a staging rehearsal.

## Done on `feature/161-separate-consent` (session `event-checkin-0d`)

- `registration_form.rs`: the required box now sets only `consent_given` and
  `deposit_agreed`. Photo and marketing each get their own unticked box.
  Photo is labelled "(required by this event)" and blocks submit when the
  event sets `require_photo_consent`, else "(optional)". Marketing is always
  optional. `signup.rs:174` already enforced the photo rule server-side.
- `/privacy` sections 2, 3 and 6 describe the separate boxes and the
  opt-in email purpose. `docs/pdpa_ropa.md` §3 and §16 item 2 updated.
- Verified by opening the page (local `wrangler dev`, no Google credentials,
  POST intercepted so nothing was written):
  - required event: three unticked boxes; ticking the main box leaves the
    other two unticked; submit is blocked with "This event requires photo
    consent"; with both ticked the body carries `photo_consent_given: true,
    consent_marketing: true`.
  - optional event: submit goes through with only the main box; body has no
    `photo_consent_given` and `consent_marketing: false`.
- Frontend clippy (wasm32, `-D warnings`) and all 25 test binaries pass.

## Old consent records (owner decision)

`consent_marketing = 1` on `attendees` rows written before this fix is not
valid consent. The same form also wrote `developer_profiles.consent_outreach`
(`register/contact.rs:127`), but that column is also set by the profile page
(`handlers/profile.rs:205`), where the opt-in is real. So a blanket reset would
erase valid consent. Options:

1. Reset `attendees.consent_marketing` to NULL for rows with
   `consent_marketing_at` before the deploy of this fix, and leave
   `consent_outreach` alone (profile opt-ins survive; form-only opt-ins stay
   wrong).
2. Also reset `consent_outreach` to 0 where it was never saved from the
   profile page. There is no column that says which path wrote it, so this
   means resetting everyone and re-asking.

Nothing sends on either column today (`NOTIFICATIONS_ENABLED = "0"`), so this
can wait for the merge. It must be settled before email sending is turned on.
