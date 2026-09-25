# 104 — The same date bug, written four times

**Status:** fixed 2026-09-14, deployed (in prod tag `deploy/production/20260925T032335Z`; `issue_ledger.py` 2026-09-25 found every linked commit there)
**Found:** 2026-09-14, from the organizer's screenshot of "Your Events"
**Severity:** low individually — the point is the pattern

## What

`8/24/2026, 1:00:00 PM` in the landing page's "Your Events" list. Seconds on an
event date, and a month-first order that is ambiguous to the Thai-majority
audience this is written for.

It is `js_sys::Date::to_locale_string` called with **no options**, which renders
the browser default. That exact call had been written four times:

| surface | state |
|---|---|
| landing "Upcoming Events" card | fixed 2026-09-13 (`.issues/094`) |
| landing **"Your Events"** list | **this** |
| `/discover` date chip | **this** — written by me, days later, with the same mistake |
| `/feedback` blocks | already used the helper |

The first was found by looking at a screenshot. The second by looking at another
screenshot. **The third I wrote myself after fixing the first** — which is the
whole argument for not fixing this one by hand either.

## The fix

Both now call `utils::format_event_datetime` / `format_event_day_parts`.

And a guard: `tests/date_formatting_has_one_home.rs` walks every `.rs` under
`src/` and fails if `to_locale_string` appears outside `utils/mod.rs`. Comment
lines are stripped first, so prose about the pattern cannot satisfy a rule about
code — the same technique as `the_enum_parse_has_exactly_one_home`.

**The guard immediately caught the fourth copy**, in `/discover`'s date chip,
which I had written and would not have thought to look at. That is the test
earning its place on the first run rather than in theory.

## The pattern, stated plainly

This is the fifth time in two days: `.issues/086`, `.issues/095` (one predicate,
three places), `.issues/101` (one key shape, wrong for one kind), `.issues/102`
(one source, two grains), and now one date format in four surfaces.

Fixing the instance is not the fix. Every one of these was a rule that had no
single home, and in three of them **I fixed one copy and shipped the others**.
The repair that holds is a home plus a test that fails when a second copy
appears.

## Related

- `.issues/094` — where the first copy was fixed.
- `.issues/092` — the fmt gate that had the same shape of gap.
