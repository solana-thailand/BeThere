# Plan 033: Win Crypto World's Fair and the AI × Solana Thailand track

**Status:** in progress. W1 code landed on develop 2026-09-27 (session
`event-checkin-d9`), not deployed; see §6. Proposed 2026-09-27 by `event-checkin-2e`.
**Supersedes the schedule of** `.plans/026` (its design rule and compliance
section still stand and are not repeated here).
**Deadlines (read 2026-09-27):**
- Colosseum Crypto World's Fair: submissions close **12 Oct 2026**, Pacific
  time (the page showed "due 16d 13h" on 27 Sep ≈ 13 Oct 07:00 UTC =
  13 Oct 14:00 ICT). Only work done **14 Sep – 12 Oct** is judged.
- Superteam Earn, AI × Solana Track Thailand: $10,000, closes
  **13 Oct 06:59 UTC** (13:59 ICT). Needs the Colosseum submission first, with
  Thailand as the country.
- **Our deadline: submit both by Sun 12 Oct 18:00 ICT.** That leaves a
  ~20-hour buffer and no deploys on the last day.

## 0. Where we stand (checked, not recalled)

| | State on 27 Sep |
|---|---|
| Traction | 16 events, 507 registrations, 256 people, 51 deposits (25,000 THB); turnout 37% → 86% with deposits; 0 forfeited (`solana-thailand-devrel-helper/reports/phase-2/SUBMISSION-GTM.md`) |
| Colosseum listing | Old Frontier text, 6 of 8 claims false. Replacement drafted, not published (`…/COLOSSEUM-DESCRIPTION.md`) |
| Solana | Badges mint on **mainnet** via Crossmint. Escrow `C6HDeZES…` live on **devnet** only; full devnet e2e green on staging (`d67f06fb`). SBPFv0 still deployable: the v3 gate is closed on all clusters. |
| AI | **None shipped.** Slip mini-QR parser exists in `domain` (`d0c0a9c3`); decoding it in the worker was rejected on CPU (`.issues/134`); jsQR is self-hosted in the frontend (`193786f7`). |
| Work inside the window so far | Postponement toolkit (plan 032), credit roll-over, security hardening, D1 scoping. Real, but none of it is AI and little is on-chain. |
| Live event inside the window | **RTM #6, Sun 4 Oct**, FabCafe. |

**The gap to close:** the track judges "Meaningful AI" and "Meaningful Solana
integration". Today we can show neither as built in the window. Everything else
(traction, founder–market fit, honesty) is already our strongest card.

## 1. What wins, mapped to what judges score

| Judged on | Our answer | Built by |
|---|---|---|
| Colosseum: Founder + market fit, Insight, Traction | Organizer built it inside the community it serves; the 37% → 86% turnout pair; nobody's deposit ever kept | Exists. Needs the listing rewrite + videos (W5) |
| Colosseum: Product + execution, Viability | A live event (RTM #6) run on it, inside the window, with footage | W1–W3 + W5 |
| Track: Meaningful AI | **Slip agent**: AI reads the deposit slip and *proposes*; deterministic checks *dispose*. No model output moves money (plan 026 §0) | W1 |
| Track: Meaningful Solana | Deposit → check-in → refund **on chain** (devnet escrow), badge on mainnet; agents pay from their own wallet | W2, W3 |
| Track: Distribution thinking | Measured acquisition per channel (GTM doc), credit roll-over as a retention lever | Exists (W5 packages it) |
| Track: Founder potential (speed) | 15 days → shipped slip agent + MCP + live event; commit history is the proof | All |

### 1a. Amendment 27 Sep: the slip agent is AI, not Solana

Asked by the owner: "how does the slip agent relate to Solana?" It doesn't.
It reads a THB PromptPay slip; nothing touches a chain. On its own it scores
on "Meaningful AI" and zero on "Meaningful Solana". W1 + W2 as planned are
two separate stories stapled together, and a judge will see the seam.

The one place where AI and Solana are the **same** act is W3: an AI agent
finds an event, registers, and pays the deposit into the escrow **from its
own wallet**. The worker already serves that transaction:
`GET /api/deposit/usdc/tx` (`worker/src/handlers/deposit/usdc/handlers/tx.rs`)
is a public Solana Pay Transaction Request that builds the unsigned `deposit`
instruction. So W3 is a thin MCP wrapper over existing code, not new chain work.

**Changes:**
- W3 goes from "should ship, cut first" to **must ship**, scheduled
  Mon 29 Sep – Tue 30 Sep (before RTM #6, devnet only, no prod deploy needed).
- New cut order: W1 vision fallback + backtest first (vision is gated on §4 Q3
  anyway), then deck polish. W1 QR path live, W2, W3 and W5 are not cuttable.
- One story instead of two: **BeThere is an attendance-commitment layer.
  Humans commit in THB (the AI checks their slip), agents commit in USDC on
  Solana (the escrow decides), and everyone who shows up gets a mainnet badge.**
  The slip agent is the on-ramp for the 100% of today's users who don't hold
  crypto; the chain is where the commitment and the proof live.
- Rejected: writing slip verdicts or hashes on chain. That is Solana as
  decoration, which §1 says loses.

**What would lose** (from plan 026 §5, still true): AI as decoration;
a demo that needs narration to excuse a manual step; claiming the escrow holds
real money when it does not; spending the window on UI polish.

## 2. Workstreams

### W1. Slip agent: AI proposes, checks dispose (must ship, the AI story)

Flow for every uploaded slip:
1. **Browser decodes the mini-QR** with the self-hosted jsQR. The `domain` parser
   (wasm) turns it into a bank reference. Costs no worker CPU, so no
   `.issues/134` problem.
2. **No QR, or it fails to parse?** The worker sends the image to Claude
   (`claude-opus-5`, structured output via `output_config.format`, raw HTTP from
   Rust) for `{amount_thb, transferred_at, bank_ref, sender_name,
   receiver_name}`. Worker CPU is only the JSON handling; waiting on the API is
   not CPU time.
3. **Deterministic checker**, no model: amount == the event deposit ·
   bank_ref never seen before (DB constraint) · transfer time inside the
   registration window · receiver matches the event's PromptPay account.
   Sender-name match is advisory.
4. Store a **proposal row**: fields, source (`qr` / `vision`), model id, each
   check's result, verdict `accepted` / `needs_review` / `rejected`.
5. **Shadow mode first:** the organizer still verifies by hand, and the
   proposal shows next to the slip ("AI: 500 THB ✓ · ref new ✓ · in window ✓").
   Auto-accept stays off until the numbers in step 6 justify it, and not before
   the submission.
6. **Measure:** agreement with the organizer's decision on every slip since
   deploy, plus a backtest on historical slips (**only with owner approval**,
   §4 Q3). Publish: accepted / queued / rejected, **false accepts = must be 0**.

Acceptance: on staging, a real slip and a doctored one (wrong amount, reused
ref) produce the right verdicts in the browser. In prod before RTM #6, every
new slip gets a proposal, and nothing changes money.

### W2. On-chain deposit loop on camera (must ship, the Solana story)

- **No program change.** `refund` requires `clock >= event_end` (`.issues/129`),
  so the demo uses a rehearsal event that **ends 10 minutes after check-in**:
  register → USDC deposit (devnet) → scan at the door → `mark_checked_in` →
  event ends → `refund` → the explorer shows it. One take, no narration of a
  manual step.
- Show the mainnet badge mint in the same video (already live).
- Run it **twice** on staging devnet before filming (`scripts/e2e_devnet_test.sh`
  is green; memory `devnet-e2e-run-recipe`).
- Stated honestly in the submission: real deposits are THB today; the USDC
  escrow runs on devnet; the mainnet date is on the roadmap (§4 Q2).
- *Not in the window:* the `checkin_authority` delegation (`.issues/129`) and
  the SBPFv3 migration (`.issues/123`). Roadmap only.

### W3. Agents as users: MCP server (should ship)

- A small MCP server over the **public** API: `find_events`, `event_details`,
  `register` (public registration path), `ticket_status`, and
  `deposit_tx` (returns an **unsigned** devnet escrow deposit transaction).
- The agent signs `deposit_tx` with **its own** devnet wallet. BeThere never
  holds an agent key; the escrow program is still what decides.
- Acceptance: a recorded session where Claude finds a staging event, registers a
  test person, pays the devnet deposit from an agent wallet, and the ticket shows
  it. The human never opens the web UI.
- ~~Cut first if W1/W2 slip.~~ Must ship since §1a (27 Sep): it is the only
  place where AI and Solana are one act.

### W4. RTM #6 on 4 Oct: the live proof (must ship)

- W1 running in shadow mode on prod before the event (deploy **Fri 3 Oct**,
  owner go).
- Film: the door scan, the organizer's slip queue with AI proposals, badges
  claimed. Collect 2–3 short attendee quotes (consent first).
- After the event: turnout vs registrations, proposal agreement numbers,
  badges minted. These figures go into the pitch.

### W5. Submission package (must ship)

| Item | Required by | Source |
|---|---|---|
| Listing text + tagline | Colosseum | `COLOSSEUM-DESCRIPTION.md` draft, updated with W1–W4 |
| Past-work disclosure | Colosseum (misrepresentation = DQ) | plan 026 §3: 5 months running, prior Frontier entry, what was built in the window |
| Go-to-market | Colosseum | `SUBMISSION-GTM.md`, re-run its scripts first |
| 2–3 min presentation video | Colosseum | founder on camera: problem → proof (37 → 86%) → what we built → ask |
| ≤3 min product demo video | Colosseum | W2 take + W1 slip queue + W3 agent clip |
| Pitch deck (Problem · User · Product · AI · Solana · Distribution · Future · Team) | Earn | `solana-thailand-devrel-helper/scripts/slides` |
| Public GitHub repo, README leading with what is deployed | both | this repo (already public) |
| Website + X post | Earn | prod site, or `ibethere.org` if bought in time |
| Architecture + integrations doc | Earn | `docs/architecture.md`, plus a one-page W1/W2 diagram |

## 3. Schedule (ICT)

| Date | Work | Owner step |
|---|---|---|
| Sat 27 Sep | This plan | Register on Colosseum (Thailand) + Earn; answer §4 |
| Sun 28 – Tue 30 Sep | W1: proposal table (migration), client QR path, Claude vision fallback, checker, admin display. Staging. | Add `ANTHROPIC_API_KEY` as a Worker secret (staging + prod) |
| Wed 1 Oct | W1 staging browser verification; W2 devnet rehearsal #1 | — |
| Thu 2 Oct | W2 rehearsal #2; fixes | — |
| Fri 3 Oct | Prod deploy with W1 in shadow mode | **Deploy go**, D1 backup |
| **Sun 4 Oct** | **RTM #6**: film, collect numbers | Run the event |
| Mon 5 – Tue 7 Oct | W3 MCP + agent devnet payment; W1 numbers write-up | — |
| Wed 8 Oct | W2 final demo take (clean) | On camera for the scan |
| Thu 9 Oct | Deck, listing text, disclosure, GTM refresh | Review the texts |
| Fri 10 Oct | Demo video edit; README/docs | Record the 2–3 min presentation |
| Sat 11 Oct | Full dry-run of both forms; fix gaps | Read everything once |
| **Sun 12 Oct** | **Submit Colosseum, then Earn, by 18:00** | Press submit (it's your account) |

Cut order if behind (changed 27 Sep, §1a): the W1 vision fallback and backtest
first (keep live QR-path shadow numbers), then the deck polish. W1 QR path,
W2, W3 and W5 are not cuttable. W3 moves up to Mon 29 – Tue 30 Sep.

## 4. Decisions only the owner can make

1. **Solo or with `lidm`?** One product per person. The old listing names both.
   *Default: solo, as said for this round.*
2. **Mainnet escrow before 12 Oct?** Rent ~0.63 SOL, and it would hold real
   money. *Default: no. Devnet demo, mainnet stated as roadmap.*
3. **Send slip images to the Claude API?** Slips show names and bank accounts
   (PDPA). *Default: the QR path needs no image to leave; vision runs only for
   slips without a readable QR, with a privacy-notice line added on the upload
   page. No historical backtest unless you approve it.*
4. **Budget:** Claude API, estimated at cents per slip (tens of slips a week).
   *Default: yes, with a monthly cap set in the Anthropic console.*
5. **Earn form wording:** it asks about "the Frontier Hackathon". Ask the track
   contact (t.me/moeiman) what to answer this round (plan 026 §3).
6. **Domain:** `ibethere.org` before submission? *Default: nice to have, not
   required; don't migrate OAuth/QR links in the last week.*

## 5. Risks

- **Solo bandwidth over 15 days.** Mitigation: the cut order in §3. No new
  features after 9 Oct.
- **A prod deploy right before a live event.** Mitigation: shadow mode changes no
  money path; staging-verified first; D1 backup; rollback runbook.
- **Overclaiming.** Every number in the submission is re-queried on
  11 Oct and cited to its script. "Devnet" is written wherever it applies.
- **PDPA.** See §4 Q3; slips stay in our R2; the proposal row holds no image.
- **Free-plan limits.** W1 adds I/O wait, not CPU; measure CPU on staging
  (`.issues/134` method) before prod.

## 6. Progress log

### 27 Sep: W1 shadow-mode slip agent, on staging (not prod)

Staging `33a0775e` = git `4ddb6439`, migration 0054 applied and read back.
Verified with the published bank vector as `slip_qr` on two disposable
events: the first upload is `needs_review` (ref new ✓), the same QR on the
second event is `rejected` (ref new ✗, `bank_ref` NULL, `claimed_ref` kept).
Opened the admin Deposits screen in headless Chrome: both lines render (muted
and red), no page errors. Probe rows removed afterwards.

- **Checker** `domain/src/slip_proposal.rs`: four checks (amount, ref new,
  in window, receiver tail), `Pass/Fail/Unknown`, any fail rejects, any unknown
  needs review. A QR-only slip can never be `accepted`: the QR carries no
  amount. Tests: `domain/tests/slip_proposal.rs` (9; mutation-checked).
- **Storage** migration `0054_slip_proposals.sql`: `claimed_ref` (evidence) +
  `bank_ref` UNIQUE (first claimant only), so a reused ref is stored, not
  refused. Rows are deleted with their deposit (90-day retention). The
  migration and the upsert were run against SQLite to confirm the race path's
  `UNIQUE constraint failed: slip_proposals.bank_ref` text.
- **QR path**: the browser decodes the mini-QR (`frontend-leptos/js/slip_qr.js`,
  BarcodeDetector then self-hosted jsQR), keeps it only if the `domain`
  parser accepts it, and sends `slip_qr` with the upload; the worker re-parses
  it (`slip_agent::facts_from_qr`) after the deposit is saved. Errors are
  logged and never fail the upload.
- **Vision fallback** `worker/src/slip_vision.rs`: `claude-opus-5`, structured
  output (`output_config.format` json_schema), `effort: low`,
  `fallbacks: "default"`. Runs in `wait_until` only when there is no usable QR
  AND `ANTHROPIC_API_KEY` is set AND `SLIP_AGENT_VISION=on`. Default off
  until §4 Q3 is answered. Every returned field is re-parsed; hostile or vague
  text becomes unknown. Vision refs are namespaced `vision:` (no bank code),
  so they are compared only with other vision reads.
- **Admin**: the pending-slip list returns `slip_proposals`; each card shows
  "Slip check (read from the slip QR): amount ? · ref new ✓ · … → needs your
  review".
- Guards in `worker/tests/slip_duplicate_guards.rs`: the agent writes no deposit,
  the upload records after saving and can't fail on it, UNIQUE + retention,
  and the vision parser.

**Still owed for W1:**
- ~~the privacy line on the upload page before vision is switched on~~ done
  27 Sep (`b23dc06c`). It shows only while `SLIP_AGENT_VISION` is on (deposit
  status `slip_vision_enabled`), so it is never missing and never false. Not
  yet opened in a browser with vision on. Whether the `/privacy` policy page
  should also name Anthropic as a processor is the owner's call (legal text);
- ~~the admin-upload path (`slip_admin_upload.rs`) doesn't propose yet~~ done
  27 Sep. Both uploads call `slip_agent::propose_after_upload`, and the admin
  form decodes the QR too. With auto-verify on, every admin-recorded slip is a
  free agreement sample. Found along the way: `.issues/154` (unchecking
  Auto-verify still verifies);
- a real bank slip photographed on a phone, uploaded through the attendee
  page (the probe sent `slip_qr` directly; the browser decoder was tested
  separately in headless Chrome on a synthetic slip image);
- measure worker CPU with vision on (`.issues/134` method);
- the agreement numbers (proposal vs organizer decision).

### W3 log, 27 Sep: MCP server built, agent paid a devnet deposit on staging

- New standalone crate `bethere-mcp/` (stdio MCP, hand-rolled JSON-RPC, no
  SDK dependency). It has seven tools: `find_events`, `event_details`,
  `agent_wallet`, `register`, `ticket_status`, `deposit_tx` (unsigned preview)
  and `pay_deposit`. It wraps existing worker routes only; there is no new
  endpoint and no worker deploy.
- Signing and submission reuse `flow-harness` (`chain::submit_signed_by`,
  `context::load_keypair_file`, `validate_live_target`). Nothing is copied.
- Rails: staging + devnet only; an operator spend cap
  (`BETHERE_MAX_DEPOSIT_USDC`, default 10 USDC); `pay_deposit` is idempotent;
  `register` requires `consent_given`.
- Live run: see `bethere-mcp/README.md` §"Verified run". Tx `5uBPpdQc…`
  finalized; the worker verified it (`verified: true`, bound to the PDA by the
  F1 guard); the ticket page renders "Deposit verified".
- **Acceptance still owed:** the *recorded* session with Claude as the MCP
  client (`claude mcp add bethere …`, README). The run above drove the same
  binary over stdio from a script.
- Fixture caveat: until an event has at least one D1 attendee, registration
  reads Sheets for its duplicate check and 500s when `sheet_id` is
  unreachable. Real events have a real sheet; the fixture needed a seeded
  attendee row.


### W3 log, 27 Sep (later): Claude as the MCP client

- A headless Claude Code session was the MCP client (`claude -p --mcp-config`
  with a throwaway config). From a plain-language task it found the event,
  registered, paid the 1 USDC devnet deposit and confirmed `verified`.
  Tx `jwmzATe6…` is finalized. It took 9 turns and cost $0.60. The full
  record is in `bethere-mcp/README.md` §"Verified run with Claude as the MCP
  client".
- `demo_fixture` now also activates the event and seeds a host walk-in.
  The fixture is a single command.
- Found and fixed: the first `register` returned a 400 (missing contact
  channel), and the agent then made up a handle. The tool descriptions now
  point at `require_contact_info` and say to ask for the handle.
- **Still owed for W3 submission:** a screen *video* of an interactive run for
  the submission package (W5). That is the owner's recording; the headless
  transcript proves the flow works.
