# 088 — The claim flow asserted a `status` field the API has never returned

**Status:** fixed 2026-09-13
**Found:** 2026-09-13, working through `.issues/084`
**Severity:** low in impact, high in cost — it made a healthy endpoint look broken

## What

`flows/claim.rs` required a `status` field and checked it against a vocabulary
(`eligible` / `pending` / `ready` / `claimed` / …). `GET /api/claim/{token}`
returns none of that. Its actual payload:

```
api_id, checked_in_at, claim_token, claimed, claimed_asset_id, claimed_at,
claimed_signature, claimed_wallet, cluster, deposit_amount_thb,
deposit_amount_usdc, deposit_enabled, event, event_id, locked_wallet, name,
nft_available, participation_type, quiz_status, total_checked_in, total_claimed
```

Twenty-one fields, no `status`. So the flow failed against a perfectly healthy
API, and on staging the failure arrived as `HTTP 500 — internal error` (an
unknown token misses D1, falls through to Sheets, and staging's empty
`PLATFORM_SHEET_ID` turns that into a 500), which reads like a server fault and
is not one. That cost real time inside `.issues/084`.

## Which side was stale

The **Worker**, because the production Leptos claim page is built on exactly
these fields. Adding a `status` to satisfy a test would have created API surface
with no consumer and a second source of truth for "can this be claimed".

## Fix

`ClaimResponse` now mirrors the real payload (`claimed`, `checked_in_at`,
`nft_available`, `claimed_at`), and the assertions derive eligibility from it:

- **shape** — a `claimed=true` payload must carry a `checked_in_at` and a
  `claimed_at`. A badge cannot be claimed without a check-in, and an incoherent
  payload is the thing worth catching.
- **pre-claim** — `claimed == false` and `checked_in_at` non-empty.

`nft_available` is deliberately **not** asserted: it reflects whether the
environment has Crossmint configured, which is an environment fact, not a
claim-path regression. Asserting it would fail the flow on any staging worker
without a mint provider.

Every field stays `#[serde(default)]`, so the ~17 fields the harness does not
model cannot break it — a test pins that with a real captured payload.

## Result

The claim flow passes against deployed staging, taking the suite from 3/6 to
**4/6**. The remaining two are the on-chain horizon constraint documented in
`.issues/084`, unrelated to this.

## Related

- `.issues/084` — the preflight gate this unblocks one more flow of.
- `.issues/078` — why an unknown claim token 500s on staging rather than 404s.
