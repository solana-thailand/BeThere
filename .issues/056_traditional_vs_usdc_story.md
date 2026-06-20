# 056 — Traditional vs USDC Journey: Cartoon Storyteller Comparison

> **Date**: 2026-06-20
> **Status**: 📋 Planned — awaiting 4 design decisions in §3 before drafting
> **Priority**: P1 for Demo Day 2026-06-23 (narrative content for the pitch)
> **Depends on**: nothing technical. Pure content/design deliverable.
> **Target completion**: 2026-06-21 (one day before submission deadline)

---

## Summary

A side-by-side cartoon/storytelling comparison of the **traditional event check-in flow** (bank transfer, manual reconciliation, paper sheets) versus the **BeThere + USDC escrow flow** (on-chain deposit, instant verify, auto-refund, cNFT badge). Intended as pitch material for IslandDAO V4 demo (2026-06-23).

Format TBD pending operator decision (see §3.1) — options range from static slide deck to embedded HTML feature.

## Motivation

The BeThere pitch needs a sharp "why does this matter" narrative. The technical architecture (Solana escrow PDAs, cNFT minting, R2 storage) is impressive to engineers but invisible to judges. A side-by-side cartoon showing the **human experience** of each flow makes the value concrete:

- **Pain**: "I registered, paid via bank transfer, sent the screenshot to admin, waited 2 days for confirmation, showed up, they couldn't find my name, eventually got in, never got my deposit back when I cancelled."
- **Relief**: "I registered, deposited USDC in 10 seconds, showed my QR, got in instantly, claimed an NFT badge, cancelled the next day and the deposit auto-refunded."

This contrast is the heart of the pitch. It needs to be **visual, fast, memorable**.

## Scope

### In scope

- A narrative arc (6–10 beats) walking through both journeys in parallel
- Captions / dialogue per beat (short, punchy)
- Visual direction (character design, palette, composition guidance)
- Source content that can adapt to multiple output formats (slides, HTML, PDF)

### Out of scope (unless explicitly requested)

- Building a BeThere admin feature to auto-generate per-event journey comics
- Animation / video
- Localization (English only for demo)
- Hand-drawing the actual artwork (unless operator confirms illustration budget/time)

---

## 1. Proposed narrative arc (draft — operator to edit)

| Beat | Traditional | BeThere + USDC | Emotion |
|---|---|---|---|
| 1. **Register** | WhatsApp the organizer, fill a Google Form, wait for "ok added" | Magic link, Google sign-in, instant confirmation | Friction vs ease |
| 2. **Deposit** | Bank transfer, screenshot, DM the admin, "did you get it?" | Scan Solana Pay QR, USDC lands in escrow PDA, on-chain proof | Anxiety vs confidence |
| 3. **Verification** | Admin manually matches screenshot to bank statement, updates Sheet | Smart contract holds the funds; server reads on-chain state | Manual labor vs automation |
| 4. **Show up** | "Name please?" → scroll through Sheet → "ah yes here you are" | Staff scans QR → green checkmark, instant verify | Awkwardness vs flow |
| 5. **The no-show** | "Sorry you couldn't make it, I'll refund you next week" (often never happens) | Taps "Refund" → smart contract releases USDC from escrow instantly | Loss vs fairness |
| 6. **The badge** | Nothing. Maybe a paper wristband. | cNFT minted into wallet. Permanent. Verifiable. Shareable. | Disposable vs permanent |
| 7. **The audit** | "How many people actually paid?" → open Sheet → cross-reference bank CSV → hours later, a number | Open Solana Explorer → filter escrow program → instant, immutable count | Opaque vs transparent |

**Total: 7 beats.** Compressible to 5–6 for time-constrained pitches.

---

## 2. Visual direction (proposed — operator to confirm)

### Style options

| Style | Pro | Con | Effort |
|---|---|---|---|
| **Stick figures** (XKCD-style) | Fast, universal, on-brand for "developer humor" | Can feel cheap if executed poorly | Low (1–2h) |
| **Geometric / flat illustration** | Professional, matches BeThere's `style.css` design system (`--bg-primary: #13131b`, card-based UI) | Requires more craft | Medium (3–4h) |
| **Pixel art** | Nostalgic, fun, distinctive | Time-consuming to do well | High (5h+) |
| **Photo + caption** (screenshots of real flows) | Most honest, fastest | Less memorable, less emotional | Lowest (1h) |

**Recommendation**: geometric/flat illustration matching the existing BeThere palette. Reuses design system, feels cohesive, professional enough for judges, achievable in a half-day.

### Palette (from `style.css`)

- `--bg-primary: #13131b` (deep navy)
- `--bg-secondary: #1a1a24`
- `--bg-card: #1e1e2a`
- Solana purple/green accents for the "BeThere" side
- Muted gray for the "Traditional" side

### Composition

Each beat = one panel. Two columns (Traditional | BeThere). Same character appears in both, experiencing the contrast. Optional: third column or annotation showing the "what's actually happening under the hood" (TX hash, PDA, etc.) for the technical-judge audience.

---

## 3. Decisions (locked 2026-06-20)

| # | Decision | Implication |
|---|---|---|
| 3.1 | **Keynote slide format** | Need to produce a `.key` deck (or Markdown → export to Keynote). Visual style TBD by operator when drafting. |
| 3.2 | **Mixed audience** (judges + attendees) | Plain English with light jargon. Explain "escrow" as "smart contract holds the money". Use "NFT badge" not "cNFT". |
| 3.3 | **Reframe: 3-way comparison** — not just traditional vs BeThere, but **traditional + modern competitors (Eventbrite, Luma, Meetup)** vs BeThere. | See §3.3 expanded below — this is the key narrative shift. |
| 3.4 | **Vendor wallet logos OK to use** | Solflare/Backpack/Phantom logos allowed on the BeThere side for credibility. |

### 3.3 The 3-way comparison (reframed)

Original framing was 2-column (Traditional | BeThere). Operator insight: BeThere isn't just better than paper-and-WhatsApp — it has to **stand out against the modern competitors organizers already use**. The contrast needs a third column.

#### Proposed 3-column structure

| Beat | Traditional (WhatsApp + bank transfer) | Modern competitor (Eventbrite / Luma / Meetup) | BeThere + USDC |
|---|---|---|---|
| 1. Register | WhatsApp + Google Form | Web form, account required | Magic link, no password |
| 2. Deposit | Bank transfer, screenshot | Credit card (2.5–3.5% fee + Stripe) | USDC escrow, 0.0001¢ fee |
| 3. No-show refund | Manual bank transfer, days/weeks | Manual via dashboard, days (or "no refunds" policy) | **Smart contract auto-refund, instant** |
| 4. Show up | Scroll through Sheet, name lookup | QR scan, online DB lookup | QR scan + on-chain verify |
| 5. Badge / takeaway | None | Email receipt | **cNFT in wallet, permanent** |
| 6. Payment rails | Bank (closed, slow, permissioned) | Stripe (closed, fast, 2.9% + chargeback risk) | **USDC (open, instant, no chargebacks)** |
| 7. Custody of funds | Admin's bank account | Eventbrite holds funds for 5–7 days post-event | **Escrow PDA — neither side holds** |
| 8. Audit | Bank CSV + Sheet reconciliation | Eventbrite dashboard (opaque) | **Solana Explorer (public, immutable)** |

#### The 3 sharp differentiators vs modern competitors

These are where BeThere wins against Eventbrite/Luma/Meetup, not just against paper:

1. **Payment fees** — Eventbrite charges 2.5% + $0.99 + Stripe's 2.9% + 30¢. On a $50 ticket that's ~$3.25 lost. BeThere: USDC transfer costs ~$0.001. **Story: where does the 6.5% go?**
2. **Custody** — Eventbrite/Luma hold your funds for days after the event. BeThere's escrow PDA is non-custodial: neither organizer nor platform can run away with deposits. **Story: trust the code, not the company.**
3. **No-show refund automation** — On Eventbrite, refunds are a manual organizer decision (and many events have "no refunds"). On BeThere, the smart contract releases the deposit on check-in and refunds on no-show automatically. **Story: fairness isn't a policy, it's a guarantee.**

#### What the modern-competitor framing buys us

- Judges who have used Eventbrite/Luma immediately get the contrast (no need to explain what paper tickets are).
- Positions BeThere as "the modern option, but better" rather than "better than the past".
- Opens the door to a future positioning slide: "BeThere = Eventbrite UX + Solana settlement."

#### Risk to flag

Eventbrite has features BeThere doesn't (paid tickets, marketing tools, discovery). Don't claim "BeThere replaces Eventbrite" — claim "BeThere replaces the *payment + deposit* layer with something better" for the deposit-gated event niche. The pitch is sharper if scoped honestly.

---

## 4. Effort estimate (after decisions)

| Task | Effort |
|---|---|
| Lock narrative arc (edit §1) | 30 min |
| Write final captions | 1h |
| Choose/scaffold format | 30 min |
| Produce visuals (depends on style choice §2) | 2–5h |
| Review + revise | 1h |
| **Total** | **4–8h depending on visual complexity** |

Fits a half-day to full-day. Recommend scheduling for 2026-06-21 (one day before submission), in parallel with the tail end of mobile testing.

---

## 5. Relationship to Demo Day plan

| Day | Work |
|---|---|
| 2026-06-20 (today) | Mobile Phase A (plan #011) |
| 2026-06-20 EOD | Mobile Phase B |
| 2026-06-21 AM | Mobile on-device testing + bug fixing |
| 2026-06-21 PM | **This issue: draft + produce the comparison** |
| 2026-06-22 | Rehearsal + buffer |
| 2026-06-23 | Demo |

**Do not start this issue until mobile Phase A is at least functionally working.** Mobile is the higher-demo-impact item; this is supporting narrative.

---

## Refs

- `DISCUSSION.md` §8 — wallet-signed operations (the flows being contrasted)
- `DISCUSSION.md` §10 — attendee journey by format
- `frontend-leptos/style.css` — design system palette
- Plan 011 — Solana Mobile Demo Day slice (parallel work)

## Related issues

- #042 — Solana Mobile support (the technical enabler for the "show up + deposit on phone" beat)
