# Plan 0XX — Claude Tool-Calling Integration (Design)

> **Status:** Draft design, ready for team ratification.
> **Consumes:** `docs/claude_tool_calling_brief.md` §6 (Q1–Q25), §7 (patterns), §8 (risks).
> **Place in repo as:** `.plans/0XX_claude_tool_calling.md` (renumber to next free id).
> **Prereq:** lands **after** Plan 006 (SIWS). Developable now against local dev + devnet.

---

## 0. Orientation — read this first

**What this is.** A proposed answer to the discovery brief `docs/claude_tool_calling_brief.md`, which listed 25 open questions that must be answered before "let Claude drive BeThere via tool-calling" becomes a buildable plan. This document answers all 25 and turns them into concrete decisions.

**The one-line idea.** Let an LLM operate BeThere's existing API by calling it as *tools* — but make it safe by two guarantees: (1) the model can only ever emit a **valid, well-formed** tool call (right function name + typed args, no invented routes or ids), and (2) every call is **checked against real data and permissions** before it runs. Read-only stuff runs automatically; anything that moves money or deletes data needs a human to confirm, and anything on-chain is signed by the user's own wallet — the agent never holds a key.

**What I want from you (the reviewer).**
- Sanity-check the four decisions in §1 and the tool buckets in §3 — especially which routes I marked *dangerous* vs *low-risk*.
- Fill in the two things I can't decide alone: risk-register **owners** (§6) and the open items in §9.
- Tell me if the auth model in §4 conflicts with anything in the real `auth.rs`.

**How to read it.** §1 is the summary — if you only read one section, read that. §2 is the full Q&A. §3–§5 are the concrete design (tools, auth, architecture). §6–§9 are risks, done-criteria, and phasing.

**Provenance / honesty note.** This is a *draft for discussion*, not a decision. Recommendations are opinionated defaults meant to be argued with, not rubber-stamped. The tool inventory in §3 was built from BeThere's actual route surface; the safety architecture in §5 was prototyped separately (working code exists) but has **not** yet been wired to the real worker.

---

## 1. Decision summary

Four decisions settle most of the questionnaire:

1. **Integration pattern → B (worker proxy).** New `POST /api/agent/chat` handler; worker holds `ANTHROPIC_API_KEY`; worker executes returned tool calls against internal handlers. (§7-B; Pattern A rejected on key-exposure + no-audit; Pattern C deferred until bundle size or scaling forces it.)
2. **Correctness mechanism → constrained tool schemas + live argument validation.** The model is confined to valid tool names and typed arguments *by construction*; every argument is then checked against real D1 / on-chain state before execution. This is the two-layer defense (structural validity, then semantic validity) — see §5. It is the single highest-leverage mitigation in the risk register.
3. **Agent identity → dedicated service JWT (`role=agent`) with a scope strictly narrower than the calling user.** Effective permission = `user_perms ∩ agent_scope`. `is_staff` is recomputed server-side per call (consistent with BeThere's existing email-based computed authorization — no role claims trusted from the token).
4. **On-chain → the agent never holds a Solana key.** It proposes actions by invoking the existing `*_tx_handler` builders, which return **unsigned** transactions; the browser signs via SIWS. BeThere already separates tx-building from signing, so this needs no new primitive.

---

## 2. Answers to Q1–Q25

### 6.1 Where does Claude live?
| Q | Answer |
|---|---|
| **Q1 [SCOPE]** | (b) Backend orchestration layer — Pattern B. |
| **Q2 [COST]** | Server-side only. `ANTHROPIC_API_KEY` is a Worker secret; never in the browser bundle. |
| **Q3 [SCOPE]** | Phase 1 staff-only (admin assistant). Phase 2 attendee-facing **read-only** (refund status, registration help) after redaction + HITL proven. No attendee-facing mutations. |

### 6.2 Tool scope
| Q | Answer |
|---|---|
| **Q4 [SCOPE]** | Bucketed + phased inventory (see §3). Phase 1 = read-only; then low-risk mutations behind confirmation; financial/on-chain = dangerous bucket, never auto. |
| **Q5 [SCOPE]** | Via HTTP endpoints, **not** direct D1 — reuses auth, KV cache, and the Plan 008 §3.5 JOIN read path; avoids deprecated-CSV and cache-bypass risks. |
| **Q6 [SCOPE]** | Via the worker's `/deposit/status` + `/escrow/*` abstractions, **not** raw RPC — one place for caching and RPC-flakiness handling. |

### 6.3 Auth
| Q | Answer |
|---|---|
| **Q7 [AUTH]** | (b) service JWT `role=agent` **and** (c) per-tool scoping. Not raw (a) inheritance. |
| **Q8 [AUTH]** | Yes — calling user's boundary is the ceiling, intersected with the agent's narrower scope; `is_staff` recomputed on the acting user. |
| **Q9 [AUTH]** | Agent never holds a Solana key. Signing always delegated to the browser (SIWS). Agent emits unsigned tx/intent. |
| **Q10 [AUTH]** | Narrower (defence in depth): `agent_scope ⊂ user_scope`. |

### 6.4 Data & PII
| Q | Answer |
|---|---|
| **Q11 [DATA]** | Default **no** to raw emails/phones; slip URLs **never**. Agent uses tokenized refs (`attendee_id`, masked `a***@domain`). |
| **Q12 [DATA]** | Redact **server-side at the proxy** before the Anthropic call; don't trust the model to ignore PII it was given. |
| **Q13 [DATA]** | Yes — PDPA (TH) + GDPR apply (cross-border transfer). Cleanest path: redact so no PII leaves; plus DPA + zero-retention/no-training API settings. |
| **Q14 [DATA]** | Never — staff emails / allowlist out of all agent I/O. |

### 6.5 Latency & UX
| Q | Answer |
|---|---|
| **Q15 [LATENCY]** | Streaming (SSE worker→Leptos). Single-shot only for trivial reads. |
| **Q16 [LATENCY]** | p95 ~5–8s per single-tool turn; hard ceiling 30s CPU. Cap chain depth (≤3 tool calls/turn); longer chains go async. |
| **Q17 [LATENCY]** | ~~Reuse existing 60–120s `EVENTS` KV TTL for read tools~~ — **premise void (2026-09-04, §6.4): the `EVENTS` event-config keys are written with no expiration, and no TTL in the worker is in the 60–120s band.** Read-tool caching must be designed from scratch. Unchanged: a separate namespace only for conversation state, not tool results. |
| **Q18 [LATENCY]** | One request for short chains; persist state (Durable Object / D1) for longer or multi-turn. |

### 6.6 Safety & reversibility
| Q | Answer |
|---|---|
| **Q19 [SAFETY]** | Three tiers: read = auto; low-risk mutation = confirm-then-execute; financial/on-chain = never auto (explicit confirm + dry-run). |
| **Q20 [SAFETY]** | Impossible-by-construction: constrained decoding to the tool schema + argument validation against live state. A hallucinated `event_id` fails an existence check and returns structured "not found" — never a mutation. |
| **Q21 [SAFETY]** | D1 audit table from **day one** (agent id, acting user, tool, redacted args, dry-run vs executed, result, ts). |
| **Q22 [SAFETY]** | Reads always reversible; mutations get undo where feasible (soft-delete/status revert); on-chain never reversible → never auto + always human-signed. |

### 6.7 Boundary with Plans 005/006/007
| Q | Answer |
|---|---|
| **Q23 [SCOPE]** | Land **after** Plan 006 (SIWS) — on-chain story depends on browser-delegated signing. |
| **Q24 [SCOPE]** | Yes — add agent to the flow-harness (Plan 005 §3.4), incl. adversarial cases (hallucinated id, privilege-escalation attempt). |
| **Q25 [SCOPE]** | Develop locally against dev D1 + devnet; gate only the final attendee-facing / on-chain-signing E2E on staging. |

---

## 3. Tool inventory (grounded in the real route surface)

Buckets map to Q4/Q19. Every tool call = an HTTP call to an existing handler with the agent's scoped JWT (Q5). PII columns note redaction (Q11/Q12).

> **Path prefix:** all routes below are shown without the global `/api` prefix for brevity. The worker mounts the entire router under `/api` (`.nest("/api", …)` in `worker/src/handlers/mod.rs`), so the real paths are `GET /api/events`, `POST /api/checkin/{id}`, etc.

### 3.1 Bucket R — Read-only (auto-execute)
| Tool | Route | PII handling |
|---|---|---|
| `list_events` | `GET /events` | none |
| `get_event` | `GET /events/{id}` | none |
| `get_event_summary` | `GET /events/{id}/summary` | aggregates only |
| `get_deposit_status` | `GET /deposit/status/{attendee_id}` | none (status enum + amounts) |
| `get_attendee` | `GET /attendee/{id}` | **redact**: mask email, drop phone/slip URLs |
| `list_attendees` | `GET /attendees` | **redact** per-row; prefer counts |
| `refund_queue` | `GET /refund/queue`, `GET /escrow/refund-queue` | mask attendee identity |
| `escrow_status` | `GET /escrow/cancel-status`, `GET /escrow/health` | none |
| `event_audit` | `GET /events/{id}/audit`, `GET /audit/global` | none |
| `live_dashboard` | `GET /dashboard/live` | aggregates only |
| `campaign_stats` | `GET /campaigns/{id}/stats`, `GET /community/insights` | aggregates only |

### 3.2 Bucket M — Mutation, low-risk (confirm-then-execute)
| Tool | Route | Undo |
|---|---|---|
| `check_in` | `POST /checkin/{id}` | `POST /attendee/{id}/undo-checkin` |
| `undo_check_in` | `POST /attendee/{id}/undo-checkin` | re-checkin |
| `set_participation_type` | `PATCH /attendee/{id}/participation-type` | re-set |
| `generate_qrs` | `POST /generate-qrs` | idempotent |
| `register_walkin` | `POST /walkin/register` | delete attendee (dangerous) |
| `flush_cache` | `POST /admin/flush-cache` | self-healing |

### 3.3 Bucket D — Dangerous (never auto-execute; HITL + dry-run; on-chain = browser-signed)
| Tool | Route | Why dangerous |
|---|---|---|
| `propose_refund` | `POST /escrow/refund` (`*_tx_handler`) | on-chain money → **unsigned tx → browser signs** |
| `propose_close_deposit` | `POST /escrow/close-deposit` | on-chain → browser signs |
| `propose_claim_forfeited` | `POST /escrow/claim-forfeited` | on-chain → browser signs |
| `propose_close_event` | `POST /escrow/close-event` | on-chain → browser signs |
| `mark_refund` | `POST /refund/mark/{id}`, `/refund/manual/{id}` | financial state change |
| `batch_thb_refund` | `POST /refund/batch-thb` | bulk financial |
| `verify_thb_slip` | `POST /deposit/thb/verify` | approves money |
| `delete_attendee` | `DELETE /attendee/{id}` | destructive PII delete |
| `archive_event` / `hard_delete_event` | `DELETE /events/{id}`, `DELETE /events/{id}/delete` | destructive |

### 3.4 Bucket X — Out of scope for the agent (Q14, bulk PII, security controls)
`/contacts*` (bulk contact PII), `/contacts/audience`, `/walkin/export` (CSV PII), `/auth/*`, `/privacy/*` (user-initiated only), staff allowlist, `/orgs` delete. Never exposed as tools.

> **On-chain pattern (Q9):** Bucket-D `propose_*` tools call the existing `*_tx_handler` builders, which return an **unsigned** transaction. The agent surfaces it; the user signs in-wallet (SIWS). The agent has no signing authority — enforced by not giving it a key, not merely by policy.

---

## 4. Auth path (addendum to `worker/src/auth.rs`)

- New principal type `Agent { acting_user, agent_scope }`. Minted as a short-lived service JWT with `role=agent`, always bound to an `acting_user`.
- **Authorization check per tool call:** `allow = is_staff(acting_user) ∧ tool ∈ agent_scope ∧ bucket_policy(tool)`. `is_staff` recomputed from the allowlist each call (never trusted from the token) — matches existing computed-authorization model.
- `agent_scope` defaults to Bucket R only; M/D require explicit grant. Bucket D additionally requires a per-call `confirmed_by_user` token from the HITL step.
- No Solana keypair is ever provisioned to the agent runtime.

---

## 5. Architecture: constrained tool-calling (the correctness core)

Two independent layers; a call must pass both.

1. **Structural validity — constrained decoding.** The model can only emit a valid `tool_name` + typed arguments. Well-formed by construction: no invented routes, no missing/mistyped args. (Prototyped in `katgpt-rs/examples/bethere_tool_calling.rs` — a `ConstraintPruner` grammar over the tool set; and `bethere_neural_decode.rs` / `bethere_latent_steering.rs` showing the same masking over a real neural read-out.)
2. **Semantic validity — argument validation + business rules.** Before execution: `event_id` exists in D1? attendee belongs to event? deposit in a refundable state? bucket policy + HITL satisfied? A hallucinated id fails here and returns a structured error — never a mutation. (Prototyped in `bethere_tool_dispatch.rs` — the `gate()` + backend `Status` checks.)

The step from prototype to production is exactly: swap the toy 11-token grammar for the §3 tool schema, and swap the mock escrow backend for the scoped worker handlers.

> **Prototype location (provenance).** The `katgpt-rs/examples/bethere_*.rs` files cited above live in a **separate prototype repo (`katgpt-rs`), not this BeThere monorepo** — consistent with §0's "prototyped separately." They are illustrative provenance for the constrained-decoding technique, not files a reader will find in this tree. Before P1, port the relevant grammar/dispatch logic into this repo (or vendor it) so the design is self-contained; until then treat these paths as external references.

**Execution flow (Pattern B):**
```
Leptos chat → POST /api/agent/chat (SSE)
  → worker: redact context (Q12) → Anthropic call with §3 tool schemas (constrained)
  → for each returned tool call:
       structural check (schema)         [layer 1]
       semantic check (D1/chain + auth)  [layer 2]
       bucket policy:
         R → execute, stream result
         M → stream confirmation card → on user OK → execute
         D → stream dry-run + (on-chain) unsigned tx → user signs in wallet
       write D1 audit row (Q21)
  → stream assistant summary
```

---

## 6. Risk register (owners assigned 2026-09-04 — see §6.3 *Ownership model*)

Severity is the blast radius **if the mitigation fails**, not the likelihood.
"Ratification must establish" is the concrete question each owner has to answer
yes/no to — without it, "reviewed" means nothing and the checkbox is theatre.

| Risk | Mitigation | Severity | Ratification must establish | Owner |
|---|---|---|---|---|
| Privilege escalation via inherited JWT | §4 `is_staff` recompute + `∩ agent_scope` (Q7/Q8/Q10) | **Critical** | That `agent_scope` is intersected **server-side on every call**, and that a captured staff JWT cannot widen it by replay. Who signs off on the `auth.rs` addendum? | @ozoneRatchapon |
| PII leak into Anthropic context | §3 redaction at proxy (Q11/Q12) | **Critical** | The exact redacted column list, checked against the live schema — prod currently holds 467 `attendees` and 172 `contacts` rows — and that redaction runs at the proxy, not as a prompt instruction. | @ozoneRatchapon |
| Hallucinated id → mutation | §5 layer-2 existence check (Q20) | High | That every Bucket M/D tool resolves ids against D1 before acting, and what a miss does (refuse vs. ask). | @ozoneRatchapon |
| Auto-executed refund drains escrow | Bucket D never auto; browser-signed (Q19) | **Critical** | That no Bucket D route is reachable without a browser signature. Note prod runs `mainnet-beta` today, so a failure here spends real funds. | @ozoneRatchapon |
| API key in bundle | server-side secret only (Q2) | High | That the Anthropic key is a Worker secret and never reaches `frontend-leptos/dist` — verifiable by grepping the built bundle in CI. | @ozoneRatchapon |
| 30s CPU overrun | streaming + chain-depth cap + async queue (Q15/Q16/Q18) | Medium | The chain-depth cap's actual number and what the user sees when it trips. | @ozoneRatchapon |
| Lands before SIWS | pinned after Plan 006 (Q23) | Medium | Plan 006's status at ratification. This is a sequencing gate, not a code change — it either holds or the plan slips. | @ozoneRatchapon |
| No audit trail | D1 audit table day one (Q21) | High | The table schema and retention, and that the row is written **before** the tool result returns, so a crash cannot lose it. | @ozoneRatchapon |
| Reads stale (CSV) | aggregate tools use the Plan 008 JOIN path (Q5); `get_attendee` / `list_attendees` stay Sheet-latency reads and may not ground a mutation | Medium | That no tool reads the denormalized `contacts.events_joined` CSV column, and that the two Sheet-reading Bucket R tools are read-only — never the basis for a Bucket M/D call. | @ozoneRatchapon |
| D1 hammering | ~~reuse `EVENTS` KV TTL (Q17)~~ — no such TTL exists (§6.4); the agent's read caching must be **designed**, not inherited | Medium | The cache layer the agent read tools actually get: a TTL on a new namespace, request-scoped memoization, or neither — and the number, once there is one. Q17's premise is void. | @ozoneRatchapon |

### 6.1 Mitigation review — 2026-08-21, against `develop` @ `446b6d8`

The "reviewed" half of §7's Definition of Done. Each mitigation was checked
against the actual codebase rather than accepted as written. The register was
ten unowned assertions; it is now graded, and **two risks turn out to be already
mitigated**.

| # | Risk | Mitigation status today | Evidence |
|---|---|---|---|
| 1 | Privilege escalation via inherited JWT | **Design only** — the *foundation* holds (`Claims` carries no role; role is recomputed per call), but `agent_scope` does not exist yet | §10 citation table |
| 2 | PII leak into Anthropic context | **Design only** — now concrete: the columns needing redaction are `first_name`, `last_name`, `name`, `email`, `ticket_name`, `phone`, `bank_name`, `account_name`, over 468 live attendee and 172 contact rows | `domain/src/models/attendee.rs`, prod `/api/health` |
| 3 | Hallucinated id → mutation | **Design only, but proven in prod** — `validate_campaign_id` + `campaign_exists` + `GET /campaigns/{id}/exists` shipped 2026-08-20 and is exactly the layer-2 existence check this row proposes | `worker/src/handlers/campaigns.rs:362`, `db/campaigns.rs:164` |
| 4 | Auto-executed refund drains escrow | **Design only** — `/escrow/refund` exists and sits in the authed router; the "never auto" policy is not encoded anywhere yet | `worker/src/handlers/mod.rs:150` |
| 5 | API key in bundle | **Vacuously clean** — no Anthropic key appears in the built frontend JS, but only because the integration does not exist. Re-run this grep as a CI gate once it does | `frontend-leptos/dist/*.js` |
| 6 | 30s CPU overrun | **Design only** — no chain-depth cap exists to inspect | — |
| 7 | Lands before SIWS | ✅ **SATISFIED — gate cleared** | `/auth/wallet/nonce` + `/auth/wallet/verify` are live (`handlers/mod.rs:81-82`); the frontend SIWS flow shipped in `wallet_signin.rs` |
| 8 | No audit trail | ✅ **Already available** — `audit_log` exists with a working insert/query layer and 29 rows in prod. "Day one" is satisfied by construction; the agent only has to call it | `worker/src/db/audit.rs`, prod `/api/health` |
| 9 | Reads stale (CSV) | ✅ **Audited 2026-08-21 — one real defect found and fixed; two tools constrained by design.** See §6.2 | `handlers/contacts.rs`, tool inventory §3.1 |
| 10 | D1 hammering | ~~**Available**~~ → **Design only** (corrected 2026-09-04, §6.4). The `EVENTS` binding is real, but the event-config keys an agent would cache are written with **no** `expiration_ttl`, and no TTL in the worker is in Q17's 60–120s band — there is no cache to inherit | `worker/wrangler.toml:152,248`; `worker/src/event_store/write/` |

**What this changes.** Risk 7 was a sequencing gate and it is now cleared — the
plan is no longer blocked behind Plan 006. Risk 8's mitigation is already built.
Risk 3 has a working precedent shipped in prod, so the layer-2 pattern is no
longer speculative. That leaves six design-only rows plus one (R9) that still
needs a look — a materially smaller ratification than the blank table suggested.

*Corrected 2026-09-04:* R9 has since been audited (§6.2), and R10 was re-graded
from **Available** to **Design only** (§6.4 — the TTL it assumed does not exist).
The standing count is therefore **seven design-only rows**; R7 and R8 remain the
only two whose mitigation is already built.

### 6.2 R9 audit — stale Sheets/CSV read paths (2026-08-21)

**Scope.** R9's ratification question is *"that no **tool** reads the Sheets/CSV
path"* — so the audit covers the routes in the §3 inventory, not every handler.
That distinction matters: `worker/src/handlers/` contains ~150 `sheets::` call
sites, because Google Sheets is the **primary attendee store** in this
architecture, not a legacy path to be excised. Auditing "every handler" would
have proposed rewriting the live check-in flow for 468 mainnet attendees, which
is not what this row asks for.

**Two things the stale-read rule actually covers**, per plan 008 §3.5:
1. reading the master contacts **Google Sheet** for data D1 already holds, and
2. reading the denormalized `contacts.events_joined` **CSV column**, which is
   overwritten on every upsert and drifts.

**Finding 1 — a real defect, now fixed.** `GET /api/contacts/stats`
(`contacts_stats_handler`) did *both* at once: it fetched the contacts Google
Sheet and then split `contact.events_joined` to tally per-event counts. Its
numbers could therefore drift from reality in two independent ways. It is not in
the §3 tool inventory, but it is exactly the defect this row describes.

Fixed in `9388ddf`: it now calls `audience_aggregate(db, None)` — the
`attendees`→`developer_profiles` JOIN that plan 008 §3.5 designates as the source
of truth. The per-event tally still splits a comma-separated string, but that
string is `GROUP_CONCAT(DISTINCT a.event_id)` computed fresh by the JOIN, not the
stored column; the distinction is now pinned by tests. The tally was extracted to
`tally_contacts_per_event` with 5 unit tests (blank/whitespace ids, deterministic
tie-break, empty input, repeat-count source). Verified on staging: `/contacts/stats`
and the already-D1 `/contacts/audience` now return identical figures.

Risk was low — the endpoint has **no frontend consumer**; the admin UI already
uses `/contacts/audience`.

**Finding 2 — two Bucket R tools read Sheets, and that is a design constraint,
not a bug.**

| Tool | Route | Reads |
|---|---|---|
| `get_attendee` | `GET /attendee/{id}` | `sheets::get_attendee_by_id` (`attendee/read.rs:36`) |
| `list_attendees` | `GET /attendees` | `sheets::get_attendees_for_event` (`attendee/list.rs:46`) |

These are the canonical attendee reads for the whole product, not agent-specific
paths. Rewriting them to read D1 would change behaviour for the live admin UI and
check-in flow, and depends on an unanswered question — whether D1 is authoritative
for attendees or a lagging mirror of the Sheet.

**R9's mitigation wording has been corrected in §6 (2026-09-04).** It previously
read *"tools use Plan 008 JOIN path"*, which is not achievable for these two
tools as written. The register row now says: **aggregate tools use the Plan 008
JOIN path (counts via `audience_aggregate` / `dashboard/live`); `get_attendee` /
`list_attendees` stay Sheet-latency reads and may not ground a mutation** —
acceptable for an agent answering questions, not acceptable as the basis for a
Bucket M/D call. The ratification question was narrowed to match: no tool may
read the denormalized `contacts.events_joined` CSV column, and the two
Sheet-reading Bucket R tools must stay read-only. Whoever owns this row still
confirms the wording; only the unachievable claim was removed.

**Owners — resolved 2026-09-04.** This was open from 2026-08-20 (asked twice; the
answer both times was that the owners weren't known). It is now closed the way the
previous revision of this paragraph said it should be: the plan's owner made the
call, and the call is that all ten rows are owned by the sole maintainer. See
**§6.3 *Ownership model*** for the evidence, the limits of a single-owner
register, and the trigger that re-opens it.

### 6.3 Ownership model — single owner, and what that does and does not buy

**Assignment.** All ten rows: **@ozoneRatchapon** (Ratchapon Pockathum).

**Evidence for a single owner.** `git shortlog -sne --all` on 2026-09-04 returns
two identities and one human:

| Identity | Commits |
|---|---|
| `ozoneRatchapon <ratchapon.poc@gmail.com>` | 996 |
| `Ratchapon Pockathum <61614162+ozoneRatchapon@users.noreply.github.com>` | 49 |

The second is the GitHub web/noreply identity of the first. There is no
`CODEOWNERS` file and no other maintainer to route a row to. Splitting ten rows
across one person by pretending to specialise them (auth vs. money vs. data)
would be the same theatre the previous revision refused, wearing a different hat.

**What this assignment buys.** Accountability for *answering*: before ratification,
one named person must give a yes/no to each row's "Ratification must establish"
question, and the answers are attributable. That was the point of the column, and
it is now satisfiable.

**What it does not buy — stated plainly.** Separation of duties. Rows 1, 2 and 4
are **Critical**, and they cover privilege escalation, PII belonging to 468 live
attendees, and a refund path running against `mainnet-beta` with real funds. On
those three, "the owner attests" means the person who wrote the mitigation is also
the person who signs off that it works. That is a real weakness of this register,
not a formality — a single-owner sign-off is *not* equivalent to review, and this
document should not be read as claiming otherwise.

**Compensating control (the reason this is acceptable anyway).** Ownership is
weakest where it rests on attestation, so the register should lean on attestation
as little as possible. Six of the ten rows already have, or can have, a mechanical
answer that does not depend on who is asked — though for one of them (row 10) the
mechanical answer turned out to be *"the thing this row assumes does not exist"*,
which is exactly why the check is worth more than the attestation:

| # | Row | Mechanical check instead of attestation |
|---|---|---|
| 3 | Hallucinated id → mutation | The shipped `validate_campaign_id` (`worker/src/handlers/campaigns.rs:251`) / `campaign_exists` (`worker/src/db/campaigns/crud.rs:101`) precedent — an existence check is testable, not attestable |
| 5 | API key in bundle | The CI bundle grep §6 already names. Must become a **blocking CI step** when the integration lands, not a manual check |
| 7 | Lands before SIWS | Route existence — `/auth/wallet/nonce` + `/auth/wallet/verify` are live; verifiable by request, not by opinion |
| 8 | No audit trail | `audit_log` schema + row count are queryable; "written before the tool result returns" is an integration test |
| 9 | Reads stale (CSV) | Audited in code §6.2, with the tally pinned by 5 unit tests |
| 10 | D1 hammering | Grep the `EVENTS` event-config write path (`worker/src/event_store/write/`) for `expiration_ttl` — readable, not attestable. **Run 2026-09-04: no matches.** See §6.4 *R10 correction* |

Rows **1, 2, 4 and 6** have no mechanical substitute today because the code they
would check does not exist yet. Each must ship with its own test or CI gate in the
same PR that introduces the mitigation — the register's single owner is not a
substitute for that, and P1 should not merge with any of the four resting on
attestation alone.

**Trigger that re-opens this row.** A second regular contributor, an external
security review, or the first Bucket D (money-moving) tool reaching staging —
whichever comes first. At that point the four Critical/attestation-only rows should
be re-assigned to someone who did not write their mitigation.

### 6.4 R10 correction — the `EVENTS` KV TTL Q17 reuses does not exist (2026-09-04)

Found while verifying the citations for §6.3, and recorded here rather than
silently dropped.

**The claim.** Q17 answers the caching question with *"reuse existing 60–120s
`EVENTS` KV TTL for read tools"*, R10's mitigation is *"reuse `EVENTS` KV TTL"*,
and §6.1 row 10 graded that **Available** because the `EVENTS` binding exists in
prod and staging config (`worker/wrangler.toml:152,248`). The binding half is
correct and still verifies. The TTL half does not follow.

**What is actually in the namespace.** `EVENTS` is resolved by `resolve_kv`
(`worker/src/handlers/ext.rs` — `state.events_kv.as_ref().or(state.quiz_kv.as_ref())`)
and holds two unrelated classes of key:

| Key class | Written by | Expiration |
|---|---|---|
| Event config / index / form / deposit config | `worker/src/event_store/write/` (`index.rs:17,33`, `form.rs:25`, `deposit.rs:31,60,92,123`) | **none** — `rg 'expiration_ttl' worker/src/event_store/` returns no matches |
| Claim locks | `worker/src/claim/lock.rs:194` | 300s |
| Claim-lock finalize markers | `claim/lock.rs:300,329` (`CLAIM_LOCK_FINALIZE_TTL_SECS`, `lock.rs:21`) | 90 days |
| Crossmint pending-mint markers | `worker/src/solana.rs:160`, via `lock_kv` = `resolve_kv(state)` | 86 400s |

So the namespace is not TTL-free — but the keys that carry TTLs are short-lived
*operational* markers, and the **event-config keys an agent read tool would
actually cache have no expiration at all**. `EVENTS` is a persistent store for
that data, not a cache.

**And the number does not exist either.** No `expiration_ttl` anywhere in the
worker is in the 60–120s band Q17 quotes. The full set of values is 300s
(`claim/lock.rs`, `KV_LEADERBOARD_TTL` at `handlers/wallet.rs:145`), 3 500s
(`GOOGLE_TOKEN_TTL_SECS`, `sheets/mod.rs:31`), 86 400s (`solana.rs:160`) and 90
days (`claim/lock.rs:21`).

**Consequence for ratification.** R10's owner question — *"The TTL value, and
whether it suits an agent's read pattern"* — has no answer today, because there is
no value to quote and no cache to inherit. Three places have been corrected in
place rather than left to be re-derived: Q17 in §2, the R10 row in the §6 table,
and row 10's grade in §6.1 (**Available** → **Design only**).

**Not fixed here.** Putting a TTL on the `EVENTS` event-config writes would change
caching behaviour for the live event-read path, which is well outside "assign
risk-register owners". Filed as a correction, and left as an input to whoever
designs the agent's read path in P1 — the honest starting point is that agent read
caching is **new work**, not a reused knob.

---

## 7. Definition of Done — status
- [x] Every Q1–Q25 has a written answer (§2).
- [x] Pattern A/B/C chosen with rationale (B — §1).
- [x] Tool inventory finalized in R/M/D/X buckets (§3).
- [x] Auth path documented as an `auth.rs` addendum (§4).
- [x] PII redaction policy stated (§3 columns, Q11/Q12).
- [x] Risk register **reviewed** — every mitigation checked against the codebase
      on 2026-08-21, with severity and a per-row ratification question added
      2026-08-20 (§6, §6.1). Outcome: R7's sequencing gate is cleared, R8 is
      already built, R3 has a shipped precedent, and **R9 was audited 2026-08-21
      (§6.2) — one real stale-read defect found and fixed in `9388ddf`**. Six
      rows remain design-only. R9's mitigation wording was corrected in the
      register on 2026-09-04 (see §6.2) — the unachievable "no tool reads the
      Sheets path" claim is gone; the owner still confirms the replacement.
- [x] Risk register **owners** assigned (§6) — all ten rows to @ozoneRatchapon on
      2026-09-04, on the plan owner's call. Grounded in `git shortlog` (one human
      contributor, no `CODEOWNERS`), not in an invented name. §6.3 *Ownership
      model* records what a
      single-owner register does **not** buy — no separation of duties on the three
      Critical rows — and names the mechanical check that replaces attestation on
      six of ten rows, plus the trigger to re-assign.
- [x] Output: this `.plans/` design doc.

---

## 8. Implementation phases
1. **P1 — Read-only staff assistant.** Pattern B handler; Bucket R only; redaction; audit table; SSE. Local dev + devnet. No signing.
2. **P2 — Low-risk mutations.** Bucket M behind confirmation cards; undo paths wired; flow-harness adversarial cases.
3. **P3 — Dangerous / on-chain proposals.** Bucket D `propose_*` → unsigned tx → SIWS browser signing; dry-run UI. Requires Plan 006 landed + staging.
4. **P4 — Attendee-facing read-only.** Refund-status / registration help with stricter redaction.

---

## 9. Open items for team ratification
- Confirm `agent_scope` default grants per staff role.
- Confirm PDPA/GDPR posture: redact-only vs redact + DPA + zero-retention (recommend all three).
- ~~Assign risk-register owners.~~ Resolved 2026-09-04 — §6.3.
- Decide conversation-state store: Durable Object vs D1 (Q18).
- Confirm whether `create_event` (`POST /events`) is Bucket M or D.

---

## 10. Reviewer's Note (pre-ratification, grounded in code)

> **Citations re-verified 2026-08-21** against `develop` @ `6f32317`. All three
> code references below still land on what this section claims, and the load-
> bearing assertion — that `Claims` carries **no** `role` field, so authorization
> cannot be spoofed via the token — holds unchanged:
>
> | Reference | Status |
> |---|---|
> | `domain/src/models/auth.rs#L6-15` | ✅ `Claims { email, sub, iat, exp }` — no `role` |
> | `worker/src/durable_objects/event_do/sync.rs#L6-10` | ✅ `sync_claim_lock_to_d1`, DO→D1 fire-and-forget |
> | `worker/src/handlers/walkin.rs#L171-179` | ✅ `WalkinRegisterRequest`, incl. `override_capacity` |
> | `UserRole` = `{Staff, Organizer, SuperAdmin}` (`worker/src/auth.rs:492`) | ✅ unchanged |
>
> Checked because line-number citations rot silently, and this repo has now been
> bitten twice by "verified" claims that pointed at lines which had moved or never
> existed (`.plans/016` §2, `.plans/014` P1.1). Re-run this check before
> ratification if the tree has moved on.

> Added during the draft review. These are recommendations to settle three of the open items in §9 by reading the actual handlers/structs — not new scope. Each can be accepted, amended, or rejected at ratification. Code references are repo-relative.

### 10.1 `Claims.role` — add an optional, re-validated hint (settles §4 wording)

The plan's §4 says the agent is "minted as a short-lived service JWT with `role=agent`," but the real `Claims` struct carries **no `role` field at all** — authorization is computed server-side and never trusted from the token:

```domain/src/models/auth.rs#L6-15
pub struct Claims {
    /// Google email of the staff member
    pub email: String,
    /// Subject (Google user ID)
    pub sub: String,
    /// Issued at (Unix timestamp)
    pub iat: u64,
    /// Expiration (Unix timestamp)
    pub exp: u64,
}
```

The `UserRole` enum is `{Staff, Organizer, SuperAdmin}` (no `Attendee`, no `Agent`), and `is_staff` / `resolve_user_role` recompute role from the allowlist each call — exactly the "never trust the token" posture §4 already endorses.

**Recommendation:** add `role: Option<String>` to `Claims` purely as a *hint* that is always re-validated server-side, and have `resolve_user_role` treat `"agent"` as "bind to an `acting_user` and intersect with `agent_scope`." This removes the §4 wording-vs-code inconsistency without introducing a trusted role claim. Alternative: a distinct `AgentClaims` struct — more churn, same safety outcome.

### 10.2 Conversation store — Durable Object, following the existing precedent (settles Q18 / §9)

This is not greenfield. BeThere already has a Durable Object with per-entity SQLite storage plus fire-and-forget D1 replication:

```worker/src/durable_objects/event_do/sync.rs#L6-10
impl EventDurableObject {
    /// Sync a claim lock row from DO SQLite → D1 (fire-and-forget via wait_until).
    pub(super) fn sync_claim_lock_to_d1(&self, event_id: &str, token: &str) {
```

**Recommendation:** implement conversation state as a `ConversationDurableObject` mirroring `EventDurableObject` — per-conversation single-writer semantics plus `sync_*_to_d1` replication for analytics/audit reads. D1-only would lose the single-writer guarantee that matters for multi-turn consistency; the DO+D1 pattern is already proven in-repo (`worker/src/durable_objects/event_do/`).

### 10.3 `register_walkin` — keep in Bucket M; flag the destructive undo (settles §9 / §3.2)

`register_walkin` sits in Bucket M today. Reading the handler: it accepts `override_capacity` (a staff bypass over `enforce_walkin_capacity`), writes KV with a 90-day TTL, and has **no financial path** — no deposit, no refund, no payment (walk-ins are free in-person):

```worker/src/handlers/walkin.rs#L171-179
pub struct WalkinRegisterRequest {
    pub event_id: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    /// Staff override: register walk-in even when in-person capacity is reached.
    #[serde(default)]
    pub override_capacity: bool,
}
```

**Recommendation:** keep `register_walkin` in Bucket M — no money ⇒ not-D. The one wrinkle is the undo: deleting a walk-in attendee is destructive (there is no soft-delete today), so either (a) add a soft-delete to walk-ins, or (b) gate the *undo* behind confirmation. The plan's own note ("delete attendee (dangerous)") already signals this; make it explicit that the **undo**, not the registration, is what needs the care.
