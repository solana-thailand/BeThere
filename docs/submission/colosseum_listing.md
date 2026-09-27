# Colosseum / Earn listing text — updated 27 Sep 2026

Builds on `solana-thailand-devrel-helper/reports/phase-2/COLOSSEUM-DESCRIPTION.md`
(19–20 Sep), which checked the old listing claim by claim and wrote the
replacement. That draft's figures and wording are kept here. What changed
since then is the work of the window (`docs/submission/built_in_window.md`):
the escrow ran end to end on devnet, an AI agent paid into it, and the slip
agent is on prod in shadow mode.

**Owner-gated.** Nothing is published. Re-cut every figure marked *DevRel*
on the day you submit, because they move (493 registrations was 474 a day
earlier).

## Tagline (36 chars, unchanged)

```
Get your deposit back by showing up.
```

## Description — short

```
BeThere holds a refundable deposit and returns it at the door. Across 16 events in Bangkok: 51 deposits, 25,000 THB, in-person turnout from 37% to 86%, and no deposit ever forfeited. Built this month: an AI agent that registers and pays the deposit into a Solana escrow from its own wallet, the full deposit, check-in and refund loop running on devnet, and a slip checker that proposes while the organizer decides.
```

If the form refuses its length, cut the last clause (the slip checker)
first. The turnout and "never forfeited" lines stay: no one else can claim
them.

## Description — full

> A free event has no way to price commitment. Charge for a ticket and you
> exclude the people you most want in the room; charge nothing and 42 of 67
> booked seats stay empty, which is what happened at our first meetup.
>
> BeThere takes a small refundable deposit and gives it back at the door.
> Across 16 events in Bangkok: 51 deposits, 25,000 THB of other people's
> money, and in-person turnout went from 37% to 86%. **No deposit has ever
> been forfeited**: the point is attendance, not revenue. A no-show can carry
> the deposit to the next event as credit, and the next registration spends
> it with nobody in the loop.
>
> Until now a human decided whether your money came back. In this hackathon
> we moved that decision on chain and let agents use it:
>
> - **The escrow loop runs end to end on Solana devnet**: deposit → check-in
>   → refund, with the deposit account closed afterwards. Once you are
>   checked in, the program refuses to let the organizer claim your deposit.
> - **An AI agent can commit on someone's behalf.** Our MCP server lets
>   Claude find an event, register a person and pay the USDC deposit into
>   the escrow from the agent's own wallet. BeThere never holds the key.
> - **Humans paying in baht get an AI-assisted check.** The bank QR on each
>   transfer slip is read in the browser, and deterministic checks propose
>   accept or review. A reused bank reference is refused by the database.
>   The organizer still decides.
>
> What has not happened yet is a real deposit going through the escrow: real
> deposits are THB today, and mainnet is the next step. Beyond meetups, the
> same primitive fits anywhere a no-show costs somebody money: clinics,
> workshops, restaurant tables, government appointments.

## Every figure and claim, and where it comes from

| Figure / claim | Source | Re-cut? |
|---|---|---|
| 42 of 67 empty at the first meetup; 37% → 86% | DevRel `facts.yml` (`no_deposit_rtm1`, `deposit_era_rtm3_5`), in-person denominators | re-read |
| 16 events · 51 deposits · 25,000 THB | DevRel `deposits/STATEMENT.md`: historical, RTM #3–#5. **Not** a live query (RTM #3 rows were purged) | say "historical" if asked |
| No deposit ever forfeited | DevRel `forfeits: null` on every event; owner rule 2026-09-17 (THB flow) | re-read |
| Escrow loop end to end on devnet | `scripts/e2e_devnet_test.sh` ALL PASSED 27 Sep; deposit `5xPB2EbL…`, check-in `4VtB7Ctj…`, refund `QgsveL7u…` (full signatures in `built_in_window.md`) | — |
| "the program refuses to let the organizer claim your deposit" once checked in | `bethere-escrow/src/instructions/claim_forfeited.rs`: `constraints(!attendee_deposit.checked_in())`. **No-shows can forfeit USDC on this program** after the refund deadline, so do not write "no one can keep the deposit" about the escrow | — |
| Agent pays from its own wallet | `bethere-mcp`; Claude run, tx `v9H1A84w…` finalized; the agent's keypair never leaves the machine | — |
| Slip QR read in the browser; reused reference refused by the database | `frontend-leptos/js/slip_qr.js`; migration `0054` (`bank_ref UNIQUE`); prod shadow mode since 27 Sep | — |
| "The organizer still decides" | shadow mode: `slip_agent` writes proposals only (guards in `worker/tests/slip_duplicate_guards.rs`) | — |
| No real deposit through the escrow yet | deposits are THB/PromptPay; the escrow is on devnet, not mainnet | — |

## Words not to use

- **"instantly"**: THB refunds are manual.
- **"supports restaurants"**: that is the vision, not a feature.
- **"AI verifies payments"**: the checks are deterministic, and the
  organizer decides.
- **"no one can keep the deposit"** (about the USDC escrow): no-shows can
  forfeit there.
- **"mainnet escrow"**: devnet only. Badges are on mainnet via Crossmint;
  the escrow is not.

## Prior-work disclosure (required; paste into the form)

```
BeThere existed before this hackathon: first commit 21 Apr 2026, 1,167 of 1,389 commits before 14 Sep, real events in Bangkok since spring, and a previous Colosseum entry (Frontier, Consumer Apps). Pre-existing: THB deposits with manual refunds, rolling credit, QR check-in, badge claims, and the escrow program on devnet. Built in this window (222 commits): the MCP server that lets an AI agent register and pay the escrow deposit from its own wallet, the escrow loop running end to end on devnet, the slip checker (bank QR plus deterministic checks, shadow mode on prod), linked emails with one badge per person and per wallet, and organizer tooling for postponements.
```

## Still open before publishing

1. The owner reads and agrees the text.
2. Re-cut the DevRel figures on the day.
3. Earn form Q "Did you submit to the Frontier Hackathon?": ask the track
   contact (plan 026 §3, plan 033 §4 Q5, parked).
4. Link the videos: product demo `bethere_product_demo.mp4` (2:56). The
   founder presentation video is still owed.
