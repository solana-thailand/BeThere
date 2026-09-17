# Deposit Commitment Model — canonical

> **This is the source of truth for how the BeThere deposit works, what it
> promises, and how the off-chain and on-chain rails compare.** Other docs,
> pitch material and app copy should agree with it. Where they don't, this file
> wins and the other file is stale (see §8).
>
> Written 2026-09-17 against prod `8a3d6d9d`. Every rule below was checked in
> code. Every number was read from the prod D1 on that date.
> Mechanics per endpoint: [deposit-refund-flows.md](deposit-refund-flows.md).

---

## 1. The model in one line

**Put down a deposit to hold your in-person seat. Show up and it all comes back.
Attending costs nothing.**

The deposit is not a ticket price. It turns a free RSVP (which people drop
freely) into a small commitment. The attendee keeps the money. The organizer
only holds it.

```
  1 Reserve        2 Show up        3 Scan QR        4 Get it back
  pay ฿500    +    at the venue  +  check in     +   refund or credit   =  FREE
```

---

## 2. What the data says (prod, 2026-09-17)

| Event | Deposit | In-person registered | Showed up | Show-up rate |
|---|---|---|---|---|
| RTM #1 (Apr 26) | none recorded | 67 | 25 | **37%** |
| RTM #3 (Jun 20) | ฿500 | 16 | 13 | 81% |
| RTM #4 (Jul 19) | ฿500 | 19 | 16 | 84% |
| RTM #5 (Aug 23) | ฿500 | 16 | 15 | 94% |

**Depositors only** (in-person, verified cash or credit, ended events RTM #3–#5):
**43 of 46 showed up (93%).** At RTM #5, 5 people got in on rolling credit and
all 5 came.

Use these numbers, not "98%" (an older claim in the keynote prompt that the data
doesn't support). RTM #2 has no deposit records in D1. Don't cite it until
someone checks whether its deposits were tracked in the sheet only.

Honest caveat for a pitch: #1 vs #3–#5 is a before/after on a growing community,
not a controlled experiment. The drop in registrations (67 → 16–19) is part of
the effect. The deposit filters out people who were never coming.

---

## 3. The rules as built (per rail)

Only **in-person** attendees pay. Online attendees are exempt (`signup.rs` §3e).

### 3.1 THB via PromptPay — *the live rail*

| Step | Who | What happens |
|---|---|---|
| Pay | Attendee | Scans the event PromptPay QR, uploads the slip, gives a bank account for the refund |
| Verify | Organizer | Approves the slip in admin |
| Deadline | System | If no deposit is uploaded within `deposit_deadline_hours` of registering (1 h on RTM #6), the attendee is moved to Online on their next deposit-page visit. They can reclaim a seat by paying, if seats remain |
| After the event | Attendee | Chooses **keep as credit** (card on the ticket once checked in) |
| After the event | Organizer | Or **refunds** by bank transfer and uploads proof (refund queue) |

A deposit ends up **held as credit OR refunded, never both** (D1 compare-and-set).

**No-show:** *the code has no attendance rule for THB.* The refund queue lists
every verified cash deposit, attended or not, and nothing blocks refunding or
holding a no-show's deposit. What a no-show loses is decided by hand, by the
organizer. → decision D1 in §5.

### 3.2 Rolling credit — *THB deposits kept for next time*

- Credit lives in the append-only, org-scoped `credit_ledger` (D1).
- Registering for the next event **applies** it automatically: the deposit is
  covered, no payment step. That needs a Google login with the same email (or a
  wallet already linked to it), in-person, and credit ≥ the deposit.
- **A person can hold several emails** (`person_emails`, plan 025 / issue #122,
  built 2026-09-18 — check it is deployed before relying on it). Emails linked by
  signing in to both with Google share one balance, so credit earned under a
  personal address is spendable when registering with a work address. An email
  nobody linked is its own person and behaves exactly as before. Ledger rows stay
  per email; only balances and the spend guards resolve the linked set.
- **Rule since 2026-09-17 (#118): credit is never forfeited by a no-show.**
  An apply only locks the credit for that one event. It comes back at check-in,
  or when the event ends, whichever is first. It stays the attendee's money until
  the organizer actually pays it back.
- **Withdraw:** the attendee presses "request return" → it shows in the organizer's
  payout queue with the ledger amount → the organizer transfers the cash → clearing
  the request removes the credit. The button is currently only on the ticket of the
  event where the deposit was first held (#120).

### 3.3 USDC via the Solana escrow program — *built and tested, off for every RTM event*

Program `bethere-escrow` (Quasar). One vault per event, one deposit account (PDA)
per attendee.

| Instruction | Signed by | Rule enforced on-chain |
|---|---|---|
| `create_event` | Organizer | Fixes the deposit amount, `event_end`, `refund_deadline` |
| `deposit` | Attendee | USDC moves into the event vault |
| `mark_checked_in` | Organizer (scanner) | Only before `event_end`. **A separate wallet signature, not the QR check-in** |
| `refund` | Attendee | Only after `event_end`. Checked in: no deadline. No-show: only before `refund_deadline` |
| `claim_forfeited` | Organizer | Only after `refund_deadline`, only for not-checked-in deposits |
| `rollover_deposit` | Attendee | **Only if checked in** at the source event. Same organizer, same amount |
| `deactivate_event` / `close_event` / `close_deposit` | Organizer / Attendee | Stop deposits; reclaim rent. Invariant `deposited = refunded + forfeited` |

**No-show:** they can still pull their own refund until `refund_deadline`.
After that the organizer can claim it. Refunds are **never automatic**: the
attendee signs.

### 3.4 Staff, organizers, super-admins

Registration waives the deposit: a ฿0, verified, non-refundable record
(`STAFF_COMP_WAIVED`). Since #119 it also writes the deposit status the pages
route on, so staff land on the ticket, not the deposit page.

### 3.5 Refundable tier (`max_refundable_deposits`)

Deposits past order N are flagged non-refundable at upload. **Nothing enforces
it for THB** (the refund queue doesn't read it), and it's worker-only (not
on-chain) for USDC. RTM #6 sets N = 40 = the seat cap, so it never applies. Any
copy saying "100%" is only true while N ≥ capacity.

---

## 4. The attendee's four outcomes, today

| Outcome | THB cash | Rolling credit | USDC escrow |
|---|---|---|---|
| **Came** | Refund (organizer transfers) or keep as credit | Credit returns at check-in | Pull a refund any time after the event |
| **Didn't come** | **Undefined: organizer's call** | Credit returns at event end (**no loss**) | Pull a refund before the deadline, else organizer claims it |
| **Wants cash, not credit** | Refund queue | Request return (#120) | Refund instead of rollover |
| **Moved to Online** (deadline) | Deposit, if any, stays and needs settling by hand | n/a | n/a |

The row that matters for the pitch is "didn't come". The three rails give three
different answers.

---

## 5. Decisions needed (owner)

**D1. What does a no-show lose?** This is the core of a commitment device, and today it's
inconsistent. Options:
- **A. Nothing, ever.** Deposit = friction only. Consistent with the #118 credit rule and
  the simplest story ("it's always your money"). Risk: the deterrent is only the
  hassle. The 93% in §2 was measured while the landing page told people no-shows
  forfeit, so it doesn't show how option A would perform.
- **B. Cash no-shows forfeit, credit doesn't.** What the landing page and pitch
  imply for first-timers. Needs a THB rule in code (block refund/hold for a no-show
  after a window). Loyal returners are treated gently.
- **C. Everyone forfeits** (cash and credit). The strongest deterrent, matches the
  USDC program, but reverses the 2026-09-17 decision and took money from people once
  already.

Whichever is chosen, write it in the event page's deposit section in one line
("Didn't make it? …"). An attendee should know the downside before paying.

**D2. Is "100%" a promise?** If yes, retire `max_refundable_deposits` (or force N ≥
capacity). If no, the copy has to say "first N".

**D3. Refund timing for THB.** "After the event" has no bound today (RTM #5: 9 cash
deposits still unrefunded 25 days on). Pick a promise: "within 7 days"?

**D4. Fee story.** Docs say 2–5%, the deck says 1–2%, islanddao says none. There's
no fee in code. Pick one for the pitch, or say "no fee today".

**D5. Pitch currency.** The live product is THB/PromptPay. The deck says "$5 USDC".
Plan 018 §5.6b already parks this. Lead with what's live, and present on-chain as the
trustless rail (§6).

---

## 6. Off-chain (today) vs on-chain Solana escrow

### 6.1 Side by side

| | Off-chain: PromptPay + D1 | On-chain: `bethere-escrow` |
|---|---|---|
| **Who holds the money** | The organizer's bank account | A program vault no one person controls |
| **Trust** | Attendee trusts the organizer to refund | Rules are code. The organizer can't take a checked-in attendee's deposit |
| **Proof of refund** | Uploaded transfer screenshot | The transaction itself, publicly verifiable |
| **No-show rule** | Whatever the organizer does (§3.1) | Fixed at `create_event`: refund window, then forfeit |
| **Refund effort** | Organizer transfers each one by hand | Attendee signs one transaction; seconds, a fraction of a cent |
| **Onboarding** | Any Thai banking app; zero crypto | Wallet + USDC + a little SOL. The biggest drop-off risk |
| **Check-in** | QR scan only | QR scan **plus** an organizer wallet signature per attendee, before `event_end` |
| **Credit / rollover** | Flexible ledger: partial, cross-event, no-show-safe, withdrawable | `rollover_deposit`: atomic, but only checked-in, same organizer, same amount |
| **Cancel an event** | Mark all refunded (bulk tool) | No `cancel_event`; each attendee still signs their own refund |
| **Reversibility** | Anything can be fixed by an admin (also: anything can go wrong by an admin) | Mistakes are permanent. Rules can only change by a program upgrade |
| **Audit** | D1 ledger + daily reconcile + Slack alert | Chain state; invariant `deposited = refunded + forfeited` |
| **Cost** | Free (bank transfers), organizer's time | Rent for accounts (reclaimed on close) + tx fees |
| **Where it's live** | Every RTM event | One demo event (islanddao), **on devnet**. Verified 2026-09-17 via prod `/api/health`: escrow `devnet`, badges (Crossmint + Helius RPC) `mainnet-beta`. The split is deliberate (commit `8265d13`, badges-only mainnet launch; no mainnet escrow program yet). Pitch USDC escrow as a devnet demo, badges as mainnet |

### 6.2 What each is better at

**Off-chain wins on** reach (every Thai attendee can pay in 30 seconds),
flexibility (the credit rule changed in a day, data repaired for 2 people), and
cost. It loses on **trust**: the whole promise rests on the organizer refunding.
It also doesn't scale past one organizer's time (RTM #5's refunds are still
pending).

**On-chain wins on** trust and scale. The refund can't be withheld, many
organizers can use it without each being trusted, and no-show money goes where
the rule says, provably. It loses on onboarding, and it's rigid: the #118 rule
("no-show keeps credit") **cannot be expressed** by today's program, because
`rollover_deposit` requires check-in and `claim_forfeited` takes a no-show's deposit.

### 6.3 If we move a rail on-chain, what must change

1. **Pick D1 first.** The program hard-codes option C for USDC. Choosing A or B means a
   program change (e.g. a no-show rollover instruction, or no `claim_forfeited`).
2. **Check-in signing.** Batch `mark_checked_in` per scan window, or a server-held
   check-in authority. Otherwise an organizer who forgets to sign turns attendees
   into "no-shows" on-chain.
3. **Refund UX.** A "claim" button is fine for crypto-native attendees. For others,
   consider a relayer that pays the fee.
4. **Keep PromptPay as the default** and present on-chain as opt-in "trustless mode"
   for organizers who want provable refunds. That's the honest pitch: *familiar
   payment in, provable settlement when trust matters.*

---

## 7. Pitch and copy guidance

- **Lead with the equation:** *Reserve + Show up + Scan + Get it back = FREE.*
  It's true on every rail for anyone who attends.
- **Always pair "FREE" with "deposit first".** "FREE" alone makes people skip the line
  and feel tricked at the payment step. The event page's bar says: "= FREE · Attend
  and the whole deposit comes back, as a refund or as credit for your next event."
- **Don't say** "automatic refund" (never true), "back to wallet" for PromptPay (it's a
  bank transfer or credit), "refunded at check-in" (refunds come after the event),
  "no-shows forfeit" (until D1 is decided), or "98%".
- **Do say:** "43 of 46 depositors showed up (93%), vs 37% before deposits."

---

## 8. Doc map

| File | Status |
|---|---|
| **this file** | Canonical model, comparison, decisions |
| [deposit-refund-flows.md](deposit-refund-flows.md) | Endpoint-level mechanics. Credit section follows §3.2 |
| [HANDOVER-2026-08-15-credit-and-security.md](HANDOVER-2026-08-15-credit-and-security.md) | History. Its "Model B / no-show forfeits credit" is **superseded** by §3.2 |
| [escrow_protocol.md](escrow_protocol.md), [solana_protocol_architecture.md](solana_protocol_architecture.md), [competitive_analysis_kickback.md](competitive_analysis_kickback.md) | Design-era docs. Where they say "refund anytime regardless of check-in" or "no deadline", §3.3 is what the program does |
| `README.md`, `scripts/make_pitch_deck.py`, `.deliverables/*` | Pitch copy. Align with §7 once D1–D5 are decided |
| Landing page copy (`frontend-leptos/src/pages/landing/page.rs`) | Says "auto-refund", "refunded on-chain automatically" (for THB), "no-shows forfeit". Align after D1 |
