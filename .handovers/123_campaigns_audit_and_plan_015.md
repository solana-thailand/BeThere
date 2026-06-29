# Handover 123 — Campaigns Audit + Plan 015 (Campaigns UX Completion)

**Date:** 2026-06-27
**Branch:** `feature/campaigns_guide` (off `main` @ `86526fc`, NOT yet merged)
**Commits:** `a9172ea` (campaigns guide), `4b3bd2d` (plan 015)
**Outcome:** ✅ Two doc deliverables produced from a ground-truth code audit of the Campaigns feature. NO code changes. NOT yet merged to `develop`/`main`.
**Test delta:** 0 (documentation-only session).

---

## 1. What happened

Continuing from handover #122 (the "Promote to Campaign" feature, deployed + merged to main), the user asked: *"I didn't understand what campaign setting should do?"* — then *"can you do docs to guide how to do it? or if you find there are better UX for users please note that."*

I did a ground-truth read of the campaign machinery (not just the original intent from handover #095) and produced two deliverables:

1. **`docs/campaigns_guide.md`** — a 380-line guide covering the end-to-end loop, setup, field reference, attendee experience, troubleshooting, and **8 prioritized UX gaps**.
2. **`.plans/015_campaigns_ux_completion.md`** — a focused implementation plan with the P0 claim-button fix fully spec'd.

### Key audit findings (the substance)

- **The backend is complete.** All three "remaining work" items from handover #095 have since been implemented:
  - Auto-progress on check-in: `on_event_checkin` (`worker/src/db/campaigns.rs:518`)
  - Claim-reward mints a cNFT via Helius: `claim_campaign_reward` (`worker/src/handlers/campaigns.rs:551`)
  - Leaderboard 3× scoring: `campaign_collection_mints` (`worker/src/db/campaigns.rs:136`)
- **The blocker (🔴 high impact):** the reward loop is unreachable from the UI. `claim_campaign_reward()` is implemented end-to-end (API client + backend mint) but **no button calls it.** The dev dashboard shows *"Reward available to claim!"* as static text (`frontend-leptos/src/pages/dev_dashboard.rs:280`). Attendees cannot mint their reward through the app.
- **Smaller footguns (🟡 medium):** "Add event" requires typing a raw event ID (no dropdown); `completion_criteria` is a free-text field that's stored but never enforced (the real rule is hardcoded: attend all required events); default `draft` status silently disables tracking until flipped to `active`.
- **Minor (🟢 low):** `collection_mint` has no validation; `reward_type: badge` is accepted but unsupported; no public campaign discovery page; progress view shows counts not specific events.

---

## 2. Where is the plan / code / test

| Artifact | Path | Purpose |
|----------|------|---------|
| Campaigns guide | `docs/campaigns_guide.md` | Full reference: loop, setup, fields, troubleshooting, UX gaps (§7) |
| Docs index | `docs/README.md` | Updated — guide listed under "Operations" |
| Plan 015 | `.plans/015_campaigns_ux_completion.md` | Implementation plan: P0 claim button + P1 polish |
| This handover | `.handovers/123_campaigns_audit_and_plan_015.md` | Session record + next-thread entry point |

**No code changed in this session.** All findings are documented; the fixes are scoped in plan 015.

---

## 3. What's next (the answer to "what's next?")

**Plan 015 is the next-thread entry point.** Suggested execution order for a fresh agent:

1. **Read** `docs/campaigns_guide.md` §2 + §7 (~10 min) for the full picture.
2. **Implement P0 — the claim button** (`frontend-leptos/src/pages/dev_dashboard.rs` ~line 280). It's the only item that makes the campaign feature actually deliver rewards. ~30-50 lines. Pre-work reads + acceptance criteria are in plan 015 §P0.
3. **Verify** against test data per plan 015's P0 acceptance checklist.
4. **Run** `cargo check --target wasm32-unknown-unknown` and `~/.cargo/bin/trunk build --release`.
5. **Browser-test P0** (needs OAuth — see guide §5).
6. If time remains, do the two P1 items (event picker dropdown, `completion_criteria` clarity).
7. Commit each slice on its own `feature/...` branch per gitflow; write handover #124 when done.

### Priority summary

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| 🔴 P0 | Claim button (dev_dashboard.rs) | ~30-50 lines | Makes the reward loop reachable — feature is half-shipped without it |
| 🟡 P1 | Event picker dropdown (campaigns_page.rs) | ~20 lines | Removes id-vs-slug footgun |
| 🟡 P1 | `completion_criteria` clarity | ~1 line (placeholder) | Stops a misleading field |
| 🟢 P2 | Draft-status warning, remove `badge` option, richer progress | small | Polish |

---

## 4. Reflection — struggling / solved

### Solved: did a ground-truth code read instead of trusting old handovers
Handover #095 listed auto-progress, claim-reward NFT minting, and leaderboard classification as "remaining work." I verified the **current** code state rather than parroting the old handover, and found all three items have since been completed. The audit deliverables reflect current reality, not stale intent.

### Solved: surfaced the claim-button gap honestly
The campaigns feature *looks* complete from the admin UI (everything configures cleanly), and the backend is genuinely complete. The gap only surfaces when you trace what *calls* `claim_campaign_reward` — which is nothing. Flagged this as the single highest-impact fix rather than burying it in a list.

### Solved: scoped the next work for clean handoff
Wrote plan 015 with pre-work reads, file paths, acceptance criteria, and test-data setup — so a fresh agent can implement P0 without re-doing the investigation. Pointed at `claim.rs` as the pattern reference (it already solves "get wallet + call mint endpoint").

### No real struggles
The investigation was straightforward tracing; the deliverables are documentation.

---

## 5. Remaining work (this session + prior)

### From this session
- [ ] Merge `feature/campaigns_guide` → `develop` → `main` (guide + plan 015; doc-only, no conflicts expected)
- [ ] Implement plan 015 P0 (claim button) — next thread

### Outstanding from prior work (not this session's scope)
- **Browser-test "Promote to Campaign"** (handover #122 §6): deployed to prod, not click-tested. Needs human + admin OAuth.
- **Plan 004 remaining**: 8 checkboxes (3 Docker-blocked, 4 browser-needed, 1 e2e-tx).
- **`develop` lacks Solana Mobile demo** (`c2a1309` lives only on `main`): pre-existing asymmetry.
- **185 pre-existing clippy errors** (Rust 1.96 lints on old code): worth a `cargo clippy --fix --allow-dirty` pass.

---

## 6. Issues ref

This session is **not** tied to a numbered `.issues/` entry — it was an audit + documentation pass prompted by the user's "what does campaign setting do?" question. Related prior work:

- **#049 Phase 3** (`campaigns_series_phase3`) — original Campaigns backend + admin UI (handover #095).
- **#051** (`campaign_nft_rewards`) — campaign NFT classification, 3× leaderboard (handover #096).
- **Plan 013** (`event_series_navigation`) — public prev/next nav within a campaign; thematically adjacent.

---

## 7. How to dev / test

This session produced docs only — no build/test steps. To act on the deliverables:

### Merge this branch
```
git checkout develop
git merge --no-ff feature/campaigns_guide
git checkout main
git merge --no-ff develop
git push origin develop main
git branch -d feature/campaigns_guide
```

### Implement plan 015 P0 (next thread)
See `.plans/015_campaigns_ux_completion.md` §P0 for full details. Short version:
1. Read `frontend-leptos/src/pages/claim.rs` (pattern reference for wallet + mint).
2. In `frontend-leptos/src/pages/dev_dashboard.rs` ~line 280, replace static "Reward available to claim!" text with a button that calls `api::claim_campaign_reward(&item.campaign_id, &wallet_address)`.
3. Refresh progress on success; toast errors readably.

---

## 8. Honest caveats

- **Not merged.** Lives on `feature/campaigns_guide`. `develop` and `main` are unchanged from handover #122's end state (`main` @ `86526fc`).
- **No code verification needed** (docs only), but the findings in the guide/plan are grounded in reading the actual current code — not the original intent. The "remaining work" claims in handover #095 are explicitly contradicted where the code shows otherwise.
- **The claim-button gap (P0) is real and load-bearing.** Until it's implemented, the entire campaign reward loop is unreachable from the UI — every campaign an organizer configures with `reward_type: nft_certificate` produces a reward that attendees can see but not mint. This is the highest-priority follow-up.