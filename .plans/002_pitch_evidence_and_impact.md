# Goal: Evidence-grade the pitch deck's numeric claims + add SDG 12.3 impact framing

Every external statistic in the deck currently has **zero citations** (verified by grep across `docs/`). The deck also has at least one **internal math inconsistency** (`990×` vs the stated `$0.50` POAP cost). This plan makes the deck (a) internally consistent, (b) externally defensible, (c) self-verifying, and (d) honestly framed for impact — without fabricating any source.

## Context

- Deck generator: `scripts/make_pitch_deck.py` → `.deliverables/bethere-pitch.pptx` (17 slides, 16:9)
- Audit finding: `grep -E "source:|citation|http|reference"` across all 25 `docs/*.md` → **no matches**
- Caught inconsistency: deck claims both `~$0.50` (POAP) *and* `990× cheaper` for the `$0.001` cNFT, but `$0.50 / $0.001 = 500×`, not `990×`
- Self-metrics currently hand-maintained — drift risk (already fixed once by hand in commit `8da9eaa`)
- No `docs/sources.md` evidence ledger exists
- SDG angle not yet represented; strongest genuine link is **SDG 12.3** (food-waste reduction) via the deck's existing *"Catering wasted"* problem statement

## Non-negotiables

- **Do not fabricate any citation.** If no verifiable primary source is found for a claim, record it as `UNVERIFIED` in the ledger and flag for the user. A fake citation an investor can debunk is worse than no citation.
- **Every change to the deck must be rebuilt and visually confirmed** (rebuild via `python3 scripts/make_pitch_deck.py`).
- **Scope creep guard:** if a research step goes deep, surface what's found so far and let the user decide whether to continue.

## Tasks

### Phase 1 — Fix internal inconsistencies

- [x] **1.1 Reconcile the `990×` claim.**
      Decide which figure is authoritative:
      - If POAP cost = `$0.50` → multiplier should be `500×`
      - If multiplier should stay `990×` → POAP cost should be `~$0.99`
      - Preferred: keep `990×` if defensible (requires real POAP mint cost source, see Phase 2), else correct to `500×` and align all 3 occurrences
      - Audit all occurrences: `slide_02_problem`, `slide_08_performance`, `slide_10_competitive`
      RESULT: Reframed to Option A (POAP-on-Gnosis like-for-like, ~50×) across all 6+ occurrences (deck + docs). Commits efd6be5, 9d3a12f.
- [x] **1.2 Rebuild deck + confirm consistency** across all slides mentioning POAP cost and multiplier.

### Phase 2 — Source external claims

- [x] **2.1 Research primary source for the `30–40%` no-show stat. — WELL-SUPPORTED, deck claim is conservative.**
      Result: Multiple independent sources corroborate free-event no-show rates in the 20–60% range. The deck's `30–40%` sits comfortably in the middle of the documented range (defensible, not aggressive).
      - **realevents.co** (cites **Eventbrite 2024 Event Trends Report**): "free events experience no-show rates between **20% and 50%**"; paid events "typically between **5% and 10%**". Community meetups/networking at the higher end. — *Strongest named primary source: Eventbrite 2024 report (traceable).*
      - **whos-in.app** (2026): "Free events see **40–60% no-shows**, paid events 10–20%".
      - **conferencesthatwork.com** (2025): free events "attendance rates are often **well below 50%** of registrations".
      - **grabmyslot.com** (2026): aggregate no-show "between 10 and 20 percent... without a formal deposit"; "with a deposit requirement... fall to **3 to 7 percent**". — *Directly supports BeThere's deposit value proposition.*
      - Action for ledger: cite Eventbrite 2024 Event Trends Report (via realevents.co) as primary; note secondary corroboration. Deck `30–40%` can stay as-is — it's conservatively within range.
- [x] **2.2 Research POAP/Ethereum NFT mint cost — CRITICAL FINDING: deck comparison is partially misleading (strawman).**
      Result: POAPs are **NOT minted on Ethereum mainnet in practice**. The `$0.50` / `990×` framing in the deck compares against a configuration POAP explicitly avoids.
      - **POAP mints on Gnosis Chain by default, FREE for attendees** — POAP.inc absorbs the cost across Gnosis / Base / Polygon / Celo. Sources: blockleaders.io ("No gas fees are required, POAP absorbs the costs"), binance.com, bitget.com, zenao.io all confirm Gnosis-default, free-to-attendee.
      - **Organizer cost on Gnosis/L2: ~$0.05–$0.20 per mint** (chainscorelabs.com: "costing organizers ~$0.05-$0.20 per mint").
      - Ethereum mainnet POAP is technically possible but "would make issuing thousands of free badges unsustainable" (bitget.com) — POAP avoids it by design.
      - **Impact on deck claims:** `990× cheaper` and `~$0.50` both assume Ethereum mainnet POAP — a strawman. A fair comparison (BeThere cNFT `$0.001` vs POAP on Gnosis `~$0.05–$0.20`) is only **~50–200× cheaper**, not `990×`.
      - **Credibility risk:** an investor who knows POAP will immediately flag this. Needs rework (see 2.4), not just a multiplier tweak.
- [x] **2.3 Research Luma / Eventbrite feature matrix** (deposit support, auto-refund, NFT badges) with a snapshot date — supports the competitive-table claims.
      RESULT: All 8 cells verified No against official help docs (snapshot 2026-06-15). Deck claim holds at the mechanism level.
- [x] **2.4 Decision point — RESOLVED: user chose Option A (POAP-on-Gnosis like-for-like, ~50×).**
      Surfaced to user: the `990×` / `$0.50 POAP` claim needs reframing, not a numeric patch. Options to decide before any deck edit:
      - **(A) Honest like-for-like:** compare cNFT `$0.001` vs POAP-on-Gnosis `~$0.05–$0.20` → "up to 50× cheaper" (defensible, modest).
      - **(B) Recompute vs Ethereum-mainnet generic NFT mint** (not POAP) at a cited gas price → keeps a large multiplier but changes the comparator from "POAP" to "Ethereum NFT".
      - **(C) Reframe the axis entirely:** drop the cost-multiplier headline and emphasize cNFT scalability / Solana-native UX instead of a dubious ×figure.
      - Recommendation: **(A)** — honest, still favorable, survives scrutiny. Awaiting user decision.
      - Note: `30–40%` no-show stat needs no softening (well-sourced per 2.1).

### Phase 3 — Automate self-metrics

- [x] **3.1 Write `scripts/measure_metrics.py`** that regenerates, from the live codebase: (commit 533bb3a)
      - [ ] Program size: bytes of `bethere-escrow/target/.../*.so` (built artifact)
      - [ ] Test count: parse `cargo test -p ... 2>&1` output for "X passed"
      - [ ] Kani harness count: count `#[kani::proof]` in `bethere-escrow/src/kani.rs`
      - [ ] Fee derivation: compute cNFT + TX cost from current Solana fee schedule (document assumptions)
      - [ ] Latency: flag as `NEEDS BENCHMARK` (can't measure without a running edge worker)
- [x] **3.2 Run it**, capture actual values, diff against deck claims.
- [x] **3.3 Reconcile any drift** between measured values and what the deck/docs assert.
      RESULT: test count 287→250+ (executed) reconciled across deck+docs+script; POAP strawman extended to docs. Commit 9d3a12f.

### Phase 4 — Build evidence ledger

- [x] **4.1 Create `docs/sources.md`** mapping every numeric claim to its proof: (commit 091e3d9)
      - Columns: `claim | category (external/self/comparison) | value | source | date checked | confidence | verification method`
      - External claims → primary source URL + quote + retrieval date
      - Self claims → `scripts/measure_metrics.py` output line / CI ref
      - Comparison claims → documented methodology + assumptions
- [x] **4.2 Cross-link** from `README.md` and `DEMO.md` "stats" sections to `docs/sources.md` so future edits have a single source of truth.
- [x] **4.3 Commit ledger + metrics script** (`docs(sources): add evidence ledger + metrics script`).

### Phase 5 — Add SDG 12.3 impact framing

- [x] **5.1 Decide representation** — user pre-approved the default single-line.
      Default proposal: a single SDG-12.3-aligned line on the **Problem** slide tying the `30–40%` stat to food-waste reduction — *only after* Phase 2.1 sources the stat it rests on.
- [x] **5.2 Implement the chosen representation** in `scripts/make_pitch_deck.py`. (commit 89ac8d7)
- [x] **5.3 Rebuild + reopen** deck for visual review.
- [x] **5.4 Add the SDG claim to the evidence ledger** with its causal chain documented (no-show reduction → less over-catering → less food waste → SDG 12.3). (docs/sources.md §5)

## Notes

- **Dependency order:** 1 → 2 → 3 → 4 → 5. Phase 5's credibility depends on Phase 2's sourcing.
- **Tooling:** use `fetch` for primary-source research; use `rg` / `bat` / `eza` (per environment rules), not classic `grep`/`cat`/`ls`.
- **No fabrication:** any claim left without a source after Phase 2 is marked `UNVERIFIED` and surfaced for the user to source or soften — never invented.
- **Drift prevention:** Phase 3's script is what stops the next docs/deck drift (the kind we just fixed manually in `8da9eaa`).
````
