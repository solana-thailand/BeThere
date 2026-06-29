# Campaigns Guide — Multi-Event Series with Rewards

> How to configure, run, and troubleshoot campaigns on BeThere. Written from a
> ground-truth read of the code (not just the original intent), including the
> gaps you'll hit in practice.

---

## 1. What a campaign is

A **campaign** is a **series of related events** that an attendee completes in
sequence to earn a reward — usually a campaign completion NFT that scores 3× on
the developer leaderboard (vs 1× per single-event attendance NFT).

Typical use cases:

- A multi-city meetup series ("Road to Mainnet — Bangkok / Singapore / Tokyo")
- A recurring hackathon track with checkpoints
- Any case where you want to reward *attendance across multiple events*, not
  just one

A campaign is **not** a single event. If you only have one event, you don't need
a campaign — the event's own attendance NFT already covers that.

---

## 2. The end-to-end loop (how it actually works)

All five steps below are wired up and verified in the current code.

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│ 1. Organizer    │────▶│ 2. Attendee      │────▶│ 3. Progress     │
│    defines      │     │    checks in to   │     │    auto-tracks  │
│    campaign     │     │    campaign event │     │    on check-in  │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│ 5. Leaderboard  │◀────│ 4. Attendee      │◀────│    Complete →   │
│    scores 3×    │     │    claims reward │     │    NFT minted   │
└─────────────────┘     └──────────────────┘     └─────────────────┘
```

1. **Organizer configures** the campaign: links events, sets status to
   `active`, configures the reward.
2. **Attendee checks in** to any event that belongs to the campaign — nothing
   special required of the attendee.
3. **Progress tracks automatically.** On every check-in, `on_event_checkin`
   (`worker/src/db/campaigns.rs:518`) counts how many *required* campaign events
   the attendee has now checked into, and upserts their
   `developer_campaign_progress` row.
4. **On completion** (checked-in count ≥ total required), the attendee becomes
   eligible to claim. `POST /api/campaigns/{id}/claim-reward` validates
   completion + idempotency, then mints a compressed NFT via Helius
   (`mint_compressed_nft`) and persists `asset_id` + `signature`.
5. **Leaderboard classification.** `campaign_collection_mints()`
   (`worker/src/db/campaigns.rs:136`) reads active campaigns' `collection_mint`
   values; the wallet handler matches each NFT against that set. Matches score
   **3 points**, non-matches score **1 point**.

> **Key invariant:** there is no separate "enrollment" step. Progress is
> derived directly from check-ins, so an attendee doesn't need to register for
> the campaign itself — they just check into the events.

---

## 3. Quick start: setting up a campaign

### Option A — From an existing event (fastest)

If the campaign is built around one anchor event and you'll add more later:

1. **Admin → Events** (press `1`).
2. Click the event to open its detail card.
3. Click **"Promote to Campaign"**.
4. You'll land on the Campaigns → Create form, pre-filled:
   - Campaign ID: `{event.id}-campaign` (editable)
   - Title: `{event.name} Campaign`
   - Reward type: `none`
5. **Adjust** the fields as needed (see §4 for what each means).
6. **Save.** The source event is auto-linked as the first campaign event
   (`sequence_order: 0`, `is_required: true`), and you land on the new
   campaign's Detail → Events tab.
7. **Set status → `active`** from the campaign card (otherwise nothing counts).
8. If you want the NFT reward: set `reward_type: nft_certificate` and fill
   `reward_config.collection_mint` (see §4).

### Option B — From scratch

1. **Admin → Campaigns** (press `2`) → **"+ Create Campaign"**.
2. Fill the form (fields explained in §4). Minimum required:
   - **Campaign ID** (slug, unique, immutable after creation)
   - **Title**
   - **Reward type**: `none` for analytics-only, or `nft_certificate` for
     minting
3. Save → campaign is created in `draft` status.
4. Open the campaign → **Events** tab → **Add event** (you'll need the event's
   `id` — the immutable identifier, not the slug; see §6.1).
5. Set `sequence_order` (display order) and `is_required` per event.
6. Flip status → `active`.

---

## 4. Field reference (what each setting actually does)

### Top-level fields

| Field | Type | What it does | Gotcha |
|-------|------|--------------|--------|
| `id` | string (slug) | Unique identifier; used in URLs and as the D1 primary key | **Immutable** after creation. Cannot be renamed. |
| `title` | string | Human-facing name | — |
| `description` | string | Long-form description shown in admin | Not shown to attendees anywhere I could find |
| `organization_id` | string | Optional org grouping | Free text; no validation against an orgs table |
| `status` | enum | `draft` / `active` / `completed` | **Only `active` campaigns** participate in progress tracking AND leaderboard NFT classification. Default is `draft` — easy to forget. |
| `completion_criteria` | string | **Stored but NOT enforced.** | ⚠️ See §6.2 — the real rule is "attend all required events." This field is descriptive only. |
| `reward_type` | enum | `none` / `nft_certificate` / `badge` | `badge` is accepted by validation but no badge-minting code path exists — treat it as `none`. |
| `reward_config` | JSON string | NFT metadata (see below) | Only used when `reward_type = nft_certificate`. |

### `reward_config` sub-fields (NFT metadata)

These power the minted completion NFT. All optional at the form level, but
**`collection_mint` is load-bearing** for leaderboard scoring.

| Sub-field | Used for | Required? |
|-----------|----------|-----------|
| `name` | NFT name (default: `"{title} - Campaign Complete"`) | No |
| `symbol` | NFT symbol (default: `"CAMPAIGN"`) | No |
| `description` | NFT description (default: `"Completed the {title} campaign"`) | No |
| `image_url` | NFT image | No |
| `metadata_uri` | Off-chain metadata URI | No |
| `collection_mint` | **The Solana collection mint** the cNFT is minted into | **Yes** if you want 3× leaderboard scoring. Must match the actual collection the NFT is minted under. |

### Events tab fields (`campaign_events`)

| Field | What it does |
|-------|--------------|
| `event_id` | The event's immutable `id` (not the slug). Foreign key into the `events` table. |
| `sequence_order` | Display ordering for series navigation (prev/next on the ticket page). Integer; lower = earlier. |
| `is_required` | If `true`, the event counts toward completion. If `false`, it's part of the series but optional. |

---

## 5. Attendee experience

### What the attendee sees

- **Series navigation** — on a ticket page for an event in a campaign, a
  prev/next nav appears (`ticket/series_nav.rs`, Plan 013) showing the
  neighboring events in the series.
- **Developer dashboard** (`/dev-dashboard`) — a "Campaign Progress" section
  shows each campaign they have progress in, with a progress bar
  (`events_completed / total_required`) and one of three states:
  - *"No campaign progress yet"* (empty state)
  - *"Reward available to claim!"* (when `is_complete && !reward_claimed_at`)
  - *"Reward claimed"* (when `reward_claimed_at` is set)

### ⚠️ The claim gap (important)

The dashboard shows *"Reward available to claim!"* but **there is no button to
actually claim it.** The `claim_campaign_reward()` API client function exists
(`frontend-leptos/src/api/campaign.rs:382`) and the backend endpoint works
(mints via Helius, persists `asset_id` + `signature`), but **no UI element calls
it.** An attendee who completes a campaign currently cannot mint their reward
through the app — they'd have to call the API directly.

This is the single biggest functional gap in the feature. See §7.1.

---

## 6. Troubleshooting

### 6.1 "I added an event but progress isn't tracking"

**Cause 1 — campaign is not `active`.** Progress tracking (`on_event_checkin`)
runs regardless of status, but the leaderboard classification
(`campaign_collection_mints`) only reads `active` campaigns. Set status →
`active`.

**Cause 2 — wrong event identifier.** The Events tab requires the event's
**`id`**, not its **`slug`**. These are identical at creation and stay identical
through a rename, but diverge if an organizer has explicitly edited the slug
field afterward. `id` is the immutable foreign key; `slug` is the public route.
If you typed a slug and it doesn't match the id, the join in `on_event_checkin`
silently finds no rows.

**Cause 3 — the event isn't marked `is_required`.** Only required events count
toward `total_required`. A non-required event is part of the series (shows in
nav) but doesn't advance completion.

**Cause 4 — no check-ins yet.** Progress is computed *at check-in time*. If
attendees registered but haven't checked in, the Progress tab is empty. This is
expected.

### 6.2 "I set `completion_criteria` to something custom and it's ignored"

It is ignored. The field is stored and displayed but **no code reads it.** The
actual completion rule is hardcoded in `on_event_checkin`
(`worker/src/db/campaigns.rs:518`):

> Complete when `events_completed ≥ total_required`

where `total_required` is the count of `campaign_events` rows with
`is_required = 1`. If you need a different rule (e.g., "attend 2 of 3"), that's
a code change in `on_event_checkin`, not a configuration.

### 6.3 "The minted NFT scores 1× not 3×"

`reward_config.collection_mint` must:

1. Be set (non-empty), **and**
2. Match the collection the cNFT is *actually* minted into, **and**
3. Belong to a campaign in `active` status.

`campaign_collection_mints()` builds the match set from active campaigns'
`collection_mint` values; if yours is blank, wrong, or the campaign is
`draft`/`completed`, the NFT won't match and defaults to 1×.

### 6.4 "Claim-reward returns an error"

Common causes:

- **Not actually complete** — `is_complete` is false. Check the Progress tab.
- **Already claimed** — `reward_claimed_at` is set; the endpoint returns 422
  *"reward already claimed for this campaign"*.
- **Helius failure** — the mint call returns `502` with service `"helius"`.
  Check `config.solana.api_key` and RPC connectivity.

---

## 7. UX observations & improvement opportunities

These are gaps I found while reading the code. They range from "minor polish"
to "the feature is half-shipped without this." Ordered by impact.

### 7.1 🔴 No claim button (attendee can't mint their reward) — *high impact*

**Problem:** The dev dashboard shows *"Reward available to claim!"* as static
text. `claim_campaign_reward()` is implemented end-to-end (API client + backend
mint) but never wired to a clickable element.

**Fix:** Add a "Claim Reward" button in `dev_dashboard.rs` around line 280
(the `else if item.is_complete` branch) that calls `claim_campaign_reward`
with the connected wallet address, then refreshes progress. The wallet-adapter
bridge already exists in that file (line 22 references it). ~30 lines.

**Without this, the entire reward loop is unreachable from the UI.** This is
the highest-priority fix.

### 7.2 🟡 "Add event" requires typing a raw event ID — *medium impact*

**Problem:** In the campaign Events tab, adding an event means pasting an
`event_id` string. Organizers don't memorize IDs, and id-vs-slug confusion
(§6.1) silently breaks progress tracking.

**Fix:** Replace the text input with a `<select>` populated from
`api::list_events()` — show `event.name` as the label, use `event.id` as the
value. The events list is already loaded in the admin shell.

### 7.3 🟡 `completion_criteria` field is misleading — *medium impact*

**Problem:** It's a free-text field that implies configurable rules
("attend_all", "attend_2_of_3", etc.) but no value is ever read. Organizers
will reasonably believe their custom text does something.

**Fix (pick one):**
- **Document it clearly** in the form (placeholder: *"Descriptive only — actual
  rule is attend-all-required"*) — zero code change.
- **Make it a dropdown** of implemented criteria (currently just
  `attend_all_required`) — small frontend change.
- **Remove it** and rename the implicit rule in the UI — cleanest but breaks
  existing data.

### 7.4 🟡 Default `draft` status is a footgun — *low-medium impact*

**Problem:** New campaigns default to `draft`. An organizer who creates a
campaign, links events, and walks away will find nothing tracks because the
campaign isn't `active`. There's no warning.

**Fix:** After linking the first event, show a banner/toast: *"This campaign is
in draft status — switch to Active to start tracking progress."* Or auto-flip
to `active` when the first event is added (with an undo).

### 7.5 🟢 `reward_config.collection_mint` has no validation — *low impact*

**Problem:** The load-bearing field for 3× scoring accepts any string. A typo
silently demotes the NFT to 1× with no error.

**Fix:** On save, if `reward_type = nft_certificate` and `collection_mint` is
set, optionally validate it resolves on-chain (lightweight RPC check), or at
least format-validate (base58, 32-44 chars). Non-blocking warning if invalid.

### 7.6 🟢 `reward_type: badge` is accepted but unsupported — *low impact*

**Problem:** `validate_reward_type` accepts `"badge"`, but no badge-minting code
path exists. Selecting it produces a campaign that behaves like `none`.

**Fix:** Either implement badge minting, or remove `"badge"` from the validator
until it exists. Don't ship dead options.

### 7.7 🟢 No public campaign discovery page — *low impact*

**Problem:** Attendees can see *their own* progress on `/dev-dashboard`, but
there's no public page listing available campaigns for discovery ("here are the
active series you can join"). The series nav on a ticket page shows prev/next
within one campaign, but only after you've landed on a member event.

**Fix:** A `/campaigns` public route listing `active` campaigns with their
event sequences and a "view series" link. The data is all available via
`GET /api/campaigns` (admin) or a public variant. Lower priority but improves
discoverability.

### 7.8 🟢 Progress view shows counts, not which events — *low impact*

**Problem:** The admin Progress tab shows `events_completed / total_required`
per developer, but not *which* specific events they've checked into. For
support ("why is this person stuck at 2/3?") you have to cross-reference
attendee records manually.

**Fix:** Expand the progress row to list the checked-in event names, or add a
drill-down. Cosmetic but useful for organizers.

---

## 8. Technical reference

### Database tables (`worker/migrations/0007_campaigns_tables.sql`)

| Table | Purpose |
|-------|---------|
| `campaigns` | Campaign records (id, title, status, reward_type, reward_config JSON, timestamps) |
| `campaign_events` | Junction: event belongs to campaign, with `sequence_order` + `is_required` |
| `developer_campaign_progress` | Per-developer progress: `events_completed`, `total_required`, `is_complete`, `completed_at`, `reward_claimed_at` |

### Key code paths

| Path | Role |
|------|------|
| `worker/src/handlers/campaigns.rs` | 12 API endpoints (CRUD + events + progress + stats + claim-reward) |
| `worker/src/db/campaigns.rs` | 19 query functions + `on_event_checkin` (auto-progress) + `campaign_collection_mints` (leaderboard match set) |
| `worker/src/handlers/checkin.rs:159` | Calls `on_event_checkin` via `wait_until` (non-blocking) |
| `worker/src/handlers/wallet.rs` | `classify_nfts()` uses the match set for 3×/1× scoring |
| `frontend-leptos/src/api/campaign.rs` | Typed API client (all endpoints) |
| `frontend-leptos/src/pages/campaigns_page.rs` | Admin UI (list, create/edit, detail with Events/Progress/Stats tabs) |
| `frontend-leptos/src/pages/dev_dashboard.rs` | Attendee progress display (⚠️ no claim button — see §7.1) |
| `frontend-leptos/src/pages/ticket/series_nav.rs` | Public prev/next navigation within a campaign |

### API endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/campaigns` | List (filter by `organization_id`, `status`) |
| `POST` | `/api/campaigns` | Create |
| `GET` | `/api/campaigns/{id}` | Detail + linked events |
| `PUT` | `/api/campaigns/{id}` | Update fields |
| `DELETE` | `/api/campaigns/{id}` | Delete (cascades events + progress) |
| `PATCH` | `/api/campaigns/{id}/status` | Change status |
| `PUT` | `/api/campaigns/{id}/events` | Set events (**full replace** — sends the complete list) |
| `GET` | `/api/campaigns/{id}/progress` | List developer progress rows |
| `GET` | `/api/campaigns/{id}/stats` | Completion stats + per-event drop-off |
| `GET` | `/api/campaigns/my-progress` | Current user's progress across all campaigns |
| `POST` | `/api/campaigns/{id}/claim-reward` | Mint completion cNFT (⚠️ no UI button — see §7.1) |

---

## 9. Minimum viable campaign (checklist)

If you just want a campaign that works end-to-end, verify all of these:

- [ ] Campaign status is **`active`** (not `draft`)
- [ ] At least one event is linked in the Events tab
- [ ] At least one linked event has **`is_required = true`**
- [ ] You used the event's **`id`** (not `slug`) when linking — they must match
- [ ] (If you want the reward) `reward_type = nft_certificate`
- [ ] (If you want the reward) `reward_config.collection_mint` is set and points
      to the real collection
- [ ] Attendees have **checked in** to the required event(s) (not just
      registered)
- [ ] (To actually mint rewards — currently broken from UI) see §7.1; attendees
      must call the API directly until a claim button is added