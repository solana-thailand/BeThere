# 027 — Unattended hardening queue (2026-09-22 → )

**Owner is asleep. This plan is written to be executed without asking anything.**
Every item below is *ungated*: no owner decision, no external account, no
irreversible action. Anything needing the owner stays in `.issues/129` §7 and is
listed in §9 here as explicitly NOT to be started.

---

## 0. Rules of engagement (read before touching anything)

**Re-ranked 2026-09-22, after writing this:** C and D moved ahead of B. B's
damage is held back by four independent blockers (`NOTIFICATIONS_ENABLED=0`, no
caller, empty `NOTIFICATION_FROM`, no `EMAIL` binding), so the code it needs is
dead on arrival; C and D are live on the deposit path and RTM #6 is on
2026-09-27. Order actually executed: **A → C → D → B → E → F**.

1. **Additive only.** No behaviour an attendee or organizer sees today may
   change meaning. New columns get defaults; new gates start in report-only
   mode; new env flags default to today's behaviour.
2. **Size budget is law.** The worker ships to Cloudflare's **free plan: 3 MiB
   after gzip**. Measured 2026-09-22 on the deployed bundle:

   the **wrangler dry-run bundle** — i.e. exactly what gets uploaded, not the
   raw cargo artifact:

   | file | raw | gzip |
   |---|---:|---:|
   | `…-event_checkin_worker_bg.wasm` | 4 481 545 | 1 536 840 |
   | `shim.js` (esbuild output) | 108 146 | 20 347 |
   | **total** | **4 589 691** | **1 557 187 — 49.50 % of 3 MiB** |

   (`shim.js.map` is in the outdir but is *not* uploaded, so it is not counted.)
   Headroom is **1 588 541 bytes gzip**. That is real but not infinite: any item that
   pulls in an image decoder or a QR decoder must be measured *before* it is
   merged, not after. Item **A** below makes this measurement automatic and
   fail-closed; **do A first** — every later item depends on it to be safe.
3. **Verify in both directions.** A guard that cannot fail is not a guard
   (`.issues/072`). For each new test: break the thing on purpose, watch the
   test go red with the right message, revert, watch it go green. Record both
   in the commit body.
4. **Staging before prod, always.** Batch the night's work into ONE prod deploy
   at the end, not one per item — fewer prod versions, one backup, one rollback
   point. `bethere-staging` is the rehearsal.
5. **Back up D1 before any prod migration:**
   `npx wrangler d1 export bethere-db --remote --output backups/pre-0046-$(date +%Y%m%d).sql`
   — and keep it out of git (it contains PII).
6. **Never `d1 execute` an events row in prod.** The read path is KV-first; a
   direct D1 write is masked by cache. Deposits/notifications tables are fine.
7. Local shell gotchas: always `rg --color=never` (this shell eats the match
   highlight and silently mangles output); `timeout(1)` does not exist on this
   box; `cargo check`/`test` on the worker take >2 min because of host↔wasm
   target switching — run them backgrounded.

---

## A. Worker size budget gate  ·  value: HIGH  ·  risk: none  ·  size: 0

**Why first.** The owner's standing worry is "will this still fit the free
tier". Today nothing measures it: `deploy.sh` has no size check and CI's
`wasm-build` job builds the wasm and throws the number away. Every item after
this one adds code, so without A we would be adding code blind. The number we
care about is *gzip of the uploaded bundle*, which nobody has ever recorded.

**Scope**
- `scripts/verify/worker_size_budget.sh` — gzip every file wrangler uploads
  (`worker/build/worker/**`, excluding source maps), sum, compare against a
  budget file, print the number as a percentage of the 3 MiB free-plan ceiling.
- `worker/.size-budget` (tracked — `worker/build/` is gitignored, so the
  budget cannot live there) — the committed baseline + ceiling.
  Two thresholds: **warn** at 70 % of 3 MiB, **fail** at 85 % (2.55 MiB). Fail
  leaves ~450 KiB of emergency headroom rather than discovering the limit
  during an outage.
- Wire into `deploy.sh` *before* the upload step (fail-closed), and into the CI
  `wasm-build` job so a size regression reds a PR, not a deploy.
- The gate prints the per-file breakdown on failure so the culprit is obvious.

**Verify both directions**: five cases, all run 2026-09-22 —
green at 49.50 % with delta +0 against the baseline; fail line moved below the
current size → red, naming the wasm, exit 1; warn line moved below → warning,
exit 0; empty bundle dir → refuses to pass vacuously; a budget key deleted →
refuses to fall back to a default.

**Done when** `bash scripts/verify/worker_size_budget.sh` is green locally, the
CI step exists, and `deploy.sh` refuses a bundle over budget.

**STATUS 2026-09-22: DONE.** `scripts/verify/worker_size_budget.sh` +
`worker/.size-budget` + `deploy.sh:run_size_budget_gate` (invoked before the
upload, aborts on failure) + a `wasm-build` CI step + five guards in
`worker/tests/size_budget_guards.rs`.

---

## B. Notification backlog can never blast the archive  ·  value: HIGH  ·  risk: none  ·  size: ~0

**Why.** `.issues/128`: the product has never sent an email
(`NOTIFICATIONS_ENABLED = "0"` in both environments), and `notification_outbox`
has accumulated **243 pending rows** whose `due_at` is long past. Flipping the
flag — which is a one-character edit someone will eventually make — sends all
243 at once, to people whose events ended months ago. The flag is the only
thing standing between today and that incident, and a flag is not a control.

**Scope**
- A **staleness cutoff** in the dispatcher (`worker/src/notifications/`): a row
  whose `due_at` is older than `NOTIFICATION_MAX_AGE_HOURS` (default **72**) is
  never sent — it is moved to `cancelled` with `error_code='stale'`, so the
  decision is auditable rather than silent.
- Cutoff applies at *send* time, not enqueue time: a genuinely delayed send
  (worker down for an hour) still goes out; a four-month-old reminder does not.
- A **first-enable guard**: if the number of rows that would be sent in the
  first pass after enabling exceeds `NOTIFICATION_BURST_LIMIT` (default 25),
  the pass sends nothing, Slack-alerts the count, and requires the limit to be
  raised deliberately. Belt and braces: the cutoff should already have drained
  the 243, and this catches the case where it has not.
- Both defaults are conservative and both are env-tunable in `wrangler.toml`.

**Verify both directions**: a unit test with a fabricated outbox — one fresh
row (sends), one 200-day-old row (cancelled as stale), and a 26-row fresh batch
(burst guard trips, zero sends). Then flip the age to 0 and confirm the fresh
row is *also* cancelled, proving the comparison is live and not hardcoded.

**Done when** enabling `NOTIFICATIONS_ENABLED` on staging with the real backlog
shape sends **zero** historical mail, and the outbox shows 243 `cancelled/stale`.

---

## C. A slip image can only be used once  ·  value: HIGH  ·  risk: low  ·  size: ~0

**Why.** The owner's words: *anyone can upload any image to the deposit page*,
and they catch the bypassers by remembering faces. The cheapest deterministic
defence needs **no bank API, no vendor, no OCR and no new dependency**: hash the
bytes. The same slip re-used by a second attendee — the single most common real
abuse — becomes impossible. `blake3` is already compiled into the worker via
`event-checkin-domain`'s `wire` feature, so this costs ~zero bundle size.

This is deliberately *not* the QR/bank-reference work (item G). It catches byte
duplicates only; a re-screenshotted slip defeats it. It is still worth shipping
first because it is a night's work with no external dependency, and it makes
the review queue in G much smaller.

**Scope**
- Migration `0046_thb_deposit_slip_hash.sql`: add `slip_blake3 TEXT` (nullable
  — historical rows have no hash and must not be invented) + a **partial**
  unique-ish index for lookup: `CREATE INDEX idx_thb_slip_hash ON
  thb_deposits(slip_blake3) WHERE slip_blake3 IS NOT NULL`.
  *Not* a `UNIQUE` constraint: a legitimate re-upload after rejection must
  still be possible; the check belongs in the handler where it can distinguish
  "same person re-uploading" from "different person, same image".
- `slip_upload.rs`: hash the **decoded image bytes** (not the data-URL string —
  base64 padding and MIME casing differ for identical images) before the R2
  upload; look up the hash; if it belongs to a *different* attendee, reject with
  a message that does not leak who (`AppError::Validation`, no other attendee's
  identity). If it belongs to the same attendee, allow (re-upload).
- Same treatment in `slip_admin_upload.rs` — memory `duplicated-state-transition-paths`
  says a guard that lands on one writer and not its sibling is the recurring
  defect here. **Both writers or neither.**
- Backfill is *not* attempted: historical slips live in R2 and re-hashing them
  means 243 R2 reads inside a migration. A separate opt-in script
  (`scripts/verify/backfill_slip_hashes.sh`) can do it later, read-only-first.

**Verify both directions**: local D1 harness — upload slip X as attendee 1
(accepted), same bytes as attendee 2 (rejected), same bytes as attendee 1 again
(accepted), a one-pixel-different image as attendee 2 (accepted, proving it is
not hashing something constant).

**Done when** the four cases above pass against `wrangler dev --local`, and the
admin path is covered by the same test.

**STATUS 2026-09-22: DONE (commit `f01cd24`), not deployed.** Shipped in
`report` mode. Eight runtime cases verified against real SQLite, including the
two that only SQLite can show (`''` vs `''`, `NULL` vs `NULL`) and that the
partial index is actually used. Bundle cost measured by item A: **+7,883 bytes
gzip, 49.50 % → 49.75 %**. Written up in `.issues/130`. The admin UI shows the
collision above the Approve button. Deviation from the plan as written: the
lookup is a targeted SQL query on the new index rather than a Rust scan over
`list_thb_deposits`, because that read can pull multi-MB data URLs for old rows.

---

## D. Admit an attendee without promising them ฿500  ·  value: HIGH  ·  risk: low  ·  size: ~0

**Why.** `.issues/129` Gap 1, and it is the bug under the owner's whole manual
ritual. Approving a slip is the **only** way an attendee receives their ticket
QR (`slip_verify.rs:133`, approval-only auto-QR) — and approving is
simultaneously the promise to refund ฿500. So when the organizer knows someone
did not really pay, they have no move that admits the person without creating a
refund obligation. They compensate by *remembering*.

The correct state already exists — `DepositSource::Comp`
(`domain/src/models/deposit.rs:165`) means never-cash, never-refundable — but it
can only be created **at signup, for staff/organizer**
(`register/signup.rs:824-827`). No admin action can comp an ordinary attendee
after they have uploaded.

**Scope**
- Migration `0047_thb_deposit_source.sql`: a real `deposit_source TEXT` column
  (`'cash' | 'credit' | 'comp'`, CHECK-constrained), backfilled from the
  *existing* classifier so the column starts out exactly equal to what
  `ThbDeposit::source()` already returns. No row changes meaning on migration
  day — that is the invariant to assert in the test.
- `ThbDeposit::source()` reads the column when present and falls back to the
  sentinel sniffing when NULL. **One classifier, two inputs** — do not fork the
  rule into a second function (DRY; the sentinels in `verified_by` / `slip_url`
  carry other meanings and must keep working for old rows).
- Admin action `POST /api/admin/deposit/thb/comp` — marks a deposit
  `source='comp'`, `verified=1`, `refundable=0`, and issues the ticket QR by
  the *same* code path approval uses (do not duplicate the QR issue: extract
  it once and call it from both — the duplicated-writer trap again).
- Admin UI: one button next to Approve/Reject, labelled so it cannot be
  misread — "Admit, no refund owed" — with the ฿ consequence written under it.

**Verify both directions**: migration-day invariant test (for every existing
row, column value == legacy classifier output); comp action produces a QR and a
non-refundable deposit; the refund/hold/roll paths all *skip* a comp row (they
already do via `is_non_cash()` — assert it, do not assume).

**Done when** an ordinary attendee can be admitted from the admin screen with
no refund obligation created, and no existing row changed classification.

---

## E. `.issues/127` — the UNIQUE that cost ฿700  ·  value: MED  ·  risk: MED  ·  size: 0

`thb_deposits` has no `UNIQUE(event_id, attendee_id)`, so a double upload makes
two rows and the second refund is real money. The previous thread deliberately
deferred the fix until **after RTM #6 (27 Sep)** — a schema rebuild on the live
deposits table days before an event is the wrong trade.

**Tonight: prepare only, do not apply.**
- Write `0048_thb_deposits_unique.sql` (table rebuild — SQLite cannot add a
  constraint in place; follow the 0035 pattern exactly, including dropping and
  recreating dependent triggers/views).
- Write `scripts/verify/thb_duplicate_report.sh` — read-only, lists rows that
  *would* collide, so the dedupe decision is made with the real list in hand.
- Run the report against prod (read-only) and paste the result into
  `.issues/127`.
- Mark the migration file `-- DO NOT APPLY BEFORE 2026-09-28` at the top and
  leave it unreferenced by any runner.

**Done when** the migration and the report exist, the report has been run, and
`.issues/127` names the exact rows.

---

## F. Batch deploy  ·  value: MED  ·  risk: MED

Only after A–D are green locally. In order:
1. `bash scripts/verify/worker_size_budget.sh` — must be green, record the %.
2. Full local gates: `cargo fmt --all -- --check`, `cargo clippy --workspace
   --all-targets -- -D warnings`, `cargo test --workspace`, the python suites.
3. Deploy to **staging**, apply migrations to the staging D1, run the item-B
   and item-C verifications against the real staging shape, and re-measure the
   bundle from the *staging* build (this is the owner's "prove it in staging"
   requirement).
4. Back up prod D1 (§0.5), apply migrations, deploy prod, smoke-test —
   including **Content-Type headers**, not just HTTP 200.
5. Record the deployed version id so rollback is one command.

Also carries the already-committed `3973792` (nightly-cleanup Slack alerting),
which has been sitting on `develop` undeployed.

---

## G. Slip QR → bank reference  ·  value: HIGH  ·  risk: MED  ·  **size: MUST MEASURE**

The real fix for "anyone can upload any image": Thai bank slips carry a mini-QR
holding a bank transaction reference. Decode it, store the reference with a
**schema-level** `UNIQUE` (not a code check — `.issues/127` proved what that
distinction costs), and later have a vendor resolve it against the bank.

**Decoding the QR needs no vendor and no owner decision — only bundle size.**
A QR decoder plus a JPEG/PNG decoder in wasm is the first thing in this whole
plan that could plausibly move the 1.48 MiB number. Therefore:
- Do this **after** item A, on a **branch**, and the first commit on that branch
  is a size measurement, not a feature.
- Budget: if `rqrr` + `image` (jpeg+png features only, `default-features =
  false`) pushes the gzip total past the item-A *warn* line (70 %), stop and
  write up the alternative (decode in a second Worker, or at the edge in the
  frontend with the server re-verifying the reference against the bank) rather
  than shipping it.
- The bank-resolution half stays owner-gated (vendor account, PII) — decoding
  and the UNIQUE reference do not.

---

## H. Housekeeping (whenever the queue stalls)

- `cargo clippy --workspace --all-targets --fix --allow-dirty` then review the diff.
- Doc-sync: `.issues/084` and `.plans/026` carry 2026-09-22 corrections; check
  no other doc still repeats the stale "blocked on devnet USDC" claim.
- Confirm `.gitignore` still covers `worker/scripts/.preflight-bypass.log`
  (public repo, owner's call — never un-ignore).
- Re-run `scripts/verify/check_sbpf_v3_gate.sh` and record the date; the v3
  deployment gate has been closed on all three clusters.

---

## 9. Explicitly NOT to be started (owner-gated)

Do not start these even if the queue empties. They need a decision or an
account in the owner's name, and guessing wrong costs money or sends mail.

- **`.issues/129` §7 all four**: what "refund at check-in" must mean;
  credit-by-default at check-in; slip-verify vendor (RDCW vs EasySlip — the
  vendor sees slip PII); whether a juristic entity exists to hold a payout
  account.
- **Flipping `NOTIFICATIONS_ENABLED` to 1** anywhere. Item B makes it *safe*;
  it does not make it *decided*. Sender-domain onboarding is still unstarted
  and has external DNS lead time.
- **PARIPOL / linked emails rollout** (`person_emails` has 0 rows; RTM #6 is
  27 Sep).
- **The ฿500 owed to RTM #3 attendee `019ec036…`** — a real payment.
- **The 13 orphaned R2 slip images** — deletion is irreversible.
- **Escrow `checkin_authority`** (`.issues/129` Gap 3): the design is sound and
  the 36 bytes of `_padding` in `EventEscrow` fit a 32-byte key without
  resizing, but it is a program redeploy and `.issues/123` (SBPFv0 bytecode)
  is unresolved. Devnet-only and reversible, so it may be *prototyped* — but
  not promoted, and not before F.

---

## 10. สรุปภาษาไทย

- **กติกา**: ทำได้เฉพาะของที่ *เพิ่มเข้าไป* ไม่เปลี่ยนพฤติกรรมเดิม, ขึ้น staging ก่อน prod เสมอ,
  และ deploy prod ครั้งเดียวตอนจบ (ไม่ deploy ทีละอัน)
- **เรื่อง size ที่ห่วง**: วัดแล้ววันนี้ — bundle จริง **1.48 MiB (gzip)** จากเพดาน free tier **3 MiB**
  = ใช้ไป **49%** เหลือที่ว่างอีกราว 1.5 MiB · ข้อ **A** คือทำให้มันวัดอัตโนมัติและ **fail ก่อน deploy**
  ถ้าเกิน 85% จะได้ไม่ต้องเดาอีก
- **ลำดับงาน**: A วัด size → B กันอีเมลเก่า 243 ฉบับยิงออกตอนเปิดระบบ → C สลิปรูปเดิมใช้ซ้ำไม่ได้
  (ใช้ blake3 ที่มีอยู่แล้ว ไม่เพิ่ม size ไม่ต้องต่อ API ธนาคาร) → D ปุ่ม "ให้เข้างานแต่ไม่ติดหนี้คืนเงิน"
  (แก้บั๊กที่ว่า approve = สัญญาคืน ฿500 เลยต้องจำหน้าเอา) → E เตรียม migration #127 ไว้ก่อน *ยังไม่ apply*
  รอหลัง RTM#6 (27 ก.ย.) → F deploy รวดเดียว → G ถอด QR ในสลิป (ต้องวัด size ก่อน อันนี้เสี่ยงบวม)
- **ที่จะไม่แตะเลยเพราะต้องให้คุณตัดสิน**: 4 ข้อใน `.issues/129` §7, การเปิดส่งอีเมลจริง,
  PARIPOL, เงิน ฿500 ที่ค้างคนเดิม, ลบรูปสลิป 13 ไฟล์, และ deploy โปรแกรม escrow ตัวใหม่
