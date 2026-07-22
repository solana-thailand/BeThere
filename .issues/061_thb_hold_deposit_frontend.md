# Issue #061: THB Hold-Deposit Frontend (continues #032)

> Wire the existing THB hold-deposit / rolling-credit backend to attendee + admin UI.
> Continuation of Issue #032 (Rolling Deposit Credit) — backend already shipped, frontend missing.

---

## 1. Summary

Allow attendees with a **THB** (PromptPay cash) deposit to **hold** their deposit as rolling credit
instead of claiming a refund. The held credit auto-covers their next event registration.
Status visible on both attendee (ticket page) and admin (contacts) sides.

**This is a frontend-integration task.** The backend is fully built and deployed on `develop`.
No Solana program changes, no mainnet cluster implications, no worker handler changes.

---

## 2. Why now

- THB cash friction is the #1 reason repeat-attendee retention is awkward today.
- Decoupled from the Solana mainnet cutover (THB is off-chain).
- Low risk: wires existing validated endpoints to UI.
- Backend already enforces ownership (`VULN-012`) + requires verified deposit.

---

## 3. Current backend state (already shipped — do NOT rebuild)

| Piece | File | Status |
|---|---|---|
| `DepositMethod::CreditThb` / `CreditUsdc` enum variants | `domain/src/models/deposit.rs` | ✅ shipped + tested |
| Contacts columns K–M (`deposit_credit_thb/usdc/since`) | `worker/src/sheets/contacts.rs` + `worker/src/db/contacts.rs` | ✅ shipped (Sheets + D1) |
| `POST /api/deposit/hold` — `hold_deposit_handler` | `worker/src/handlers/deposit/thb/handlers/hold_credit.rs` | ✅ shipped (validates ownership, requires verified deposit, increments credit) |
| `GET /api/deposit/credit-balance` — `credit_balance_handler` | same file | ✅ shipped (returns `{credit_thb, credit_usdc}`) |
| `increment_credit` + `get_credit_balance` helpers | `worker/src/sheets/contacts.rs` | ✅ shipped |
| Registration credit check (auto-skips deposit if covered) | `worker/src/handlers/register.rs:319` | ✅ shipped |
| `RolloverActionCard` (USDC on-chain rollover — the template) | `frontend-leptos/src/pages/ticket/action_cards.rs:267-537` | ✅ shipped |

### Backend wire shapes (for the frontend types)

`POST /api/deposit/hold` body / response:
```json
// request
{ "event_id": "...", "attendee_id": "..." }
// response
{ "credit_thb": 500, "credit_usdc": 0, "message": "..." }
```

`GET /api/deposit/credit-balance` response:
```json
{ "credit_thb": 500, "credit_usdc": 0 }
```

---

## 4. Design decisions (resolved — do not re-litigate)

### D1 — Button placement (attendee ticket page)
After check-in, the deposit action area shows:
- **Primary (safe default):** the existing refund flow (USDC on-chain) / manual refund (THB)
- **Secondary:** "Hold Deposit for Next Event" → opens a **confirm step** (not a modal) explaining
  the commitment: *"We'll keep your {amount} THB and auto-apply it to your next event registration."*

Hold gets a confirm step because cash stays with the organizer. Refund stays the no-friction default.

### D2 — Admin view scope
**Credit column on the existing contacts table + a summary header chip.** No separate ledger page.
- Column: `credit_thb` + `credit_usdc` per contact (data already in columns K–M)
- **Header chip:** "Total credit held: X THB across N attendees" — the organizer's cash liability number
- Defer the full transaction-history ledger until real usage data exists.

### D3 — Refund-from-credit (exit path)
**Lightweight "Request Refund of Held Credit" action, NOT payout automation.**
- Attendee clicks "Request Return" on the ticket page → sets a flag on the contact
- Shows in admin as a "credit refund requested" badge
- Organizer processes actual payout through the existing THB refund queue tooling
- Why: attendees need an exit or "hold forever" feels like a trap (Issue #032 trust risk), but
  automated payout = cash-on-hand liability + queue complexity not needed for v1.

### D4 — Method routing
- **THB deposit** → `HoldDepositCard` (off-chain credit, this issue)
- **USDC deposit** → existing `RolloverActionCard` (on-chain atomic transfer, Issue #032 Option B)
- The two are mutually exclusive based on `deposit.method`.

---

## 5. Implementation tasks

### Phase 1 — Attendee side (core)

**Backend wire shapes already match — just add frontend types + calls.**

- [ ] `frontend-leptos/src/api/deposit.rs` — add:
  - `HoldDepositRequest { event_id, attendee_id }`
  - `HoldDepositResponse { credit_thb: u64, credit_usdc: u64, message: String }`
  - `CreditBalanceResponse { credit_thb: u64, credit_usdc: u64 }`
  - `hold_deposit(body)` → `api_post_json("/deposit/hold", body)`
  - `get_credit_balance()` → `api_get("/deposit/credit-balance")`
- [ ] `frontend-leptos/src/pages/ticket/action_cards.rs` — add `HoldDepositCard` component:
  - State machine: `Ready` → `Confirm` → `Holding` → `Confirmed` → `Error`
  - Props: `event_id`, `attendee_id`, `deposit_amount_thb` (for confirm copy)
  - On confirm: `api::hold_deposit(...)`; on success show new balance + success state
  - Model after `RolloverActionCard` but **simpler** (no wallet flow, no Solana TX)
- [ ] `frontend-leptos/src/pages/ticket/in_person_view.rs` — insert `<HoldDepositCard />` in the
      deposit-action branch when: `dep.verified && !dep.refunded && is_checked_in && method == Thb`
- [ ] (Optional polish) Credit chip on ticket page that fetches `get_credit_balance()` on mount
      when `is_checked_in` — shows "Deposit Credit: 500 THB" if balance > 0.

### Phase 2 — Admin side (status visibility)

- [ ] Admin contacts view — add `credit_thb` / `credit_usdc` columns (data already in D1/Sheets K–M)
- [ ] Liability header chip — "Total credit held: X THB across N attendees" (sum query)
- [ ] Badge for "credit refund requested" attendees (Phase 3 dependency)

### Phase 3 — Exit path (lightweight)

- [ ] Backend: `credit_refund_requested` flag on contact (one column or KV flag)
- [ ] Attendee: "Request Return" button on ticket page (sets flag, no payout)
- [ ] Admin: badge/queue for "credit refund requested" → processed via existing refund tooling

---

## 6. Explicitly deferred (scope guardrails)

- Full credit ledger / transaction-history page
- Automated payout from credit (manual via existing tools for v1)
- Currency conversion for THB↔USDC mismatch (keep balances separate — already the design)
- Multi-organizer credit isolation (Issue #029 — single-org for now)
- On-chain vault per attendee (Issue #032 Option C — future, 3+ organizers)

---

## 7. Placement reference (verified against `develop`)

The deposit action-card state machine lives in `frontend-leptos/src/pages/ticket/in_person_view.rs`
around L236-L290. Current branch order:

1. `deposit_info` exists:
   - if `verified` → `DepositVerifiedCard`
   - else → `DepositPendingCard`
   - if USDC + `rollover_target_event` + checked-in → `RolloverActionCard`
   - if `refunded` → `RefundCard`
2. else if deposit_enabled + deadline_expired + reclaim available → `ReclaimActionCard`
3. ...

**Insert `HoldDepositCard`** in branch 1, alongside the rollover/refund logic, gated on:
`dep.verified && !dep.refunded && is_checked_in && dep.method == DepositMethod::Thb`.

---

## 8. Risks / open questions

0. ~~**Backend double-credit gap** — `hold_deposit_handler` incremented credit without settling the
   source deposit, allowing re-calls to double-increment.~~ **RESOLVED** — added a distinct
   `held_as_credit` flag to `ThbDeposit` (migration `0022`), the handler now settles the deposit
   *before* incrementing credit and guards against re-hold (`refunded || held_as_credit`); the
   USDC arm is rejected (defense-in-depth — USDC uses the atomic on-chain rollover). Frontend
   surfaces `held_as_credit` so the card mounts in `AlreadyHeld` on reload.
1. **Organizer liability** — for THB, holding means organizer keeps physical cash until the
   attendee spends it on a future event. Fine for own events; needs a cap/timeout if multi-organizer
   ever happens (Issue #029).
2. **No exit path until Phase 3** — without D3, "hold forever" feels like a trap. Phase 3 should
   land in the same release window as Phase 1/2 even if minimal.
3. **USDC `RolloverActionCard` only checks `rollover_target_event` presence** — if backend ever
   returns a target for THB attendees, the USDC card would wrongly render. Confirm backend
   only sets `rollover_target_event` for USDC deposits.

---

## 9. Branch / commit plan

```
develop/feature/061_thb_hold_frontend  (gitflow, branches off develop)
  ├── commit 1: feat(api): add hold_deposit + credit_balance types/calls
  ├── commit 2: feat(ticket): HoldDepositCard component
  ├── commit 3: feat(ticket): wire HoldDepositCard into in_person_view
  ├── commit 4: feat(admin): credit columns + liability header
  └── commit 5: feat(credit): "Request Return" flag flow (Phase 3)
```

Each commit: `cargo check --target wasm32-unknown-unknown` clean. After Phase 1: `bash build.sh`
+ verify on `:8787` (Cmd+Shift+R) before push.

---

## 10. References

- **Issue #032** — Rolling Deposit Credit (the parent design + backend implementation)
- **Handover #077** — Rollover Deposit E2E (USDC on-chain implementation, the template)
- **`worker/src/handlers/deposit/thb/handlers/hold_credit.rs`** — the live backend endpoint
- **`frontend-leptos/src/pages/ticket/action_cards.rs:267-537`** — `RolloverActionCard` template
- **`frontend-leptos/src/pages/ticket/in_person_view.rs:236-290`** — placement target