# 133 — The ticket page dies when the event has no venue map link

**Status:** open, reproduced locally, NOT fixed
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
