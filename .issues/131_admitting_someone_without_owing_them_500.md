# 131 — Admitting someone without owing them ฿500

**Status:** implemented 2026-09-22, not yet deployed.
**Closes:** `.issues/129` Gap 1.
**Severity:** high — it is the mechanism behind the owner's whole manual refund
ritual, and it is why the knowledge of who bypassed lives in a person's memory.

## 1. The bug, stated plainly

Approving a payment slip is the **only** way an attendee receives their ticket
QR (`slip_verify.rs`, approval-only auto-QR). Approving is **also** the promise
to refund their ฿500.

So an organizer who knows somebody did not really pay has exactly two moves:

1. approve → they get in, and the organizer now owes them ฿500; or
2. reject → they are told their payment failed and they do not get in.

Neither is what the organizer wants when the answer is "let them in, but I'm not
paying that back". In practice they choose (1) and keep a mental list. That list
is the reason refunds still require them to be physically present, going down
the attendee list from memory — the thing `.issues/129` was opened to fix.

The state that should have carried this already existed — `DepositSource::Comp`,
which every refund/hold/roll rule honours through `is_non_cash()` — but:

- it could only be created **at signup, for staff and organizers**
  (`register::signup::record_staff_comp`), and
- it was *detected* by sniffing sentinels out of two columns that mean other
  things (`verified_by = 'SYSTEM_STAFF_WAIVE'`, `slip_url = 'STAFF_COMP_WAIVED'`,
  `amount_thb = 0`).

So reclassifying a real slip meant destroying the evidence: blanking `slip_url`
or zeroing `amount_thb`. Nobody was going to do that, and they shouldn't have.

## 2. The fix

**Migration 0047** adds `thb_deposits.deposit_source` (`'cash' | 'credit' |
'comp'`, CHECK-constrained, nullable), backfilled from the existing classifier.

**`ThbDeposit::source()`** returns the recorded column when set and falls back
to the legacy sentinels when NULL. One classifier, two inputs — never two
classifiers. The fallback is not scaffolding: those sentinels are still what
`record_staff_comp` and the rolling-credit application write.

**`POST /api/deposit/thb/comp`** (organizer-authed) admits the attendee and
writes off the deposit. It sets `deposit_source = comp`, `verified = true`,
clears `refundable` on the deposit status, issues the ticket QR, and writes a
`DepositCompedByAdmin` audit entry. `amount_thb` and `slip_url` are left exactly
as uploaded — the record of what was claimed is evidence, and not having to
destroy it is the entire point of the column.

**Admin UI:** a third button beside Approve/Reject — "Admit, no refund owed" —
with a two-step confirmation (same as Reject, because this writes off real
money) and a tooltip saying what it does.

**`admit::issue_ticket_qr_if_absent`** is new, and `slip_verify.rs` now calls
it too. That block used to be written out twice in the verify handler, once per
`worker_ctx` branch. A comped attendee's ticket is issued by the same code as an
approved one, so the two cannot drift.

## 3. Guards on the money

Refused, with a reason the organizer can act on:

| state | why comping is refused |
|---|---|
| already refunded | the cash has gone back out; there is nothing to write off |
| already held as rolling credit | it would delete credit the attendee can already spend |
| **covered by rolling credit** | that ฿500 came out of their balance — comping consumes credit they are still owed |
| already comped | idempotent success, not an error |

The credit case is the subtle one and the most expensive to get wrong.

## 4. Migration-day invariant

No existing row changes classification. The backfill's `CASE` is a
transcription of `source()`'s fallback **in the same order**, and order is
load-bearing: a row that is both rolling-credit and ฿0 is Credit, not Comp.
Reversing those two arms would backfill every credit-covered ฿0 deposit as a
comp — writing off money the attendee still owns.

Verified against real SQLite (`wrangler d1 --local`, all 47 migrations applied
in sequence): 7 fixture rows, one per classifier branch plus the
credit-and-฿0 trap, all backfilled to exactly what `source()` returns. The CHECK
constraint rejects `'refunded'` and accepts `'comp'`.

## 5. Verification

- 4 domain tests: the sentinel fallback still classifies every legacy shape;
  credit beats comp when a row matches both; a recorded source overrides the
  sentinels **with the evidence intact**; and the column wins even when it
  disagrees with a sentinel (otherwise it would be decorative).
- 7 worker guards: backfill order matches the Rust classifier and reads every
  sentinel; the migration is additive and CHECK-constrained; comping never
  writes `amount_thb = 0` or blanks `slip_url`; all three settled states are
  refused; `refundable` is cleared; comp and approval issue the same ticket
  **and** the verify path calls the QR helper exactly once (so the old
  duplication cannot come back); the route is in the authed router.
- Whole tree: 758 workspace + 205 frontend tests pass, clippy `-D warnings`
  clean, fmt clean.
- Bundle: **+4,618 bytes gzip, 49.75% → 49.89%** of the free-plan ceiling.

## 6. Remaining

- [ ] Deploy with migrations 0046 + 0047.
- [ ] Comping an attendee who never submitted a deposit at all (a walk-in guest)
      is **not** supported — the endpoint requires an existing row. Signup-time
      comp covers staff; a guest who never uploaded still has no path. Worth a
      follow-up, but it is a different flow and was left out deliberately.
- [ ] `.issues/129` §7 still holds the four owner decisions. This changes none
      of them; it makes the ฿500 question answerable per-attendee instead of
      per-memory.

## Related

- `.issues/129` — the design; Gap 1 is this.
- `.issues/130` — the duplicate-slip flag. Together they close the loop the
  owner described: the system flags the suspicious slip, and the organizer has
  a move that is neither "owe them ฿500" nor "turn them away".
- `.plans/027` item D.

## 7. สรุปภาษาไทย

- **บั๊ก:** กด Approve สลิป = ทางเดียวที่ผู้เข้าร่วมจะได้ QR เข้างาน **และ** = สัญญาว่าจะคืน ฿500
  เลยเหลือแค่ 2 ทาง คือ "ให้เข้า + ติดหนี้" หรือ "ไม่ให้เข้า" → เจ้าของงานเลยเลือกกด Approve
  แล้วจำเอาเองว่าใครไม่ต้องคืน ซึ่งคือสาเหตุที่การคืนเงินต้องมีเจ้าของอยู่ด้วยตลอด
- **ที่แก้:** เพิ่มคอลัมน์ `deposit_source` (migration 0047) + ปุ่มที่ 3 ในหน้าแอดมิน
  **"Admit, no refund owed"** — ให้เข้างานได้ QR เหมือนกดอนุมัติทุกอย่าง แต่บันทึกว่าเป็น comp
  ไม่เข้าคิวคืนเงิน · **ไม่ลบหลักฐาน**: ยอด ฿500 กับรูปสลิปยังอยู่ครบ (เมื่อก่อนถ้าจะทำแบบนี้
  ต้องไปล้าง slip_url หรือเซ็ตยอดเป็น 0 ซึ่งคือทำลายหลักฐาน)
- **กันพลาด:** ห้าม comp ถ้าคืนเงินไปแล้ว / แปลงเป็นเครดิตไปแล้ว / **จ่ายด้วยเครดิตของเขาเอง**
  (อันหลังสำคัญมาก — ถ้า comp จะเท่ากับกินเครดิตที่เขายังเป็นเจ้าของอยู่)
- **ยืนยันแล้ว:** ข้อมูลเดิมทุกแถวยังถูกจัดประเภทเหมือนเดิมเป๊ะ (ทดสอบกับ SQLite จริง 7 เคส)
  · เทสต์ผ่าน 758 + 205 · ขนาด bundle เพิ่ม 4,618 ไบต์ = **49.89%** ของเพดาน free tier
