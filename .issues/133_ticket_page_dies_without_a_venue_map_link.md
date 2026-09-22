# 133 — The ticket page dies when the event has no venue map link

> **DEPLOYED TO PRODUCTION 2026-09-23**, version
> `05d7df99-f1cd-45af-a4c5-e6cfdbf054d0`. Verified in a real browser against
> prod: two previously-dead tickets (`intro-to-vibing-on-solana`,
> `solana-in-latent-space-part-1`) now render with no parse error, and the
> field that used to be `null` returns `''`. The 476 dead tickets are alive.

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

## 5. Blast radius — MEASURED 2026-09-22 (see §10)

~~NOT measured.~~ The `7403` that blocked this cleared; the remote D1 answers
again from this machine, and the failure was measured **against the deployed
production worker**, not inferred from the table. Headline:

> **13 of the 14 production events that have attendees serve `null`.
> 476 of 514 attendee tickets are dead today.** The one that works is
> RTM#6 — the event being run this week.

The original "run this query" plan is kept below because it is the wrong query,
and knowing why is the point — see §10.2.

```sql
-- NOT sufficient: the read path is KV-first, and this asks the table.
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

## 10. Blast radius, measured against production — 2026-09-22

Measured by session `event-checkin-aa`. Every number below is a response from
`https://bethere.solana-thailand.workers.dev`, the live worker, not a local
build and not a table read.

### 10.1 The result

One ticket per event, requested exactly the way the ticket page requests it:

| | events | attendees |
|---|---:|---:|
| payload serves `null` → **page dies** | **13** | **476** |
| payload serves a string → page renders | 1 | 38 |
| not probed (event has 0 attendees) | 2 | 0 |
| **total** | 16 | 514 |

The single working event is `solana-x-ai-builders-the-road-to-mainnet-6-bangkok`
— **RTM#6, the event being run on 2026-09-27**. It is the only production event
whose `location_map_url` is set. Every past event's ticket — RTM#1 through #5,
all seven *Latent Space* parts, *intro-to-vibing-on-solana*, *islanddao-v4-demo*
— is dead for its attendees right now, and has been since `7e1d3c1` reached
production (deployed 2026-09-19 16:47 UTC; latest prod version
`f5b99ef0-2843-4352-8c8b-9800020f5c4a`).

So §4's guess ("every event anyone has opened a ticket for presumably has a map
link") is **backwards**. Exactly one event has one, and it is the one currently
in use — which is precisely why the organiser has not seen this.

**This also means RTM#6 works by accident.** Nothing in the product requires a
map link. The next event created without one takes its whole attendee list with
it.

### 10.2 Two traps in measuring it — both hit here

**Trap 1 — the D1 query in §5 gives the wrong answer.** It reports 15 of 16
events with an empty `location_map_url`. The served payload disagrees, because
the read path is KV-first ([[kv-masks-direct-d1-event-writes]]): the table is
not what the worker answers from. The number happens to be close, but it is
close by luck and was derived from the wrong source. **Ask the API.**

**Trap 2 — probing without `event_id` reads as a clean bill of health.**

```
GET /api/public/ticket/{attendee_id}              → map url present, looks FINE
GET /api/public/ticket/{attendee_id}?event_id={id} → null, page dies
```

`get_public_ticket` starts with `resolve_event(&state, query.event_id)`. With
no `event_id` it resolves to the **active** event — RTM#6 — so every attendee
of every dead event probes green, and the *attendee id in the URL is ignored*
for the purpose of the event fields. A first pass over all 14 events returned
the same RTM#6 map URL fourteen times, which is the tell: identical values
across unrelated events mean the parameter is not reaching the query.

That form is not hypothetical-only — it is what a person debugging by hand
would type. Every real link the product hands out carries `event_id`
(`register/my_registration.rs`, `notifications/content.rs`,
`notifications/outbox.rs`, the claim, deposit, admin and feedback pages — all
checked), so the dead path is the only one attendees ever travel.

Same family as [[false-clean-probes-from-shell-aliases]]: the probe answered,
answered consistently, and was measuring something else.

### 10.3 Method, so it can be re-run

```bash
# 1. events + one attendee id each (ids, not slugs — they differ, e.g.
#    slug solana-in-latent-space-part-2 has id solana-in-latent-space-part-1-copy)
npx wrangler d1 execute bethere-db --remote --json --command \
  "SELECT e.id AS event_id, e.slug, MIN(a.id) AS attendee_id, COUNT(a.id) AS attendees
     FROM events e LEFT JOIN attendees a ON a.event_id = e.id GROUP BY e.id;"

# 2. ask the live worker the way the page asks it
curl -s "https://bethere.solana-thailand.workers.dev/api/public/ticket/$AID?event_id=$EID" \
  | python3 -c "import json,sys; print(repr(json.load(sys.stdin)['data']['event_location_map_url']))"
# null  → that event's tickets are dead
# "..." → they render
```

The payload is wrapped: the field is under `data`, not at the top level. Reading
the top level reports the key as absent, which is a third way to get a false
pass.

### 10.4 What this changes

1. **This is a live production outage, not a latent defect.** 476 of 514
   attendee tickets. Previously carried as "blast radius unmeasured".
2. **The fix in §9 is still the right one** — nothing here changes the
   mechanism, it only sizes it.
3. **Shipping it is a production deploy**, and prod is deliberately
   undeployed (`.plans/027` §F.6: two pending migrations, a preflight gate that
   has never been satisfiable, five days before RTM#6). The fix cannot go out
   on its own without either cherry-picking it onto what is deployed or
   accepting the rest of the undeployed queue with it. **That is the owner's
   call and is not taken here.**
4. **RTM#6 itself is not at risk from this bug** — its map link is set. The
   damage is to past attendees revisiting their tickets, which is also the
   mildest possible version of 476 broken pages.
