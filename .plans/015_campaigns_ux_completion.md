# Plan 015 — Campaigns UX Completion

> Spun out of the 2026-06-27 campaigns audit. The campaign backend is complete
> and the admin UI works, but the **attendee reward loop is unreachable** — the
> claim API exists with no button calling it. This plan closes that gap and
> addresses the smaller UX issues surfaced in the audit.

---

## Context

- **Campaigns feature**: fully built end-to-end on the backend (Issue #049
  Phase 3, handover #095) — auto-progress on check-in, claim-reward mints a
  compressed NFT via Helius, leaderboard scores campaign NFTs 3× (handover #096).
- **Admin UI**: works — list, create/edit, Events/Progress/Stats tabs.
  "Promote to Campaign" button shipped (handover #122, deployed, merged to main).
- **Campaigns guide**: `docs/campaigns_guide.md` documents the full feature
  including 8 UX gaps (§7 of that doc).
- **The blocker**: `claim_campaign_reward()` API client
  (`frontend-leptos/src/api/campaign.rs:382`) is fully implemented and the
  backend endpoint mints via Helius, but **no UI element calls it.** The dev
  dashboard shows *"Reward available to claim!"* as static text. Attendees
  cannot mint their reward through the app — the entire reward loop is
  unreachable from the UI.

## Goal (this plan)

Close the gap that makes campaigns functionally complete for attendees:
**P0 — add the claim button** so a completed campaign actually delivers the NFT.

Plus two P1 polish items that remove real footguns (event picker dropdown,
`completion_criteria` clarity).

## Non-goals (deferred)

- Public campaign discovery page (guide §7.7) — separate plan, needs design.
- Badge reward type implementation (guide §7.6) — remove the option instead.
- `collection_mint` on-chain validation (guide §7.5) — low value, high complexity.
- The 185 pre-existing clippy errors from Rust 1.96 lints — separate cleanup.

---

## P0 — Claim button (HIGH impact, ~30-50 lines)

### Problem

`frontend-leptos/src/pages/dev_dashboard.rs` around line 280: the
`else if item.is_complete` branch renders static text
*"Reward available to claim!"* with no call-to-action. The API and the wallet
bridge both exist but are unused for campaigns.

### Pre-work to read first (do not skip)

1. **`frontend-leptos/src/pages/claim.rs`** — the single-event NFT claim page.
   It already solves "get connected wallet address and call a mint endpoint."
   Mirror its wallet-address acquisition pattern exactly.
2. **`frontend-leptos/src/api/campaign.rs:382`** — the `claim_campaign_reward`
   signature. It takes `(id: &str, wallet_address: &str)` and returns
   `ClaimCampaignRewardResponse { asset_id, signature }`.
3. **`frontend-leptos/src/pages/dev_dashboard.rs:22`** — confirms the wallet
   adapter JS bridge is already wired in this file (same bridge as claim.rs /
   deposit.rs).
4. **`docs/campaigns_guide.md` §2 + §7.1** — full context on the loop and the gap.

### Implementation

**File**: `frontend-leptos/src/pages/dev_dashboard.rs`, the campaign item render
block (~lines 255-288).

**What to build**:

1. A loading-state signal, e.g.:
   `let (claiming_id, set_claiming_id) = signal(None::<String>);`
2. In the `else if item.is_complete` branch, replace the static text with a
   "Claim Reward" button.
3. On click:
   - Disable the button + set `claiming_id` to this campaign's id (loading state).
   - Get the connected wallet address (mirror how `claim.rs` does it).
   - Call `api::claim_campaign_reward(&item.campaign_id, &wallet_address)`.
   - On `Ok`: refresh progress via `api::my_campaign_progress()` so the row
     flips to "Reward claimed"; show a success toast with the `asset_id`.
   - On `Err`: toast the message (Helius failures return 502 / service
     "helius"; already-claimed returns 422 — handle both readably).
4. Gate visibility: button shows only when
   `item.is_complete && item.reward_claimed_at.is_none()`.

### Acceptance criteria

- [ ] Completed campaign (test row: `is_complete = true`, `reward_claimed_at = null`)
      shows a "Claim Reward" button on `/dev-dashboard`.
- [ ] Clicking mints the cNFT — verify via returned `asset_id` + `signature`
      and the row flipping to "Reward claimed".
- [ ] Already-claimed campaigns show "Reward claimed" (no button).
- [ ] Incomplete campaigns show no button.
- [ ] Helius errors surface a readable toast, not a silent failure.
- [x] `cargo check --target wasm32-unknown-unknown` clean.
- [x] `~/.cargo/bin/trunk build --release` clean.

### Test data setup (needed to verify end-to-end)

1. An active campaign with `reward_type: nft_certificate` and a valid
   `reward_config.collection_mint` pointing at a real collection.
2. At least one required event linked to the campaign.
3. A developer who has **checked in** to all required events
   (so `is_complete = true`). Note: progress is computed at check-in time via
   `on_event_checkin` (`worker/src/db/campaigns.rs:518`), not at registration.
4. `reward_claimed_at` null for that developer.

Easiest created via the admin UI against a dev/local environment with a known
developer wallet.

---

## P1 — Event picker dropdown (medium, ~20 lines)

### Problem

`campaigns_page.rs` Events tab "Add event" requires typing a raw `event_id`
string. Organizers don't memorize IDs, and the id-vs-slug confusion silently
breaks progress tracking (guide §6.1) because `on_event_checkin`'s join finds
no rows when the wrong identifier is stored.

### Implementation

**File**: `frontend-leptos/src/pages/campaigns_page.rs`, the add-event form
section (look for `add_event_id` / `set_add_event_id`).

Replace the text `<input>` bound to `add_event_id` with a `<select>` populated
from `api::list_events()`. Show `event.name` as the label, use `event.id` as the
value. Filter out events already linked to this campaign.

The events list can be loaded either by:
- A new mount Effect in CampaignsPage calling `api::list_events()` into a local
  signal, OR
- Threading the existing `events_list` signal down from `admin.rs` as a prop.

### Acceptance

- [ ] Add-event shows a dropdown of event names (not a text input for IDs).
- [ ] Selecting an event and clicking Add links it using `event.id`.
- [ ] Events already in the campaign are excluded or disabled in the dropdown.

---

## P1 — `completion_criteria` clarity (small)

### Problem

`completion_criteria` is a free-text field stored but **never enforced.** The
actual rule is hardcoded in `on_event_checkin`: complete when
`events_completed >= total_required`. Organizers reasonably believe their
custom text does something.

### Fix (recommend option A)

- **A (zero code change, recommend)**: update the form label/placeholder in
  `campaigns_page.rs` to make clear it's descriptive only — e.g. placeholder
  *"Descriptive only — actual rule is 'attend all required events'"*.
- **B (small frontend)**: convert to a `<select>` with the one implemented
  option (`attend_all_required`).
- **C**: remove the field (breaks existing data — not recommended).

---

## P2 — Minor polish (batch later)

Lower-priority; safe to roll into a single cleanup pass:

- **§7.4 draft status warning**: after the first event is linked, show a
  toast/banner prompting activation. Small.
- **§7.6 remove `"badge"` from `validate_reward_type`**
  (`worker/src/handlers/campaigns.rs:188`) until badge minting exists. 1-line
  validator change + drop the `<option>` in the form.
- **§7.8 richer progress view**: show which events each developer checked into
  (not just counts). Frontend enrichment over existing data.

---

## Outstanding from prior work (not this plan's scope)

- **Browser-test "Promote to Campaign"** (handover #122 §6): deployed to prod
  but not click-tested. Needs a human with admin OAuth. 9-step checklist lives
  in the handover.
- **Plan 004 remaining**: 8 checkboxes (3 Docker-blocked, 4 browser-needed,
  1 e2e-tx). The `.docs/` precondition is unmet.
- **`develop` lacks Solana Mobile demo** (`c2a1309` lives only on `main`):
  pre-existing asymmetry, unrelated to campaigns.
- **185 pre-existing clippy errors** (Rust 1.96 lints on old code): worth a
  `cargo clippy --fix --allow-dirty` pass but out of scope here.

---

## References

| Artifact | Path |
|----------|------|
| Campaigns guide (full reference + UX gaps) | `docs/campaigns_guide.md` |
| Handover — Promote to Campaign feature | `.handovers/122_promote_event_to_campaign.md` |
| Handover — Campaigns Phase 3 backend | `.handovers/095_campaigns_series_phase3.md` |
| Handover — Campaign NFT classification | `.handovers/096_campaign_nft_classification.md` |
| Plan — Event series navigation (related) | `.plans/013_event_series_navigation.md` |
| Claim API client | `frontend-leptos/src/api/campaign.rs:382` |
| Dev dashboard (claim button target) | `frontend-leptos/src/pages/dev_dashboard.rs:280` |
| Single-event claim (pattern reference) | `frontend-leptos/src/pages/claim.rs` |
| Backend claim handler (mints via Helius) | `worker/src/handlers/campaigns.rs:551` |
| Auto-progress on check-in | `worker/src/db/campaigns.rs:518` |
| Leaderboard match set | `worker/src/db/campaigns.rs:136` |

---

## Suggested execution order for a fresh agent

1. Read `docs/campaigns_guide.md` §2 + §7 for the full picture (~10 min).
2. Implement **P0 (claim button)** — it's the only item that makes the feature
   actually deliver rewards. Follow the pre-work reads above before coding.
3. Verify against test data per the P0 acceptance criteria.
4. Run `cargo check --target wasm32-unknown-unknown` and
   `~/.cargo/bin/trunk build --release`.
5. Browser-test P0 on a dev/local environment (needs OAuth — see guide §5).
6. If time remains, do the two P1 items.
7. Commit each slice on its own `feature/...` branch per gitflow; write a
   handover (next number after #122) when done.