# Escrow Contract Surface Inventory

> **Purpose**: Map every on-chain escrow error variant → the instruction that raises it → the
> worker endpoint that can surface it → how the frontend currently handles it → gap status.
> This is the "contract divergence audit" deliverable from plan 005 §3.2, and the baseline that
> plan 006 (SIWS) / plan 007 (mobile) must not regress.
>
> **Provenance**: Compiled from `bethere-escrow/src/**`, `worker/src/handlers/**`,
> `domain/src/models/**`, and `frontend-leptos/src/pages/deposit/**` on
> `feature/005_flow_verification_and_staging` (2026-06-17).
>
> **Companion docs**: `docs/escrow_protocol.md` (design / "what & why"), `docs/security_audit.md`.
> This doc is the **audit / diff** layer: where the client's model of the program diverges from
> the program's actual behavior.

---

## 1. Executive Summary

**One confirmed, user-facing divergence** was found, exactly matching the suspect flagged in
plan 005 §1:

> **`RefundDeadlinePassed` (#19) is not pre-gated by the frontend.** No-show attendees past
> `refund_deadline` see an active "Request Refund" CTA, click it, and only then discover the
> failure when the on-chain transaction reverts. The on-chain program is correct and final —
> this is purely a UX gap, not a funds-at-risk bug.

Root cause, in one line:

```event-checkin/frontend-leptos/src/pages/deposit/types.rs#L196-198
pub fn event_refund_window_open(event_end_ms: i64) -> bool {
    event_end_ms > 0 && now_ms() >= event_end_ms
}
```

The frontend's refund gate checks **`event_end` only**. The on-chain refund validator
(`refund.rs#L77-85`) has a **two-path** model:

- **Checked-in attendee** → refund window `[event_end, ∞)` — no deadline. ✅ frontend gate is
  correct for this path.
- **No-show attendee** → refund window `[event_end, refund_deadline)` — bounded. ❌ frontend
  gate does not enforce the upper bound, so no-shows past `refund_deadline` see a dead CTA.

Two structural reasons the frontend *cannot* currently do the right thing even if the gate
function were fixed:

1. **`DepositStatusResponse` has no `checked_in` field** (`domain/src/models/deposit.rs#L151-215`).
   The frontend has no signal to branch the two paths. It carries `refund_deadline_hours`
   (relative) and `event_end_ms` (absolute), but neither the checked-in state nor an absolute
   `refund_deadline_ms`.
2. **`refund_deadline` is exposed only as relative hours** (`refund_deadline_hours: u32`), and
   the deadline is computed **for display only** via `compute_refund_info`
   (`frontend-leptos/src/pages/deposit/types.rs#L294-299`) — never used to gate the CTA.

A secondary issue: the doc comment on `event_refund_window_open` claims it "Mirrors
`refund::validate_and_update`: refunds are allowed iff `clock.unix_timestamp >= event_end`".
That is an **incomplete model** of the on-chain check (which also enforces `< refund_deadline`
for no-shows). The wrong contract model has been internalized in a comment, which is how this
drift survived.

**All other 22 error variants are `ok`** (handled correctly, organizer-only, or server-side
enforced with no client gap) — see §4.

---

## 2. Contract Surface — Error Variants (23)

Error codes are stable enum discriminants from `bethere-escrow/src/errors.rs`. The
`gap` column is the headline of this audit.

| #  | Error                       | Raised by (instruction)            | Worker endpoint(s)                                          | Frontend handling                                                                 | Gap            |
| -- | --------------------------- | ---------------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------------------- | -------------- |
| 0  | `IncorrectDepositAmount`    | `deposit`                          | `POST /deposit/usdc`, `/deposit/usdc/tx`                    | Amount comes from server `DepositStatusResponse.deposit_amount_usdc`              | **ok**         |
| 1  | `RefundNotYetAllowed`       | `refund` (`clock < event_end`)     | `POST /escrow/refund`                                       | CTA hidden until `event_refund_window_open(event_end_ms)`                         | **ok**         |
| 2  | `NotCheckedIn`              | `rollover_deposit`                 | rollover path (organizer)                                   | Organizer-side                                                                    | **ok**         |
| 3  | `RefundDeadlineNotPassed`   | `claim_forfeited`, `create_event`  | forfeited-claim, `POST /escrow/init`                        | Organizer-side (forfeit) / creation validation                                    | **ok**         |
| 4  | `AlreadyRefunded`           | `refund` (`refunded == true`)      | `POST /escrow/refund`                                       | CTA hidden once `refund_status == "refunded"`; on-chain is backstop               | **ok**         |
| 5  | `AttendeeCheckedIn`         | `claim_forfeited`                  | forfeited-claim (organizer)                                 | Organizer-side                                                                    | **ok**         |
| 6  | `NoForfeitedFunds`          | `claim_forfeited`                  | forfeited-claim (organizer)                                 | Organizer-side                                                                    | **ok**         |
| 7  | `EventNotActive`            | `deposit`                          | `POST /deposit/usdc`                                        | `usdc_deposits_accepted` flag hides the USDC option when escrow not `Initialized`  | **ok**         |
| 8  | `EventStillActive`          | `close_event`                      | `POST /escrow/close-event` (organizer)                      | Organizer-side                                                                    | **ok**         |
| 9  | `Unauthorized`              | `refund`, `rollover`, `claim_*`    | `POST /escrow/refund`, rollover, forfeit                    | PDA seeds + signer enforced server- and client-side                               | **ok**         |
| 10 | `VaultMismatch`             | `refund`, `deposit`                | `POST /escrow/refund`, deposit                              | Server builds TX with the escrow's stored vault ATA                               | **ok**         |
| 11 | `MintMismatch`              | `refund`, `deposit`                | `POST /escrow/refund`, deposit                              | Server builds TX with the escrow's mint                                           | **ok**         |
| 12 | `InvalidDepositAmount`      | `create_event`                     | `POST /escrow/init` (organizer)                             | Organizer-side / creation validation                                              | **ok**         |
| 13 | `EventEndInPast`            | `create_event`                     | `POST /escrow/init` (organizer)                             | Organizer-side / creation validation                                              | **ok**         |
| 14 | `Overflow`                  | `refund` (`total_refunded` add)    | `POST /escrow/refund`                                       | Defensive arithmetic; unreachable with USDC u64 amounts                           | **ok**         |
| 15 | `VaultNotEmpty`             | `close_event`                      | `POST /escrow/close-event` (organizer)                      | Organizer-side                                                                    | **ok**         |
| 16 | `EventEnded`                | `mark_checked_in` (SEC-011)        | check-in (scanner)                                          | Scanner is organizer-operated; on-chain rejects post-`event_end` check-ins        | **ok** (verify) |
| 17 | `DepositNotRefunded`        | `close_deposit`                    | refund+close pairing                                        | Worker pairs `refund + close_deposit` in one TX (`refund_and_close_tx_handler`)    | **ok**         |
| 18 | `EventEscrowStillActive`    | `close_event`                      | `POST /escrow/close-event` (organizer)                      | Organizer-side                                                                    | **ok**         |
| 19 | **`RefundDeadlinePassed`**  | `refund` (no-show, `clock >= rd`)  | `POST /escrow/refund`                                       | **NOT pre-gated** — CTA shown to no-shows past `refund_deadline`                  | **DIVERGENCE** |
| 20 | `EscrowVersionMismatch`     | `validate_version` (many ix)       | all escrow-touching                                         | Backward-compat guard; surfaces as generic 500 on version skew                    | **ok**         |
| 21 | `DepositVersionMismatch`    | `validate_version` (many ix)       | all escrow-touching                                         | Backward-compat guard                                                             | **ok**         |
| 22 | `RefundRequiresClose`       | `refund` (SEC-010 sysvar scan)     | `POST /escrow/refund`                                       | Worker always pairs with `close_deposit` (`refund_and_close_tx_handler`)          | **ok**         |

**Legend**: `ok` = client/server model matches program; `verify` = believed correct but wants
an integration test (§5); `DIVERGENCE` = client permits an action the program rejects (or vice
versa).

---

## 3. The Refund Window — On-chain Truth vs Client Model

This is the heart of the audit, so it is spelled out in full.

### 3.1 On-chain truth (`bethere-escrow/src/instructions/refund.rs#L77-85`)

```event-checkin/bethere-escrow/src/instructions/refund.rs#L72-85
        // Verify event has ended.
        if clock.unix_timestamp.get() < self.event_escrow.event_end() {
            return Err(EscrowError::RefundNotYetAllowed.into());
        }

        // If attendee was NOT checked in, they must refund before the deadline.
        // After refund_deadline, the organizer can claim no-show deposits.
        // Checked-in attendees can refund anytime — they showed up.
        if !self.attendee_deposit.checked_in()
            && clock.unix_timestamp.get() >= self.event_escrow.refund_deadline()
        {
            return Err(EscrowError::RefundDeadlinePassed.into());
        }
```

Formally, a refund transaction succeeds iff:

- `clock >= event_end` (both paths — else `RefundNotYetAllowed`), AND
- **either** `attendee.checked_in == true` (no upper bound),
- **or** `clock < refund_deadline` (no-show path — else `RefundDeadlinePassed`).

Note `refund_deadline` on-chain is an **absolute** `i64` unix-timestamp (seconds), stored on
`EventEscrow` (`state.rs#L34-38`) and set at `create_event` time with the invariant
`refund_deadline > event_end` (`create_event.rs#L58-62`, proven by Kani at
`kani.rs#L185-197`).

### 3.2 Client model (`frontend-leptos/src/pages/deposit/types.rs#L196-198`)

```event-checkin/frontend-leptos/src/pages/deposit/types.rs#L188-198
/// Whether the on-chain refund window is open as of the client clock.
///
/// Mirrors `bethere-escrow::instructions::refund::validate_and_update`:
/// refunds are allowed iff `clock.unix_timestamp >= event_end`. The on-chain
/// check uses seconds; we compare in milliseconds for consistency with
/// `DepositStatusResponse.event_end_ms`.
///
/// Treats `event_end_ms <= 0` (legacy/missing field) as "not yet open" so
/// the refund CTA stays hidden on bad data — fails safe.
pub fn event_refund_window_open(event_end_ms: i64) -> bool {
    event_end_ms > 0 && now_ms() >= event_end_ms
}
```

The client computes: `refund_window_open = (now >= event_end)`. **No `refund_deadline`, no
`checked_in`.** The doc comment's claim that this "mirrors `validate_and_update`" is wrong:
it mirrors only the first half of the on-chain check.

### 3.3 Where the gate is wired

Two call sites render or route to the refund flow off this single predicate:

```event-checkin/frontend-leptos/src/pages/deposit/already_deposited.rs#L77-81
    // hides the CTA until the client clock passes event_end_ms. Fails safe
    // (treats missing/zero event_end_ms as "not yet open").
    let event_ended = event_refund_window_open(data.event_end_ms);
```

```event-checkin/frontend-leptos/src/pages/deposit/mod.rs#L331-341
                        DepositPageState::RefundChooseWallet(data) => {
                            if event_refund_window_open(data.event_end_ms) {
                                let wallets = detected_wallets.get();
                                refund::refund_choose_wallet_view(
                                    &data,
                                    &wallets,
                                    set_state,
                                    handle_refund_connect_wallet.clone(),
                                )
                            } else {
                                already_deposited::already_deposited_view(&data, &set_state)
```

So the same `event_end`-only predicate decides both (a) whether the CTA is shown and (b) whether
the refund wallet-picker view is reachable. Neither path consults `refund_deadline` or
`checked_in`.

### 3.4 The server does not save us

The worker's `refund_and_close_tx_handler` (`worker/src/handlers/deposit/escrow/handlers.rs#L168`)
performs only these soft checks before building the TX: `deposit_enabled`, non-empty
`escrow_address`, valid `wallet_address`, `status.verified`, `method == Usdc`, and
`status.refundable` (tier). **It does not check `refund_deadline` or `checked_in`** — it
intentionally delegates the time-window enforcement to the program. So a no-show past the
deadline gets a *valid-built* transaction that then reverts on-chain with
`RefundDeadlinePassed`. The user experiences this as: click → sign → fail.

### 3.5 Why `checked_in` is structurally absent

`DepositStatus` (`domain/src/models/deposit.rs#L46-101`) carries: `attendee_id`, `event_id`,
`method`, `amount`, `currency`, `tx_signature`, `verified`, `deposited_at`, `wallet_address`,
`deposit_order`, `refundable`, `rejected`. **No `checked_in`.** The on-chain
`AttendeeDeposit.checked_in` flag (`state.rs#L68-72`) is not mirrored into the API response,
so the frontend has no way to branch the two refund paths even if the gate function were
extended. This is the deeper structural fix behind the headline divergence.

---

## 4. Divergence #19 — `RefundDeadlinePassed` (detailed)

- **Symptom**: No-show attendee, after `refund_deadline`, opens the deposit page and sees a
  working "Request Refund" CTA. Clicking it builds a TX (`POST /escrow/refund`), the wallet
  prompts for a signature, and the TX reverts on-chain with code `19 (RefundDeadlinePassed)`.
- **Impact**: UX/frustration + a wasted signature. **Not funds-at-risk**: the program correctly
  forbids the refund, and the organizer may already have claimed the deposit as forfeited via
  `claim_forfeited` (which requires `!checked_in && clock >= refund_deadline`).
- **Who hits it**: any no-show who returns to the deposit page between `refund_deadline` and
  whenever the UI is next updated. For a 7-day default window, this is the entire second week
  after the event.
- **Severity**: Medium. Real users will hit it; it just shouldn't be possible to click.

### Recommended fix (two parts, both within plan 005 scope per §3.2)

1. **Expose the data.** Add `checked_in: bool` and `refund_deadline_ms: i64` (absolute) to
   `DepositStatusResponse`. The worker can read `checked_in` from the on-chain
   `AttendeeDeposit` (already fetched elsewhere) or from the event-store mirror; the absolute
   deadline is `event_end_ms + refund_deadline_hours * 3_600_000`.
2. **Fix the gate.** Replace `event_refund_window_open` with a predicate that encodes the
   two-path model:

   ```text
   refund_cta_enabled =
       (now >= event_end) AND (
           checked_in                              // [event_end, ∞)
           OR now < refund_deadline_ms             // [event_end, refund_deadline)
       )
   ```

   Also fix the doc comment so it stops advertising the incomplete model as a mirror of the
   program. Keep the existing "fail safe on missing data" behavior (if `checked_in` unknown,
   treat as no-show → stricter).

### Edge case to decide

If `checked_in` cannot be read cheaply for a given attendee (e.g. no on-chain deposit record
found), the safe default is to assume **no-show** (stricter gate). This may hide the CTA from
a checked-in user momentarily, but that is strictly better than showing a dead CTA to a no-show.
The deposit-status fetch path should be audited for this as part of the fix.

---

## 5. "Verify" Items (believed correct, want an integration test)

These are marked `ok (verify)` in §2 and should be covered by plan 005 §3.3 (LiteSVM) and/or
§3.4 (E2E harness):

- **#16 `EventEnded` on `mark_checked_in`** — the scanner flow should gracefully reject
  post-`event_end` check-ins. Confirm the scanner UI surfaces the revert reason rather than a
  generic toast. (On-chain guard SEC-011 is in `mark_checked_in.rs`; the scanner is
  organizer-operated so the blast radius is small, but the harness should exercise it.)
- **#4 `AlreadyRefunded`** — confirm the CTA stays hidden once `refund_status == "refunded"`
  reaches the client, and that a stale client (cached pre-refund status) still gets a clean
  on-chain rejection rather than a confusing retry loop.
- **#22 `RefundRequiresClose`** — confirm the worker's paired `refund + close_deposit`
  transaction (`refund_and_close_tx_handler`) always satisfies the SEC-010 instruction
  introspection across all wallet adapters (some adapters reorder or strip instructions).

---

## 6. Worker Escrow-Touching Endpoint Surface

For cross-reference with plan 005 §3.2 / the harness flow list. Routes from
`worker/src/handlers/mod.rs` (per plan 005 §1 evidence):

| Route                          | Builds/executes ix                                  | Error codes it can surface                 |
| ------------------------------ | --------------------------------------------------- | ------------------------------------------ |
| `POST /deposit/usdc`           | `deposit` (Solana Pay URL)                          | 0, 7, 9, 11                                |
| `POST /deposit/usdc/tx`        | `deposit` (serialized TX)                           | 0, 7, 9, 11                                |
| `POST /deposit/hold`           | (off-chain THB; no ix)                              | n/a (worker validation only)               |
| `GET  /deposit/status/{id}`    | (read)                                              | n/a                                        |
| `POST /escrow/init`            | `create_event`                                      | 3, 12, 13, 20                              |
| `POST /escrow/refund`          | `refund` + `close_deposit` (paired)                 | **1, 4, 9, 10, 11, 14, 17, 19, 21, 22**    |
| `POST /escrow/close-event`     | `close_event`                                       | 8, 15, 18                                  |
| `GET  /escrow/cancel-status`   | (read)                                              | n/a                                        |
| forfeited-claim path           | `claim_forfeited`                                   | 3, 5, 6, 9                                 |
| check-in (scanner)             | `mark_checked_in`                                   | 2 (rollover), 16, 20, 21                   |
| rollover path                  | `rollover_deposit`                                  | 2, 4, 9, 20, 21                            |
| `GET  /claim/{token}`          | (NFT mint; separate program)                        | n/a for escrow                             |

The `POST /escrow/refund` row is where #19 surfaces to end users — and the only row whose
client-side gate is currently wrong.

---

## 7. Out of Scope / Not Audited Here

- **NFT claim flow** (`/claim/{token}`) — uses a separate cNFT program (Helius), not the
  escrow. Its error surface is out of scope for this inventory.
- **THB / PromptPay refund flow** — purely off-chain (KV + R2 slips, admin-verified). No
  on-chain error codes. The THB refund *queue* and *batch* endpoints are not escrow-touching.
- **`introspection` instruction** — read-only program introspection used for debugging; cannot
  fail in a user-relevant way.
- **The actual fix for #19** — this doc is the **audit**. The fix (§4) is implemented as
  separate commits within plan 005, per its §3.2 ("Contract divergence fixes … fixed as part of
  this plan, not deferred").

---

## 8. Status Checklist

- [x] All 23 escrow error variants mapped to instruction / endpoint / frontend / gap.
- [x] Two refund paths audited (checked-in vs no-show).
- [x] `refund_deadline` exposure audited (relative hours, not absolute; display-only).
- [x] `checked_in` exposure audited (absent from `DepositStatusResponse`).
- [x] `deposit_order` / `max_refundable_deposits` tier logic present (`is_refundable_tier`) and
      server-enforced (`status.refundable` check in `refund_and_close_tx_handler`).
- [x] `close_deposit` ↔ `refund` coupling (SEC-010) audited; worker pairs them.
- [ ] Fix #19: expose `checked_in` + absolute `refund_deadline_ms` in `DepositStatusResponse`.
- [ ] Fix #19: rewrite `event_refund_window_open` to the two-path predicate + correct doc comment.
- [ ] LiteSVM test for the two refund paths (plan 005 §3.3).
- [ ] E2E harness flow: `refund_no_show_deadline` (plan 005 §3.4).
