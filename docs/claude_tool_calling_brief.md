# Claude Tool-Calling Integration — Briefing & Discovery Questionnaire

> **Status:** Discovery / pre-plan (sits between the discussion phase and implementation phase).
> **Owner:** TBD
> **Created:** 2026-07-09
> **Purpose:** Capture the current BeThere system state and surface every open question, so the implementing team can land on a design that fits the existing architecture with zero loss.

---

## 0. How to use this doc

This is a **context-gathering** document for the "Claude tool calling" feature. It exists because jumping from discussion → implementation without first pinning down the current architecture and the open decisions risks shipping something that breaks auth assumptions, leaks PII, or duplicates existing endpoints.

- **Sections 1–5** describe what BeThere is today. Read these so the integration lands on real architecture, not assumptions.
- **Section 6** lists every open question. **The team must answer every Q-numbered item** before implementation starts. Each question is tagged with the dimension it affects (`[AUTH]` `[DATA]` `[LATENCY]` `[SAFETY]` `[SCOPE]` `[COST]`).
- **Section 7** sketches three integration patterns to choose from — each has trade-offs the questions will resolve.
- **Section 8** is a risk register of things that *would* be lost or broken if the integration is rushed.
- **Section 9** is the definition-of-done checklist for this discovery phase.

---

## 1. What BeThere is

BeThere is a **Solana-native event check-in and registration platform** deployed on Cloudflare Workers. Organizers create events; attendees register, deposit USDC (devnet) into an on-chain escrow, check in via QR, and claim a commemorative NFT. No-shows can refund; organizers claim the deposit after a refund deadline.

### Stack
- **Backend:** Rust compiled to `wasm32-unknown-unknown` via `workers-rs`. Workspace members: `domain` (shared types, pure logic, both `cdylib`+`rlib`) and `worker` (handlers, D1/KV/R2, wasm entry). Excludes `frontend-leptos`.
- **Frontend:** `frontend-leptos` (Leptos 0.8) — separate crate, its own workspace.
- **Database:** Cloudflare D1 (`bethere-db`) — primary SSOT for events, attendees, contacts, registrations.
- **Cache:** Workers KV (`EVENTS` binding) — 60–120s TTL on public reads.
- **Storage:** R2 (`ASSETS_BUCKET`) — posters, badge SVGs, financial slips.
- **Sheets:** Google Sheets — staff allowlist + per-event data sync.
- **Solana:** `bethere-escrow` program on devnet (program id `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`).
- **DOs:** Blocked (Cloudflare API errors 10013/10021) — D1-only path is currently live.
- **flow-harness:** Standalone Rust crate at repo root — E2E regression harness against staging (Plan 005 §3.4).

### Branches (as of 2026-07-09)
- `main` / `develop` — stable.
- `feature/event_recap` — Plan 008 (event lifecycle) + Plan 005 §3.4/§3.5 (flow-harness + preflight gate). 4 commits ahead of `origin`; pushed, ready for review.
- `feature/campaign_create_ux` — campaign UX work.

---

## 2. Current API surface (~123 routes)

Authoritative source: `worker/src/handlers/mod.rs`.

### Public (no auth)
- `GET /api/public/events`, `GET /api/public/events/past` — listing.
- `GET /api/public/event/{slug}`, `GET /api/public/event/{slug}/recap`.
- `GET /api/public/ticket/{id}` — attendee ticket lookup.
- `GET /api/metadata/{event_id}` — NFT metadata.
- `GET /api/deposit/status/{attendee_id}`.
- `GET|POST /api/claim/{token}` — NFT claim flow.
- `GET /api/quiz`, `POST /api/quiz/{token}/submit`, `GET /api/quiz/{token}/status`.
- `POST /api/waitlist`.
- `GET /api/wallet/{address}/nfts`, `GET /api/wallet/leaderboard`.
- `POST /api/escrow/onchain-webhook` — Helius webhook (Bearer auth, separate from attendee auth).

### Attendee-authed (JWT)
- `POST /api/public/register`, `POST /api/public/event/{slug}/register-post-event`.
- `GET /api/my-registration/{slug}`, `GET /api/my-registrations`.
- `POST /api/deposit/usdc`, `GET /api/deposit/usdc/confirm`.
- `POST /api/deposit/thb/upload`, `POST /api/deposit/hold`.
- `POST /api/escrow/refund`, `POST /api/escrow/close-deposit`, `POST /api/escrow/rollover-deposit`.
- `GET /api/contacts/{email}/history` — Plan 008 §3.5 (replaces deprecated `contacts.events_joined` CSV).

### Staff / Admin (JWT + email allowlist)
- `POST /api/events/seed`, `POST /api/events/{id}/restore`, `DELETE /api/events/{id}/delete`.
- `GET /api/events/{id}/summary`, `GET /api/events/{id}/pr-pack`.
- `POST /api/admin/flush-cache`.
- `GET|POST /api/orgs`, `/api/campaigns/...`.
- `POST /api/escrow/close-event`.

---

## 3. Auth model — READ CAREFULLY

This is the single biggest constraint on a Claude tool-calling integration.

### Token
- JWT in `Authorization: Bearer <token>` (or cookie session).
- `Claims { email, sub, iat, exp }` — 24-hour expiry. **No role, no permissions, no scopes** in the token itself.

### Authorization
Authorization is **computed at request time**, not encoded in the token:
1. `is_staff(email)` — checks `STAFF_EMAILS` env var (CSV) + Google Sheets "staff" tab (`worker/src/auth.rs`).
2. `is_organizer(event_id, email)` — per-event organizer check.
3. `is_super_admin` — small static list.

**Implication:** an AI agent cannot simply "have staff permissions" by claiming a role in its token. It must either:
- (a) impersonate an allowlisted staff email (auditable, centralizes blame), or
- (b) be added to the staff allowlist as its own service identity, or
- (c) require a new auth pathway (e.g., a `role = "agent"` claim + handler-level gates).

Plan 006 (SIWS hybrid auth) is **pending** — it will add Solana wallet sign-in alongside Google OAuth. Any agent-auth design must anticipate SIWS landing.

---

## 4. Solana layer

- **Program:** `bethere-escrow` on devnet.
- **PDAs:**
  - `EventEscrow ["escrow", organizer, event_id]` — holds per-event deposits.
  - `AttendeeDeposit ["deposit", event, attendee]` — per-attendee record.
- **Flows:** deposit USDC → verify → check-in → refund (window-gated) / claim NFT / organizer close.
- **Signing:** currently attendee-side (wallet in browser) for deposit/refund; organizer-side for close-event.
- **Reference:** `docs/escrow_contract_surface.md` (23/23 variants mapped; one known divergence #19).

### Tool-calling implication
If Claude is to trigger or simulate on-chain ops, **who signs?** Options:
- Read-only (RPC `getAccountInfo` / simulation only) — safe.
- Service keypair (deposits pre-funded, refunds/closes signed by service) — needs key custody model.
- User wallet (Claude returns an unsigned TX, frontend signs) — preferred for production.

---

## 5. Data sensitivity

- **PII in `contacts`:** email (PK), display name, phone (optional). The `events_joined` CSV column is **deprecated** (Plan 008 §3.5) — read path now JOINs `attendees → events`.
- **Financial docs:** THB slips + refund receipts in R2, gated by identity.
- **Staff data:** Google Sheets is the allowlist SSOT — never expose staff emails in agent responses.
- **Audit data:** `events/{id}/audit` and `/audit/global` return sensitive mutation history.

---

## 6. Open questions — ANSWER EVERY ITEM

### 6.1 Where does Claude live?
- **Q1 `[SCOPE]`** Is Claude a:
  - (a) Frontend chat widget (Leptos component calling Anthropic API from the browser)?
  - (b) Backend orchestration layer (worker handler that proxies to Anthropic)?
  - (c) Standalone service (separate worker / Cloudflare AI Worker)?
- **Q2 `[COST]`** Where does the Anthropic API key live? Browser-side (must be rate-limited / scoped) or server-side (preferred)?
- **Q3 `[SCOPE]`** Is this scoped to staff (admin assistant) or also attendee-facing (registration help, refund status)?

### 6.2 Tool scope
- **Q4 `[SCOPE]`** Which endpoints should be exposed as Claude tools? Suggested starter set:
  - **Read-only:** `GET /api/public/events`, `GET /api/events/{id}/summary`, `GET /api/contacts/{email}/history`, wallet NFT lookup.
  - **Mutation (low-risk):** `POST /api/admin/flush-cache`, `POST /api/waitlist`.
  - **Mutation (high-risk):** `POST /api/events/seed` (draft), event updates.
  - **Dangerous — gate behind explicit confirmation only:** deposit, refund, close-event, hard-delete.
- **Q5 `[SCOPE]`** Should Claude read D1 directly, or only via HTTP endpoints? (Direct = faster; HTTP = reuses auth + caching.)
- **Q6 `[SCOPE]`** Should Claude be able to read on-chain state directly via RPC, or only via the worker's `/deposit/status` abstraction?

### 6.3 Auth
- **Q7 `[AUTH]`** How does Claude authenticate to the worker?
  - (a) Inherit the calling user's JWT (Claude acts *as* the user — simplest, audit trail is per-user).
  - (b) Dedicated service JWT with `role=agent` (needs new middleware path).
  - (c) Per-tool scoping (some tools run as user, some as service).
- **Q8 `[AUTH]`** If Claude impersonates a user, do we enforce that user's full permission boundary (so a non-staff user's agent cannot hit `/admin/*`)?
- **Q9 `[AUTH]`** How does this interact with Plan 006 (SIWS)? Should agents ever hold a Solana key, or always delegate signing to the browser?
- **Q10 `[AUTH]`** Should the agent have a *narrower* permission set than the calling user (defence in depth), or exactly equal?

### 6.4 Data & PII
- **Q11 `[DATA]`** Is Claude allowed to see raw contact emails? Phone numbers? Financial slip URLs?
- **Q12 `[DATA]`** Do we redact PII before sending context to Anthropic, or trust a worker-side proxy to filter?
- **Q13 `[DATA]`** Are there jurisdiction constraints (PDPA Thailand / GDPR) on sending attendee data to Anthropic? If yes, can the agent run on aggregated/anonymized data only?
- **Q14 `[DATA]`** Are staff emails / the staff allowlist ever in scope for agent output? (Default recommendation: never.)

### 6.5 Latency & UX
- **Q15 `[LATENCY]`** Streaming responses (SSE from worker → Leptos) or single-shot JSON?
- **Q16 `[LATENCY]`** What's the acceptable p95 latency for a tool call? Worker CPU limit is 30s on paid plan; sub-invocations compound.
- **Q17 `[LATENCY]`** Cacheable tool results (e.g., event summary) — reuse the existing 60–120s KV TTL, or a separate agent cache namespace?
- **Q18 `[LATENCY]`** Multi-step tool chains (Claude calls a tool, reads result, calls another) — run inside one worker request, or persist conversation state across requests?

### 6.6 Safety & reversibility
- **Q19 `[SAFETY]`** Which tool calls require **human-in-the-loop confirmation** before execution? My recommendation:
  - **Auto-execute:** reads, cache flush, waitlist join.
  - **Confirm-then-execute:** event create/update, contact write, R2 upload.
  - **Never auto-execute:** deposit, refund, close-event, hard-delete.
- **Q20 `[SAFETY]`** How do we surface a Claude hallucination that calls a non-existent route or invents an event id?
- **Q21 `[SAFETY]`** Audit log: every tool call writes `{timestamp, user, tool, args, result_summary, claude_request_id}` to D1? From day one, or retrofitted?
- **Q22 `[SAFETY]`** Reversibility: which agent actions have an "undo" path? (Reads always; mutations sometimes; on-chain never.)

### 6.7 Boundary with Plan 005/006/007
- **Q23 `[SCOPE]`** Should Claude tool-calling land before or after Plan 006 (SIWS)?
- **Q24 `[SCOPE]`** Does Claude need to be exercised by the flow-harness (Plan 005 §3.4) as a regression surface?
- **Q25 `[SCOPE]`** Does this block on staging being live (Plan 005 §3.1), or can the agent be developed against local dev?

---

## 7. Three integration patterns to choose from

### Pattern A — Frontend widget, browser-side Anthropic call
Leptos component hosts the chat; calls Anthropic directly with tools defined inline. Tools are HTTP fetches to `/api/*` with the user's session cookie.
- **Pros:** no worker CPU spent on LLM, simplest auth inheritance.
- **Cons:** API key in browser (must be Anthropic-scoped + rate-limited), no central audit, PII leaves browser uncontrolled.

### Pattern B — Worker proxy (recommended baseline)
New `POST /api/agent/chat` handler. Worker holds `ANTHROPIC_API_KEY` secret; calls Anthropic with tool schemas; executes returned tool calls against internal handlers (in-process, not HTTP).
- **Pros:** server-side key, central audit, PII filtering, reuses auth context, single latency budget, reuses existing KV cache.
- **Cons:** worker 30s CPU limit (need streaming or sub-invocation queue), increases worker bundle size.

### Pattern C — Separate AI worker
Dedicated worker (`bethere-agent`) that talks to the main worker over HTTP with a service JWT.
- **Pros:** isolation, independent scaling, can use heavier models without bloating main bundle.
- **Cons:** extra hop latency, new auth path, another deploy pipeline.

---

## 8. Risk register — what could be lost

| Risk | Impact | Mitigation |
|---|---|---|
| Claude calls `/admin/*` because user JWT was inherited but user is non-staff | Privilege escalation | Q7/Q8/Q10 — enforce per-call `is_staff` check, not just at session creation |
| PII (email/phone) leaks into Anthropic prompt context | Privacy breach, PDPA | Q11/Q12 — server-side proxy with field redaction |
| Hallucinated event id triggers unintended mutation | Data corruption | Q19 — mutations require HITL confirmation + dry-run preview |
| Auto-executed refund drains escrow | Financial loss | Q19 — never auto-execute financial tools |
| Anthropic API key leaks via browser bundle | Key abuse, cost spike | Q2 — server-side only |
| Tool call exceeds 30s worker CPU | Request fails, partial state | Q15/Q16/Q18 — streaming + async tool queue |
| Integration lands before Plan 006 (SIWS) and breaks auth assumptions | Rework | Q23 — pin to land after SIWS, or design auth to be SIWS-compatible |
| No audit trail for agent actions | Untraceable outages | Q21 — D1 audit table from day one |
| Agent reads deprecated `events_joined` CSV instead of `attendees` JOIN | Stale data, confused users | Q5 — agent tools must use the Plan 008 §3.5 read path |
| Agent bypasses KV cache and hammers D1 | D1 read quota, cost | Q17 — agent must reuse `EVENTS` KV TTL |

---

## 9. Definition of done for this discovery phase

- [ ] Every Q1–Q25 has a written answer from the team.
- [ ] Pattern A/B/C chosen, with rationale.
- [ ] Tool inventory finalized (read-only / mutation / dangerous buckets).
- [ ] Auth path chosen and documented as an addendum to `worker/src/auth.rs`.
- [ ] PII redaction policy agreed (which fields Claude may see).
- [ ] Risk register reviewed; every mitigation has an owner.
- [ ] Output: a new `.plans/0XX_claude_tool_calling.md` that captures the chosen design and feeds straight into implementation.

---

## 10. References
- `worker/src/handlers/mod.rs` — full route list.
- `worker/src/auth.rs` — auth middleware, `is_staff`.
- `domain/src/models/auth.rs` — `Claims` struct definition.
- `docs/escrow_contract_surface.md` — Solana API surface.
- `.plans/005_flow_verification_and_staging.md` — flow-harness (regression safety net).
- `.plans/006_siws_hybrid_auth.md` — pending SIWS work.
- `.plans/008_event_lifecycle_summary_pr.md` — recent event-lifecycle work (contact history endpoint, deprecated CSV read path).
- `flow-harness/` — existing E2E harness; pattern reference for any agent regression suite.
- `.handovers/126_plan_005_flow_harness_scaffold.md` — flow-harness design rationale.