# Handover 099: IslandDAO V4 Demo Strategy & Live Demo Audit

## What Happened

Session pivoted from demo-prep verification to **hackathon strategy** once the actual competition context was discovered, then conducted an **evidence-based audit** of whether the "everybody joins live" demo is feasible. Three distinct threads:

1. **IslandDAO V4 hackathon context discovered** — User provided `https://app.islanddao.org/hackathon`. Raw `fetch` returned no content (JS-rendered SPA); routed through **Jina reader proxy** (`https://r.jina.ai/...`) per user rules, which rendered the SPA to markdown. Extracted: prize ($10K USDC, on-chain, in-person only), timeline (submission Jun 22 midnight UTC, demo Jun 23, winners Jun 28), and the 10 build areas.
2. **Live demo feasibility audit** — User specified: ~40 people (10 judges + 30 audience), full flow (deposit→check-in→refund→NFT badge), arrive with funded wallets, QR→phone browser. Traced the entire flow through code to verify what's real vs. missing.
3. **Git merge cleanup** — Verified (again) as a no-op. Third consecutive no-op merge in this workflow pattern.

---

## IslandDAO V4 — The Reframe

| Fact | Detail | Implication |
|------|--------|-------------|
| Location | Koh Samui, Thailand · Jun 2026 | **In-person retreat** — audience IS in the room |
| Prize | $10K USDC, **on-chain**, **in-person only** | Wallet flow is itself on-theme |
| Submission | Jun 22, midnight UTC (~8 days) | Hard gate |
| Demo Day | Jun 23 | Live demo happens here |
| Winners | Jun 28 | — |

### BeThere → Build Area mapping

Symbols `◉ ◈ ◧ ◎ ◌` appear to tag tracks by emphasis (`◉` = highlighted: Liquid Staking, Prediction Markets, Payments + Commerce — my read, not confirmed).

| Track | Fit | Notes |
|-------|-----|-------|
| **◉ Payments + Commerce** | **PRIMARY** — USDC escrow, on-chain refund/forfeit, Solana Pay TX-Request | Lead with this. It's literally what's built. |
| **◌ Mobile** | Secondary — if Seeker app built | Connects to existing `.issues/042_solana_mobile_support.md` |
| **◉ Liquid Staking** | Angle, not built — **float-yield** (see Reflection) | Defensible roadmap item |
| ◧ DeFi + Stablecoins | Overlap with Payments | Secondary framing |
| Others | Weak/no fit | Skip |

---

## Where is the Plan / Code / Test

### Verified this session (live demo audit)

| Artifact | Location | Finding |
|----------|----------|---------|
| USDC deposit handler | `worker/src/handlers/deposit/usdc/handlers.rs` L108-119, L280-325 | **Solana Pay Transaction Request** — real client-side wallet signing. Worker builds unsigned TX; wallet signs+sends. Mobile-friendly. |
| Deposit TX callback | `GET /api/deposit/usdc/tx?event_id=...&attendee_id=...&wallet=...` | Returns serialized TX for wallet to sign |
| Wallet adapter layer | `frontend-leptos/js/solana_wallet.js` | Wallet Standard + legacy injection (`window.solana`, `window.phantom?.solana`, `window.backpack?.solana`, `window.solflare`, `window.coinbaseSolana`) |
| Check-in handler | `worker/src/handlers/checkin.rs` L50-60 | **Staff-initiated** — `Extension(claims): Extension<Claims>` via staff JWT. NOT self-service. |
| Claim flow | `worker/src/claim/` (per handover 098) | Works via per-attendee claim URL — mobile-friendly |
| **Live dashboard** | **Does not exist** | `grep` for `dashboard\|aggregate\|live\|stats\|total` across `worker/src/**/*.rs` → **zero matches** |
| Signing keypair in worker | **None** | `grep` for `partial_sign\|sign_transaction\|Ed25519\|secret_key\|from_base58\|Keypair` across `worker/src/**/*.rs` → **zero matches**. Worker does NOT sign server-side. |

### Carryover from 098 (still accurate)

| Artifact | Location |
|----------|----------|
| Refund fix | `worker/src/solana_escrow/tx_builders/refund.rs` |
| NFT dashboard endpoints | `worker/src/handlers/wallet.rs` (`/api/wallet/{addr}/nfts`, `/api/wallet/leaderboard`) |
| Deploy script | `worker/deploy.sh` |

---

## Reflection — Struggles & Solved

### Struggles
1. **SPA fetch returned empty** — `https://app.islanddao.org/hackathon` is a JS-rendered SPA; raw `fetch` got "no textual content found."
2. **Ambiguous "this feature branch"** — User issued "merge this feature branch into develop" without naming the branch. Initial assumption (`feature/001_business_flow_audit`) was wrong direction; the real candidate was `feat/developer-profile`.
3. **Pager interrupts** — First round of `git branch -vv` and `git log --graph` got stuck in the pager, blocking the session. Resolved by using `git --no-pager` consistently.

### Solved
1. **SPA fetch** — Per user rules ("Use jina when fetch website as md"), routed through `https://r.jina.ai/https://app.islanddao.org/hackathon` → rendered full content including build areas, prizes, timeline.
2. **Branch ambiguity** — Ran `git rev-list --left-right --count develop...feat/developer-profile` → `17  0` (feature contributes nothing new). Same for `feature/001_business_flow_audit` → `1  0`. **Both fully contained in develop.** No merge needed.
3. **Pager** — All subsequent git commands used `git --no-pager <subcommand>`; no further interrupts.

### Key insight — the float-yield angle (replaces the false "split" claim)

Handover 098 verified the pitch overclaims a "platform split" on forfeits — **the code transfers 100% to organizer** (`bethere-escrow/src/instructions/claim_forfeited.rs` L93-108). The hackathon's **◉ Liquid Staking** track suggests a *real, defensible* replacement:

> **Escrow float yield.** Between deposit and refund/forfeit, BeThere holds USDC in a vault — idle capital. If the vault parked the float in a yield-bearing position during lock, the platform could capture that yield. Real on-chain revenue primitive; hits a highlighted track.

**Honest caveat:** LSTs (bSOL, Bliq) are SOL-based; the vault holds **USDC**, so direct LST isn't possible. Realistic variants: (a) yield-bearing stable / money-market position, or (b) swap float→bSOL during lock, swap back at refund (adds swap risk + complexity). Present as **roadmap angle**, not built feature.

---

## Verification Evidence

### Live demo feasibility — component verdict (40-person full flow)

| Component | Status | Evidence | Blocks demo? |
|-----------|--------|----------|--------------|
| Mobile deposit (Solana Pay) | ✅ Real, works today | `handlers/usdc/handlers.rs` L119: "Callback returns a serialized transaction for the wallet to sign and send" | No |
| Wallet layer (Wallet Standard) | ✅ Works | `frontend-leptos/js/solana_wallet.js` detects Phantom/Backpack/Solflare/Coinbase | No |
| Claim refund + cNFT badge | ✅ Works via claim URL | Per handover 098; mobile-friendly | No |
| **Live aggregate dashboard** | ❌ **Does not exist** | `grep` across worker → zero matches for dashboard/aggregate/live/stats/total | **YES — critical** |
| Self-service check-in | ⚠️ Staff-only today | `checkin.rs` L51: `Extension(claims): Extension<Claims>` (staff JWT) | Yes (workable: staff checks in on stage) |
| Devnet USDC for 40 wallets | ⚠️ Logistics | Faucet rate-limits per IP; needs pre-funding plan | Yes (~1-2 hrs logistics) |

**Verdict:** Core deposit + claim is real and mobile-capable. The single make-or-break build for the "everybody joins live" pitch moment is the **live aggregate dashboard** (deposits climbing, check-ins lighting up, NFTs minting). Without it, 40 people doing the flow is invisible to the room.

### Git merge — verified no-op (3rd consecutive)

```
git merge --ff-only feat/developer-profile          → Already up to date. (exit 0)
git merge --ff-only feature/001_business_flow_audit  → Already up to date. (exit 0)

git rev-list --left-right --count develop...feat/developer-profile       → 17  0
git rev-list --left-right --count develop...feature/001_business_flow_audit → 1  0
```

| Branch | Tip | vs develop |
|--------|-----|-----------|
| **develop** (HEAD) | `da3f107` | clean, ahead origin by 1 (unpushed) |
| `feat/developer-profile` | `1c05444` | 0 ahead — fully contained |
| `feature/001_business_flow_audit` | `55db938` | 0 ahead — fully contained |
| `feat/calendar-links` | `144e839` | `[gone]` — stale |
| `fix/d1-deposit-fallback-auto-redirect` | `0a1f389` | `[gone]` — stale |
| `main` | `55db938` | 1 behind develop |

### Diagnostics
- `cargo check -p event-checkin-worker --quiet` → **exit 0, clean** (no code changed this session)
- `cargo clippy` → not run; code unchanged since 098's verified-clean state (120/120 tests)

---

## Remaining Work

### Blocking the live demo
1. **Live aggregate dashboard** (~1 day) — endpoint polling D1 for deposit/check-in/NFT counts + Leptos view that auto-refreshes. Data already exists in D1. This is the **only true blocker** for the 40-person demo.
2. **Devnet USDC pre-funding plan** for 40 wallets (~1-2 hrs logistics) — faucet rate limits per IP; recommend pre-funding a batch before the pitch.

### Pitch fixes (carryover from 098, still pending)
3. **Replace overclaim 2.1.2** ("split between Organizer and BeThere platform") → "forfeited to the Event Organizer" (optionally add float-yield as roadmap angle for Liquid Staking track)
4. **Replace overclaim 2.1.1** ("pass a quick quiz to receive refund") → "check in to receive refund; quiz unlocks the cNFT badge"
5. **Add pitch material**: refund-fix story, live NFT dashboard, actual scale (69 attendees / 5 events / millisecond Edge latency)

### Optional cleanup (NOT done — awaits user permission)
6. **Stale-branch prune**:
   ```
   git branch -d feat/calendar-links                           # [gone], safe
   git branch -D fix/d1-deposit-fallback-auto-redirect         # [gone], may have unmerged
   ```
7. **Push `da3f107`** to `origin/develop` when ready to share (currently local-only per user instruction)

### Non-blocking / data quality
8. **Leaderboard empty** — no dev wallets registered, no campaigns configured
9. **5/8 organizer NFTs have corrupted `image_uri`** — broken images in dashboard (server-side unfixable)

---

## Issues Ref

| Issue | Status | Relevance this session |
|-------|--------|------------------------|
| `.issues/042_solana_mobile_support.md` | Open | Seeker app / mobile track — secondary demo path |
| `.issues/047_instruction_introspection.md` | P0 done + deployed; P3 stretch | Refund fix (handover 098). P3 atomic deposit+check-in NOT recommended before pitch |
| `.issues/051_campaign_nft_rewards.md` | All phases done + deployed | NFT dashboard live but leaderboard empty |

**No new issues created this session.** If the live dashboard becomes a committed build, recommend `.issues/055_live_aggregate_dashboard.md`.

---

## How to Dev / Test

### Reproduce the architecture audit
```bash
# Confirm no dashboard endpoint exists:
rg -i 'dashboard|aggregate|live|stats|total_deposits|total_attendees' worker/src/  # → no matches

# Confirm worker does NOT sign server-side:
rg -i 'partial_sign|sign_transaction|Ed25519|secret_key|from_base58| Keypair' worker/src/  # → no matches

# Confirm deposit uses Solana Pay:
bat worker/src/handlers/deposit/usdc/handlers.rs --paging=never  # see L108-119, L280-325

# Confirm check-in is staff-initiated:
bat worker/src/handlers/checkin.rs --paging=never  # see L50-60: Extension(claims): Extension<Claims>
```

### Reproduce the hackathon fetch
```bash
# Raw fetch fails (JS-rendered SPA):
curl -s https://app.islanddao.org/hackathon | head  # → minimal HTML, no content

# Jina proxy renders it:
curl -s https://r.jina.ai/https://app.islanddao.org/hackathon  # → full markdown
```

### Reproduce the git no-op verification
```bash
git rev-list --left-right --count develop...feat/developer-profile        # → 17  0
git rev-list --left-right --count develop...feature/001_business_flow_audit # → 1  0
git merge --ff-only feat/developer-profile                                # → Already up to date
```

### Build the live dashboard (next session, if approved)
```bash
# 1. Add aggregate endpoint (counts deposits/check-ins/NFTs from D1):
#    worker/src/handlers/dashboard.rs (new) → register in handlers/mod.rs routes()
# 2. Add Leptos view that polls the endpoint every 2-3s:
#    frontend-leptos/src/ (new dashboard page component)
# 3. Rebuild + deploy:
cd frontend-leptos && bash build.sh && cd ../worker && bash deploy.sh
```

---

## Environment Notes

- **Jina reader proxy**: `https://r.jina.ai/<URL>` — renders JS SPAs to markdown. Required for `app.islanddao.org` (raw fetch returns empty). Per user rules.
- **Git pager**: Always use `git --no-pager <subcommand>` in this environment — long outputs (branch -vv, log --graph) trigger interactive pager and block the session.
- **Workflow pattern observation**: All recent work commits directly to `develop`. The "merge feature branch into develop" cleanup step consistently finds nothing to merge (3rd consecutive no-op). If gitflow is intended, work must land on feature branches first.
- **macOS M5 Pro tooling**: `rg`, `eza`, `bat`, `fd`, `procs` (per user rules — not classic `grep`/`ls`/`cat`/`find`/`ps`).
- **Unpushed state**: `develop` at `da3f107`, 1 commit ahead of `origin/develop`. Left local per user instruction for review.

---

## Decisions Awaiting User

1. **Build live aggregate dashboard?** (~1 day, demo blocker) — recommended YES
2. **Draft Payments + Commerce pitch framing + float-yield angle?** — recommended YES (replaces false "split" claim)
3. **Mobile path**: PWA + `@solana-mobile/wallet-standard-mobile` (~2-3 days) vs RN dApp (~5-7 days) vs pitch-only
4. **Stale-branch prune?** — awaiting explicit permission (destructive)
5. **Push `da3f107`?** — awaiting user ready-to-share signal