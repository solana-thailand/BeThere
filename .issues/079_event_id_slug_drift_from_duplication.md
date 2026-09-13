# 079 — Duplicated events keep a `-copy` id forever, so ids name the wrong event

**Status:** open
**Found:** 2026-09-13, answering the DevRel `BETHERE-ASKS.md` item 3
**Severity:** low as a defect, medium as a trap — no row points anywhere wrong;
every human reading of the id is wrong

## What

`events.id` is the join key: `attendees.event_id`,
`registration_responses.event_id`, `event_summaries.event_id`,
`notification_outbox.event_id`, every audit row, and the R2 poster object key.
On **10 of the 16 production events the id disagrees with the slug**, and on 9
of those it names the *previous* event in the series:

| slug (correct) | id (names the previous event) |
|---|---|
| `solana-in-latent-space-part-2` | `solana-in-latent-space-part-1-copy` |
| `solana-in-latent-space-part-3` | `solana-in-latent-space-part-2-copy` |
| `solana-in-latent-space-part-4` | `solana-in-latent-space-part-3-copy` |
| `solana-in-latent-space-part-5` | `solana-in-latent-space-part-4-copy` |
| `solana-in-latent-space-part-6` | `solana-in-latent-space-part-5-copy` |
| `solana-in-latent-space-part-7` | `solana-in-latent-space-part-6-copy` |
| `...road-to-mainnet-4-bangkok` | `...road-to-mainnet-3-bangkok-copy` |
| `...road-to-mainnet-5-bangkok` | `...road-to-mainnet-4-bangkok-copy` |
| `...road-to-mainnet-6-bangkok` | `...road-to-mainnet-5-bangkok-copy` |

The tenth (`...road-to-mainnet-2-bangkok` / `...road-to-mainnet-2`) differs only
by a dropped suffix and is harmless.

## Mechanism

The id is minted from the slug, once, at creation:
`let (id, slug) = deduplicate_slug(&slug, &existing_id_refs);`
(`event_store/write/create.rs:93`).

Duplication does not ask for the new slug. It hardcodes one:
```rust
// handlers/events/duplicate.rs:131
let new_slug = format!("{}-copy", source.slug);
```
So a duplicate is born as `part-2-copy`, the organizer then renames the **slug**
to `part-3` in the UI — and the id, which is immutable, keeps saying `part-2`.
Nine events, two series, entirely predictable.

## Why it matters

Nothing is corrupt and the slug is right everywhere a user sees it. The damage
is that the id *looks* readable, so it gets read.

It reached outside the repo. DevRel found it via `poster_url`, which is exactly
`/api/storage/posters/{id}` (`handlers/events/poster.rs:114`), concluded the
posters had their own broken naming scheme, and built an external
`assets/poster-history/manifest.json` to record the mapping. The posters are
fine. The id is the only thing wrong — in a column with far more reach than a
filename.

An operator running an ad-hoc D1 query, reading a `tracing` line, or scanning
audit history will attribute to RTM #3 what belongs to RTM #4.

## Why not just rename the ids

Renaming rewrites every foreign key in the database — attendees, registration
responses, THB deposits, credit ledger, claim tokens, audit history, R2 object
keys — on live data that includes money movement, for a cosmetic gain. All
downside.

## Fix worth doing

**Stop minting them.** Let the duplicate request carry the intended slug
instead of hardcoding `-copy`:

- `DuplicateEventRequest` already takes `new_name`. Derive `new_slug` from it
  (falling back to `{source.slug}-copy` only when `new_name` is empty), or add
  an explicit optional `new_slug`.
- Then the id is right from birth and the existing ten rows stay as history.

A guard test asserting that a duplicate created with an explicit name gets an id
matching that name would keep it fixed.

Secondary, found while reading the same function: `duplicate.rs:170` copies
`poster_url: source.poster_url.clone()`, so a fresh duplicate renders the
**source event's** poster until someone re-uploads. The nine rows in prod all
happen to have been re-uploaded, so this is latent rather than active.

## Verification

```sql
SELECT id, slug FROM events WHERE id != slug ORDER BY event_start_ms;
```
10 rows today. After the fix, newly duplicated events must not appear here.

## Related

- `.issues/055_duplicate_event.md` — the feature that mints these ids.
- DevRel `reports/phase-2/BETHERE-REPLY.md` item 3.
