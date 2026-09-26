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
- Cut first if W1/W2 slip.

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

Cut order if behind: W3 first, then the W1 backtest (keep live shadow numbers),
then the deck polish. W1 live, W2 and W5 are not cuttable.

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

### 27 Sep: W1 shadow-mode slip agent, on develop (not on staging)

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
- the privacy line on the upload page before vision is switched on;
- the admin-upload path (`slip_admin_upload.rs`) doesn't propose yet;
- staging: apply 0054, then upload a real slip and a doctored one (reused ref)
  in the browser;
- measure worker CPU with vision on (`.issues/134` method);
- the agreement numbers (proposal vs organizer decision).
