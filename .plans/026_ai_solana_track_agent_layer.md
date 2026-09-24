# Plan 026 — AI × Solana track: where AI earns its place, and what to build by 12 October

**Status:** proposed, not started · **Written:** 2026-09-18 by the DevRel agent, for this repo's owner agent
**Window that counts:** 14 September – 12 October 2026 (Colosseum judges only work done inside it)
**Submissions:** Colosseum (Crypto World's Fair, closes 12 Oct) **and** Superteam Earn AI × Solana track (closes 13 Oct 06:59 UTC)

---

## 0. The design rule this plan exists to enforce

> **AI proposes. The program disposes.**

The owner's objection killed the first draft of this plan, and it was right:

> *"เราจะใช้ AI ในการ refund ทำไม ในเมื่อเรามี escrow program ที่ deterministic ตามเงื่อนไขแล้ว ถ้า agent คำนวณพลาดจะทำยังไง"*

Correct. **A refund is not a judgement call.** `mark_checked_in` happened or it did
not; `refund` releases or it does not. Putting a model in that path adds a way to be
wrong to a path that currently cannot be wrong, and the whole pitch of the escrow is
that nobody — not the organiser, not a model — can move the money by opinion.

So this plan puts AI **only** where the determinism has already run out: turning
messy real-world input into a claim that a deterministic rule can then accept or
reject. Three consequences, and they are the acceptance criteria for every task
below:

1. **No AI output moves money.** Every transfer is an on-chain instruction whose
   preconditions are checkable without a model.
2. **Every AI output is a proposal with a verdict.** It is stored with the evidence
   it was derived from, the deterministic check that ran, and pass/fail. A wrong
   proposal fails the check and nothing happens.
3. **Where no deterministic check exists** (a forecast), the output is advisory,
   labelled as such, and its error is measured after the fact against what actually
   happened.

---

## 1. Where the determinism actually runs out

### 1a. The Thai-baht boundary — this is the real problem, and the real AI case

The escrow can only enforce rules over money it holds. Today's 500-baht deposit is a
**bank transfer / PromptPay slip**, and somebody has to decide: *does this image mean
this person paid 500 baht?* That decision is human, unforced, and it is exactly where
the 22 rows worth 10,500 THB in no recorded state come from — see the RTM #6 deck.

The mess is real: blurry screenshots, a transfer from a parent's account, the wrong
amount, the same slip sent twice, a name that does not match the registration.

**What AI does here:** read the slip (vision) and propose a claim —
`{attendee, amount, timestamp, bank reference}`.

> **Correction, 2026-09-22 (`.issues/129`).** Vision is the *fallback*, not the
> first move. A Thai bank slip carries a mini-QR whose payload is a bank
> transaction reference, and services that resolve that reference against the
> bank network already exist (RDCW Slip Verify, EasySlip). So the first pass is
> deterministic: decode the QR, require the reference to be unique **in the
> schema**, ask the bank. A screenshot assembled in an image editor fails at
> step one with no model involved. OCR/vision earns its place only on slips
> whose QR will not decode — which is exactly the boundary this plan says AI
> belongs on, one step further out than §1a originally drew it.

**What the deterministic checker does, with no model involved:** amount equals the
required deposit · the bank reference has never been used before · the timestamp is
inside the registration window · the account name matches the attendee or a linked
identity. Only a claim that passes all four becomes a deposit.

A wrong read fails a check and lands in a review queue. It cannot create money.

### 1b. One human, many registrations

`person_emails` shipped on 2026-09-18 (`771daec`). Deciding that two rows are the
same human is fuzzy — nicknames, a second address, a walk-in row typed at the door.
AI can propose the link; the on-chain rule is unaffected, because one badge per PDA
is enforced by the program whatever the model believes.

### 1c. Forecast and ops — advisory only

How many will actually turn up, so how much food to order, how many seats to release.
There is a labelled dataset: 12 events, 477+ attendee rows, real turnout, deposit vs
no deposit. No money moves on the answer. Success is measured, not asserted: predicted
headcount vs actual, per event, printed after each one.

### 1d. Agents as users, not as organisers

The track's own words: *"Agents can discover information, use applications, hold
assets, make payments, and take action on behalf of users."* An MCP server over the
existing API lets someone's assistant find an event, register, place a deposit and
claim a badge. Here AI is the **interface**, and Solana is what makes it possible for
software to hold and move value at all. The escrow still decides; the worst a broken
agent can do is spend its own fee.

---

## 2. What to build in the window

Four workstreams, ordered. **A is the one that must ship**; D is the stretch.

### A. The escrow becomes the money, not a design (must ship)

> **BLOCKER, found 2026-09-19 — read `.issues/123` before planning this week.**
> The escrow builds to **SBPFv0** (`e_flags = 0`, verified independently on
> `target/deploy/bethere_escrow.so`), and SIMD-0500 makes the loader reject any
> new deployment, upgrade or finalization that is not v3. It had not activated on
> mainnet as of 2026-09-17, so a mainnet deploy *today* still lands — but the
> toolchain here is `platform-tools v1.52` / `cargo-build-sbf 3.1.10`, and the
> migration needs v1.56 / v4.2.0. **Do not pass `--arch v3` to v1.52**: Anza says
> it emits incompatible bytecode.
>
> This reorders the week. Either (a) do the toolchain migration first and deploy
> v3 — cleanest, but it means bumping the pinned `quasar` rev, which is a
> deliberate act and unverified for v3 output; or (b) deploy v0 to devnet now,
> demo from there, and state the migration as known work in the submission.
> Deciding this is step one of workstream A, not a detail inside it.

- Deploy `bethere-escrow` to **devnet** first, then **mainnet** once a full cycle has
  run twice on devnet without manual intervention. Mainnet rent is ~0.63 SOL
  (recorded in `docs/mainnet_deployment_checklist.md`).
- Wire the existing check-in scanner: a successful scan submits `mark_checked_in`,
  and `refund` releases the deposit. The door flow must not get slower — sign and
  submit asynchronously, show the attendee the same instant confirmation, and
  reconcile signatures in the background.
- `rollover_deposit` on the "cannot come" path. This is the instruction those 22 rows
  needed.

> **Two constraints found in the program on 2026-09-22 (`.issues/129`), both of
> which change what §A can promise on camera:**
>
> 1. **`refund` requires `clock >= event_end`** (`instructions/refund.rs:74`).
>    The one-take demo below — *scan a QR, the deposit is back in the attendee's
>    wallet* — **cannot happen at the door** with the deployed program. Either
>    the demo is "scan at the door, refund after the event ends, both on chain,"
>    or `refund` gains a checked-in fast path. Decide before filming, not during.
> 2. **`mark_checked_in` demands the organiser's own signature**
>    (`instructions/mark_checked_in.rs:13,15`, `has_one(organizer)`). There is
>    no delegated door role, so scanning without the owner present means handing
>    someone the key that can also `claim_forfeited` and `close_event`.
>    `EventEscrow` has 36 bytes of `_padding` and a `version` byte
>    (`state.rs:20,48`): a 32-byte `checkin_authority` fits with no resize.
>
> Also: the escrow **is** live on devnet (`C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`,
> `Executable: true`), absent on mainnet, and the harness wallet holds 9.99998
> devnet USDC — `.issues/084`'s USDC blocker was stale and has been corrected.
- **Acceptance:** at RTM #6 or on a rehearsal event, one take on camera: scan a QR,
  the transaction lands, the deposit is back in the attendee's wallet, and a block
  explorer shows it. No hidden manual step.

### B. The slip pipeline — AI proposes, checks dispose (must ship)

- `POST /api/deposit/slip` takes an image, returns a **proposal** row, never a
  deposit.
- Proposal carries: extracted fields, model + version, the raw evidence reference, the
  four check results, and a verdict of `accepted` / `rejected` / `needs_review`.
- Accepted proposals create the deposit record that the on-chain path then mirrors;
  rejected ones are visible to the organiser with the reason.
- Bank reference uniqueness is a database constraint, not a check in code — a
  duplicate slip must be impossible, not merely unlikely.
- **Acceptance:** feed it the historical slips we already hold. Report precision and
  recall, and the count that landed in `needs_review`. Publishing "94% accepted, 6%
  queued, 0 false deposits" is a stronger claim than any demo.
- **Do not** let a model decide a refund, a rollover, or a badge. If a reviewer reads
  this plan and finds a code path where a model output reaches a transfer, that is a
  bug and this plan is the ticket.

### C. Agents as users — MCP + x402 (should ship)

- MCP server exposing: `find_events`, `register`, `deposit_status`, `claim_badge`.
  Read paths are public; anything that spends requires the agent's own signature.
- One x402-priced endpoint so an agent can pay for something without an account —
  badge art generation is the natural candidate, since it costs real compute.
- **Acceptance:** a recorded session where an assistant registers a test user, pays a
  deposit from an agent wallet, and the human never touches the web UI.

### D. Forecast + ops assistant (stretch, advisory only)

- Predict per-event turnout from registration history; output a food count and a
  suggested overbooking number, both labelled advisory.
- Backtest against all 12 past events and publish the error. An honest ±N is worth
  more to judges than a claim of accuracy.
- **Acceptance:** the backtest table exists in the repo and the number is quoted in
  the submission, whatever it says.

---

## 3. Compliance — read this before writing the submission

From `colosseum.com/hackathon` FAQ, read 2026-09-18:

- *"Teams may begin development before the hackathon, but products are judged only on
  the work completed between the competition's start and end dates."*
- *"Builders may use pre-existing code, but teams must disclose all relevant past
  development work in the submission form."* Misrepresentation → disqualification, ban,
  prize revocation.
- **One product submission per team, and therefore per individual, per hackathon.**
  The Earn AI × Solana track is not a second entry; it is the same product submitted
  to a track, and it requires the Colosseum submission to exist first, with Thailand
  selected as the country.

**Disclose, specifically:** BeThere already ran six events; the repo has 1,312 commits
since 21 April 2026; and it was submitted to a previous Colosseum hackathon
(**Frontier**, Consumer Apps track, team `ozone` + `lidm` —
colosseum.com/arena/projects/bethere). The honest frame is the strong one: *the base
is a system that has been running for five months and is fully disclosed; what was
built in this window is the agent layer and moving the money on-chain.*

**One correction to carry over.** The existing Colosseum project description says
users "lock a refundable USDC deposit", funds "return instantly" on check-in, and
"no-shows forfeit to the organizer". None of that is true of the system as it runs
today: deposits are Thai baht off-chain, refunds are manual, and **no attendee's
deposit has ever been taken**. Judges may compare the two submissions. Rewrite the
description to match what is deployed at submission time; if workstream A lands, most
of it becomes true and the rest should be stated as roadmap.

**Superteam Earn track** (`superteam.fun/earn/listing/ai-solana-track`): $10,000 USDG
— 3,000 / 2,000 / 1,500 / 1,000 / 500 plus 8 bonus spots of 250. Needs project name,
description, GitHub, website, X link, pitch deck or video, and the Colosseum project
link. Its form asks *"Did you submit this project to the official Frontier Hackathon
on Colosseum?"* — that names last season's hackathon, so **ask the track contact
(t.me/moeiman) what it should say for this round** before submitting.

---

## 4. Deliverables that are not code

Judges see these first, and Colosseum says so explicitly.

- **2–3 minute presentation video** — the single highest-leverage artefact.
- **Product demo video, ≤3 minutes** — the one-take scan-to-refund from §A.
- **Pitch deck** answering the track's eight questions: Problem · User · Product · AI ·
  Solana · Distribution · Future · Team.
- **Website** and an **X post**, both required fields on Earn.
- **Documentation** of product, architecture and integrations — this repo already has
  it; make sure the README leads with what is deployed.

The poster and video pipeline in `../solana-thailand-devrel-helper/scripts/poster-v4`
renders locally and is already tuned to this brand; use it rather than starting from
a template.

---

## 5. What would make this lose

- **AI as decoration.** A background image generator in a track that asks for
  *meaningful AI* reads as a Solana project wearing a hat. §1a is the answer: the AI
  sits on the one boundary where the system currently loses money's state.
- **A demo that needs a story.** If the scan-to-refund take needs narration to explain
  why a step was manual, workstream A is not finished.
- **Claiming the escrow is live before it is.** The deck for RTM #6 is built on being
  the project that says which rows do not pass yet; a submission that overclaims
  contradicts a talk given two weeks earlier, in public, to the same community.
- **Spending the window on the interface.** C and D are worth points; A and B are the
  product.

---

## 6. Open questions for the owner

1. **Solo or with `lidm`?** The owner said solo for this round; the Frontier entry was
   a two-person team and the old project page still lists both. One product per person
   means `lidm` cannot enter separately with the same product.
2. **Do THB deposits stay?** §1a assumes they do and treats the slip as the boundary.
   The alternative — deposits in USDC only — makes the demo cleaner and the product
   smaller. Both are defensible; the plan does not choose.
3. **Mainnet date.** The owner's rule: devnet first, mainnet only when certain. Put a
   date on it, because "when certain" and 12 October can disagree.
