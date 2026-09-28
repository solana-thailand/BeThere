# 161: One registration checkbox also grants photo and marketing consent

**Status:** open (2026-09-28). Found by session `event-checkin-af` while correcting `/privacy` for `.issues/158`. Changing the consent flow is the owner's call and touches the RTM #6 registration path, so nothing is changed yet.

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
