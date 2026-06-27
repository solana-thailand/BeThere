# Handover 122 — Promote Event → Campaign (one-click flow)

**Date:** 2026-06-27
**Branch:** `feature/campaign_from_event` (off `develop` @ `28c143a`, NOT yet merged)
**Commit:** `a81208d` — `feat(frontend): promote event to campaign one-click flow`
**Outcome:** ✅ Implemented + compiles clean (`cargo check --target wasm32-unknown-unknown` 0/0). NOT yet merged to `develop`. NOT yet browser-tested. NO backend changes.
**Test delta:** 0 automated tests added (frontend-leptos has no wasm test harness for admin UI; behavior is reactive signal flow verified by `cargo check` only).

---

## 1. What happened

The user asked (continuing from the Plan 004 Checkbox Audit Checkpoint thread): *"Can you take a look if I want to add campaign from existing event, how can I do it easiest?"*

I traced the codebase and reported three options:
1. **UI manual** (current default) — Create campaign form, then add event by id in the Events tab.
2. **Direct API** — `curl` POST `/api/campaigns` then PUT `/api/campaigns/{id}/events`.
3. **A "Promote to Campaign" button** — the actually-easiest long-term fix (~30 lines, no backend changes).

User said *"continue from your suggest"* → I implemented Option 3.

### What the organizer now sees

1. Admin → Events → click an event → detail card.
2. New **"Promote to Campaign"** button appears (next to Edit / Summary / Duplicate), gated by `can_manage_events` (organizer+).
3. Click → jumps to Campaigns with the create form **pre-filled**:
   - Campaign ID: `{event.id}-campaign`
   - Title: `{event.name} Campaign`
   - Reward type: `none` (safe default; passes backend `validate_reward_type`)
4. Save → campaign created, source event **auto-linked** as first campaign event (`sequence_order: 0`, `is_required: true`), view lands on the new campaign's **Detail → Events tab** so the linked event is visible immediately.

### Architecture decision: signals, not closure props

The cleanest design given the existing `Admin` shell architecture (which already passes `set_toast: WriteSignal<...>` and `active_event_id: ReadSignal<...>` to children gated by `<Show>`):

- `admin.rs` owns `(pending_promote_event, set_pending_promote_event) = signal(None::<PromoteEventPayload>)` plus a watcher `Effect` that flips `active_section` to `Campaigns` whenever a payload appears.
- `EventsPage` receives `set_pending_promote_event` (write only) and the button writes the payload.
- `CampaignsPage` receives both read + write. On mount it consumes the payload (pre-fills form + records source event id in a local `pending_event_to_link` signal), then clears the handoff signal. After a successful create it calls `set_campaign_events` (full-replace API) with the single event and navigates to Detail.

**Why this works (timing):** the `<Show>` gates unmount/remount, so switching section from Events → Campaigns always mounts `CampaignsPage` fresh. The payload is set in the same synchronous handler as the section switch (batched), so when `CampaignsPage` mounts its first `Effect` run sees `Some(payload)`.

### Defensive clears

`pending_event_to_link` (local to `CampaignsPage`) is cleared in three places so a leftover "promote" intent can never wrongly auto-link an event to an unrelated campaign:
- After successful create
- `handle_back` (cancel out of form)
- `handle_create_new` (manual "+ Create Campaign" button)

---

## 2. Where is the plan / code / test

| Artifact | Path | Lines | Purpose |
|----------|------|-------|---------|
| Type | `frontend-leptos/src/pages/campaigns_page.rs` | ~11-20 | `pub PromoteEventPayload { event_id, event_name }` |
| Consume on mount | `frontend-leptos/src/pages/campaigns_page.rs` | ~225-247 | `Effect::new` reads payload, prefills form, clears handoff |
| Auto-link on save | `frontend-leptos/src/pages/campaigns_page.rs` | ~355-405 | create branch: captures `link_event_id`, calls `set_campaign_events`, navigates to Detail |
| Button | `frontend-leptos/src/pages/events_page.rs` | ~336-362 | "Promote to Campaign" in event detail card, gated `can_manage` |
| Handoff signal + watcher | `frontend-leptos/src/pages/admin.rs` | ~245-255 | `(pending_promote_event, set_pending_promote_event)` + section-switch Effect |
| Prop wiring | `frontend-leptos/src/pages/admin.rs` | ~1820-1838 | props passed to `CampaignsPage` + `EventsPage` |

**No backend code changed.** All backend APIs used (`POST /api/campaigns`, `PUT /api/campaigns/{id}/events`) already existed from Issue #049 Phase 3 (handover #095).

**No tests added.** The frontend-leptos crate has no wasm-level test harness for admin UI flows; behavior is reactive signal plumbing verified by `cargo check` only. See §5 (Remaining work) for the browser test that's still owed.

---

## 3. Reflection — struggling / solved

### Solved: didn't reach for a closure prop

Initial instinct was to pass `on_promote_to_campaign: Arc<dyn Fn(Payload)>` to `EventsPage`. Reconsidered: that would have required `EventsPage` to either know about `AdminSection` (coupling) or call back into admin-owned section state via a closure (less typed than a signal). The signal-only design matches the existing `set_toast`/`active_event_id` pattern exactly and keeps `EventsPage` decoupled from section internals.

### Solved: caught the format-string field-access trap before CI

`format!("{p.event_id}-campaign")` does NOT compile — Rust's inline format capture supports simple identifiers (`{var}`) but not field access (`{p.field}`). The user's rule explicitly says `format!("{var}")` not `format!("{}", var)`, so I bound locals first (`let event_id = p.event_id.clone()`) then used `{event_id}`. The first `cargo check` caught two of these; fixed in one pass.

### Solved: didn't overclaim on clippy

The crate reports **185 clippy errors** under `-D warnings` on the current Rust 1.96 toolchain. I verified (by filtering clippy output for my added line ranges) that **zero** of those errors point at lines I added — they're all pre-existing patterns (`let set_toast = set_toast;` redefinitions, `match … { Ok => …, Err => {} }` single-match) now flagged by newer lints (`redundant_locals`, `single_match`, `manual_div_ceil`, etc.). Reported this honestly rather than "clippy clean."

### No real struggles

The implementation is mechanical signal plumbing over a well-understood existing pattern. ~2.5 hours end-to-end including the trace + design decision.

---

## 4. Remaining work

### Blocking merge (must do before `develop`)
- [ ] **Browser test the full flow** — open Events, click an event, click "Promote to Campaign", verify form pre-fills correctly, edit/save, verify campaign appears in Detail view with the source event linked in the Events tab. This is the one verification gap.

### Non-blocking, worth doing
- [ ] **Consider using `event.slug` instead of `event.id`** for the campaign id default if slug/id divergence becomes common. Currently `{event.id}-campaign` — id is immutable and primary, so this is the safer choice for the FK relationship, but the campaign id is human-visible in the admin UI.
- [ ] **Optional: pre-fill `organization_id`** if events grow an org field. `EventMeta` currently has no `organization_id`, so the field is left blank. Campaigns have always supported blank org.
- [ ] **Optional: pre-fill description** from event detail (would require fetching `EventDetail` via `get_event_detail` in the button handler, like the Edit button already does). Currently left blank to keep the click synchronous.

### Pre-existing, NOT introduced here
- 185 clippy errors across the frontend crate from Rust 1.96 lints. Worth a separate cleanup pass (`cargo clippy --fix --allow-dirty`) but out of scope for this branch.

---

## 5. Issues ref

This change is **not** tied to a numbered `.issues/` entry — it was an ad-hoc UX improvement prompted by the user's "easiest way to add campaign from existing event" question, continuing from the Plan 004 audit thread. Related prior work:

- **#049 Phase 3** (`campaigns_series_phase3`) — original Campaigns backend + admin UI (handover #095). This branch is a pure-frontend enhancement on top of that surface.
- **#051** (`campaign_nft_rewards`) — campaign NFT classification. Unaffected; this branch doesn't touch reward logic.

---

## 6. How to dev / test

### Local build check (already verified clean)
```
cd frontend-leptos
cargo check --target wasm32-unknown-unknown --quiet
```

### Clippy on this branch's files (verify no new lints)
```
cd frontend-leptos
cargo clippy --target wasm32-unknown-unknown --quiet -- -D warnings 2>&1 \
  | rg -e "campaigns_page\.rs:[0-9]" -e "events_page\.rs:[0-9]" -e "src/pages/admin\.rs:[0-9]"
```
Expected: only pre-existing lines (the `let set_toast = set_toast;` redefinitions and `single_match` patterns in pre-existing handlers). My added lines (campaigns_page ~225-247, ~355-405; events_page ~336-362; admin ~245-255, ~1820-1838) should NOT appear.

### Browser test (the gap — needs user)
1. `trunk serve` (or however the dev frontend runs locally against the worker)
2. Log in as an admin/organizer
3. Go to Admin → Events (Alt+1)
4. Click an active event to open the detail card
5. Click **"Promote to Campaign"**
6. Verify: jumps to Campaigns, "Create Campaign" form, with:
   - Campaign ID = `{event.id}-campaign` (editable)
   - Title = `{event.name} Campaign`
   - Reward type = `none`
7. Click Save
8. Verify: lands on the new campaign's Detail view, **Events tab**, with the source event listed (sequence 0, required)
9. Sanity: go back to Campaigns list, confirm the new campaign appears with status `draft` (status defaults to draft; flip to `active` separately if you want it to count in progress tracking / NFT classification)

### Merge when verified
```
git checkout develop
git merge --no-ff feature/campaign_from_event
git branch -d feature/campaign_from_event
```

---

## 7. Honest caveats

- **Not browser-tested.** The signal-flow logic is verified by `cargo check` only. There's a small chance the `Effect::new` consume-on-mount fires at the wrong time (e.g. if Leptos batches the section switch + payload set in a way that defers the CampaignsPage mount past the Effect's first run). The defensive design (payload is set BEFORE section switch in the same synchronous handler; `<Show>` remounts the component) should make this robust, but only the browser test in §6 confirms it.
- **Not merged.** Lives on `feature/campaign_from_event`. `develop` and `main` are unchanged.
- **No backend changes** — but the campaign id collision behavior is the backend's responsibility. If `{event.id}-campaign` already exists as a campaign id, `create_campaign` will fail with whatever D1 unique-constraint error the handler surfaces. The default suffix `-campaign` makes this unlikely but not impossible.
- **Pre-fill is minimal by design.** Only id, title, and reward_type are pre-filled. Description, organization_id, completion_criteria, and reward_config (NFT collection mint etc.) are left blank for the organizer to fill. This was a deliberate "easiest path" decision, not a limitation — richer pre-fill would require fetching `EventDetail` (an extra async hop on click, like the Edit button does).