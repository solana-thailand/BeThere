# 133 — The ticket page dies when the event has no venue map link

**Status:** fixed and guarded — **but UNCOMMITTED in the working tree** (see §9)
**Found:** 2026-09-22, incidentally, while browser-verifying `.issues/132`
**Introduced by:** `7e1d3c1 feat(events): add Google Maps link for event location`
**Unrelated to 0049** — the announcement feature merely gave a reason to open a
ticket page for an event that had no map link.

## 1. Symptom

The entire ticket page fails to render. The attendee sees:

```
Something Went Wrong
Failed to load ticket: API error (0): Failed to parse ticket data:
invalid type: null, expected a string at line 1 column 765
Go Home
```

Not a missing map link — the whole ticket. No QR, no name, no check-in state.

## 2. Mechanism

Server, `worker/src/handlers/attendee/read.rs:358`:

```rust
"event_location_map_url": safe_map_url(&event.location_map_url),
```

`safe_map_url` returns `Option<String>`, and for a stored empty string it
returns `None`:

```rust
pub fn safe_map_url(stored: &str) -> Option<String> {
    normalize_map_url(stored).ok().filter(|url| !url.is_empty())
}
```

`None` serialises to a literal JSON `null`.

Client, `frontend-leptos/src/api/types.rs:388`:

```rust
#[serde(default)]
pub event_location_map_url: String,
```

**`#[serde(default)]` only applies when the key is ABSENT.** An explicitly
present `null` is still deserialised into `String` and fails. The attribute
looks like it covers this case and does not.

That is the whole bug: one side says "absent or null are the same thing", the
other side only handles absent.

## 3. Reproduction (done, not theorised)

Same build, same seeded event, one column changed between runs:

| `events.location_map_url` | ticket page |
|---|---|
| `''` | **dead** — "Something Went Wrong", `.ticket-announcement-card` absent, no ticket |
| `'https://maps.app.goo.gl/probe123'` | renders normally |

Local harness: `wrangler dev --local --port 8788 --persist-to /tmp/bethere-dev-state`,
event `probe-evt`, attendee `probe-inperson`.

## 4. Why this may have gone unnoticed

Every event anyone has opened a ticket for presumably has a map link set. The
case that breaks is an organizer who leaves the field blank — and an
**online-only event is exactly where nobody would ever set a venue map link**,
so the audience most likely to hit this is the one least likely to be watched.

## 5. Blast radius — NOT measured

`npx wrangler d1 execute bethere-db --remote` fails from this machine with
`code: 7403, "The given account is not valid or is not authorized to access
this service"`, so the count of production events with an empty
`location_map_url` is **unknown**. Do not assume it is zero.

The query to run once someone has authorised access:

```sql
SELECT COUNT(*) AS total,
       SUM(CASE WHEN location_map_url IS NULL OR location_map_url = '' THEN 1 ELSE 0 END) AS no_map
FROM events;
```

## 6. Candidate fixes

Not applied — this is outside the scope of the change that found it, and the
choice affects the wire contract.

1. **Client-side, narrowest:** make the field tolerate null.
   `#[serde(default, deserialize_with = "...")]` mapping `null` to `""`, or
   change the type to `Option<String>` and update the two or three call sites.
   Fixes today's clients; an old deployed frontend stays broken.
2. **Server-side, fixes every client at once:** emit `""` instead of `null` —
   `safe_map_url(&event.location_map_url).unwrap_or_default()`. The field is
   already documented client-side as "Empty = no link", so `""` is the value
   the contract was written for. **This is the one I would pick.**
3. Both, if the deployed-frontend-vs-new-worker ordering matters.

Whichever is chosen, the regression test belongs at the boundary: assert the
ticket payload for an event with no map link deserialises into the client type.
Neither `cargo check`, `clippy -D warnings` nor an HTTP 200 catches this —
see [[verify-frontend-by-opening-it]]; the page returns 200 while showing an
error card.

## 7. A wider question worth one pass

`safe_map_url` is not the only helper returning `Option<String>` into this
payload. The same shape — server `None` becomes `null`, client declares a bare
`String` with `#[serde(default)]` — would fail identically. The ticket payload
had 9 null fields in the repro:

`claimed_asset_id`, `cluster`, `deposit_deadline_hours`, `deposit_info`,
`event_location_map_url`, `in_person_available`, `qr_image`, `refund_link`,
`rollover_target_event`

Only `event_location_map_url` broke. I checked the other eight against their
client declarations and **all eight are `Option`** — `claimed_asset_id`,
`cluster` and `qr_image` are `Option<String>`, `deposit_deadline_hours` is
`Option<u32>`, `in_person_available` is `Option<bool>`, `deposit_info` and
`rollover_target_event` are `Option<...>`, `refund_link` is `Option<String>`.

So `event_location_map_url` is the single odd one out today, and the audit
above is already done for this payload. The thing worth keeping is the *rule*:
a server helper returning `Option<T>` into a payload whose client field is a
bare `T` is a page-killer, and nothing in the build catches it.

## 8. The sibling path gets it right — that is the real defect

Credit to session `event-checkin-ff`, which spotted this; verified here against
source rather than taken on trust.

`safe_map_url` has **two** consumers, and they disagree about the contract:

| consumer | server emits | client declares | result |
|---|---|---|---|
| public event page | `worker/src/handlers/public_event.rs:216` — nullable | `frontend-leptos/src/pages/public_event/types.rs:226` — `Option<String>` | **survives** |
| ticket page | `worker/src/handlers/attendee/read.rs:358` — nullable | `frontend-leptos/src/api/types.rs:388` — bare `String` | **dies** |

The public-event side even re-runs the helper defensively
(`details_card.rs:14`: `data.location_map_url.as_deref().and_then(safe_map_url)`).
The ticket side just declares `String` and hopes.

This is why it went unnoticed: the path anyone would have tested by hand —
open the public event page for an event with no map link — works fine. Only
the ticket page, which needs a seeded attendee to reach, is broken.

So the fix should make the contract **uniform** rather than patch one field.
Same helper must mean the same thing to every consumer; today it means
"optional" on one page and "always present" on another, and nothing in the
build tells you they disagree. This is the same shape as
[[duplicated-state-transition-paths]]: a guard lands on one entry point and the
sibling keeps the bug.

**Ownership:** `event-checkin-ff` is taking the fix
(`frontend-leptos/src/api/types.rs`, possibly `worker/src/handlers/attendee/read.rs`,
plus a test under `frontend-leptos/tests/`). This issue file and the repro stay
with the session that found it.

## 9. The fix, and the guard that would have caught it

Written by session `event-checkin-ff`. Recorded here because this is the issue
file; the claims below were **verified against the tree**, not transcribed.

### The fix — server-side, deliberately

`worker/src/handlers/attendee/read.rs:358`:

```rust
"event_location_map_url": safe_map_url(&event.location_map_url).unwrap_or_default(),
```

Server-side and not client-side on purpose: it repairs **every** client at
once, including already-loaded tabs running a cached wasm bundle, which a
client-side fix cannot do. It also puts the wire back in step with what the
client already believed — `api/types.rs:388` documents "Empty = no link", and
`frontend-leptos/src/pages/ticket/event_context.rs:66` re-runs `safe_map_url`
on a plain `String` (verified). So `""` is the value the client was written
for all along.

### The guard — `frontend-leptos/tests/wire_nullability.rs`, 4 tests

Derives every `pub fn … -> Option<…>` under `domain/src` from source, so a
helper written tomorrow is covered the day it exists; brace-matches the `json!`
blocks in `read.rs` and `public_event.rs`; asserts any entry calling such a
helper is either flattened or `Option` client-side. Floors on both sides so it
fails loudly rather than silently matching nothing.

`event-checkin-ff` reports proving it fires by reverting `read.rs` to the
broken version and watching the guard **fail against the real files** (exit
101), then pass with the fix restored. I did not re-run that A/B myself — it
would mean editing another session's in-flight file — so it is recorded here as
their result, not mine.

### Why `mirror_field_types.rs` did not catch this

That test **already flags exactly this hazard** (domain `Option<T>` vs frontend
`T`); its module docs say so. It missed this one for two independent reasons,
both verified here:

1. **`AttendeeData` is not in `MIRROR_STRUCT_PAIRS` at all.** There are 24
   pairs — `PublicEventData`, `AttendeeListItem`, `AttendeeResponse`,
   `StatsResponse`, `EventDetail`, `PrPack` and so on — and the ticket
   payload's struct is not among them. Confirmed: `AttendeeData` appears zero
   times in that file.
2. **Even if it were, it could not see this.** `EventConfig.location_map_url`
   is a plain `String`. The `null` is manufactured by a **handler expression**
   in the `json!` block, and `mirror_field_types.rs` parses *struct
   definitions*.

Point 2 is the durable lesson, and it generalises well beyond this field:
**struct-level mirror checks cannot see nullability created in the payload.**
Any guard built by comparing type declarations has a blind spot exactly the
shape of a `json!` macro.

### Honest coverage — do not over-trust the new guard

The guard covers **1 of the 9** top-level nullables in the ticket payload: the
one that arrives via a domain helper. The other eight — `qr_image`,
`deposit_info`, `claimed_asset_id`, `cluster`, `rollover_target_event`,
`in_person_available`, `refund_link`, `deposit_deadline_hours` — reach `json!`
as locals or struct fields and are checked by **nothing**.

All eight are correctly `Option` client-side today (three independent passes
agree, and `-78` extended it to 21 nullables across three nesting levels), but
nothing keeps them that way.

`event-checkin-ff` initially described those eight as "covered by
`mirror_field_types.rs` instead"; `-78` caught that it was false, and the docs
were corrected to say unchecked. Worth preserving as the reason this section
exists: **a guard that overstates its coverage ends the search.** The comforting
version of this note would have closed the issue with eight fields unguarded.

### Verification recorded

`worker cargo clippy --all-targets -D warnings` exit 0 · frontend `cargo fmt`
clean · frontend suite **216 passed / 0 failed** including the 4 new tests —
this last one I re-ran myself and confirm.

### NOT COMMITTED

`ff`'s instructions are to commit only when its user asks, so the fix and the
guard sit **uncommitted in the working tree**:

```
 M worker/src/handlers/attendee/read.rs
?? frontend-leptos/tests/wire_nullability.rs
```

Nobody has claimed the commit. This is the live loose end on this issue — a
verified fix for a page-killing bug, sitting unstaged, one `git checkout` away
from being lost.
