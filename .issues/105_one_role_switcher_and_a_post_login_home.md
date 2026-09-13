# 105 — Two role switchers, and a post-login deep link to the wrong event

**Status:** implemented 2026-09-14, **not deployed**
**Found:** 2026-09-14, reviewing the landing page against the live DOM
**Severity:** medium — one is cosmetic, one sent repeat attendees to an April ticket

## 1. Two switchers, one axis, disagreeing about how many roles exist

Read off production:

```
roleToggles: ["For Attendees", "For Organizers",
              "I am an Attendee", "I am an Organizer", "I am Event Staff"]
```

A hero pill with two options and a tab row in "How it works" with three, on
separate signals (`persona`, `feature_tab`) with a one-way `Effect` syncing the
first into the second. A reader who picked a side at the top had to pick again
800px later, and the two did not agree that Staff exists.

Now one switcher. The hero pill carries all three and the "How it works" section
follows it; `feature_tab` is gone.

Staff also got hero copy of its own rather than falling through to the organizer
branch — the previous `if persona == 0 { attendee } else { organizer }` would
have sold a door scanner on no-show payouts.

## 2. `/discover` is the home after sign-in

An attendee landing on the marketing page has just read the marketing page. The
three things they came for were below hero, stat tiles and the pitch.

**This needed the header first.** `.issues/100` recorded it as the blocker and it
was real: the nav lived inline in `pages/landing/page.rs`, so redirecting to a
page without it would have stranded people with no menu, no profile and no way
to sign out. It is now `pages/landing/nav.rs::SiteHeader`, used by both. The
in-page anchors became absolute (`/#faq`) so they navigate home and scroll
instead of doing nothing.

Auth state is a prop, not a fetch: the landing page already loads it for its
dashboard button, and a self-fetching header would double every page's calls to
`/auth/me`. `/discover` reads it once through the **non-redirecting**
`fetch::get` — `api_get` and `get_me` end a 401 with
`redirect_to_login_expired()`, which is exactly how `.issues/099` happened.

### The deep link that was removed, and why

Post-login, an attendee was sent to `regs.first()`, in a branch whose own log
line called it *"the latest registration"*.

`my_registrations.sql` orders `event_start_ms ASC`. **`.first()` is the oldest.**
A repeat attendee signing in was taken to the ticket for an event from April.
Nobody noticed because until this week most people had one registration; the
organizer has eleven.

`/discover` is what that heuristic was reaching for and gets right — every
registration, soonest-first for what is coming, most-recent-first for what is
done, with the same next-step link on each row. So the branch is gone rather
than repaired.

Organizers and staff keep `/admin` and `/staff`. An explicit `?next=` still wins.

## Not verified

Both changes are visual and the signed-in half needs a session. Compiled,
linted, tested, and to be checked on deployed staging before prod — but the
post-login redirect itself cannot be exercised without signing in.

## Related

- `.issues/100` — recorded the missing header as the blocker for exactly this.
- `.issues/096` — the page.
- `.issues/094` — the landing observations this closes two of.
