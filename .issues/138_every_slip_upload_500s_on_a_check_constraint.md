# 138 — Every slip upload 500s: an empty string where the CHECK wants NULL

**Status:** cause proven, fixed, guarded — 2026-09-23. **A regression I
introduced the same morning.**
**Found:** 2026-09-23, owner reported an attendee who transferred money that
morning could not upload their slip: `Failed to upload slip: API error (500):
internal error`.
**Severity:** CRITICAL. Every THB slip upload in production failed, for every
attendee, from the moment migration 0047 was applied. Attendees who had
already sent money could not prove it. RTM#6 is 2026-09-27.

## 1. What broke, and who broke it

Migration `0047_thb_deposit_source.sql` adds:

```sql
ALTER TABLE thb_deposits ADD COLUMN deposit_source TEXT
    CHECK (deposit_source IS NULL OR deposit_source IN ('cash', 'credit', 'comp'));
```

`worker/src/db/thb_deposits.rs` bound that column through:

```rust
fn deposit_source_str(source: Option<DepositSource>) -> &'static str {
    match source {
        Some(DepositSource::Cash) => "cash",
        …
        None => "",          // ← here
    }
}
```

**`''` is neither `NULL` nor a member of that list.** It fails the CHECK, SQLite
aborts the statement, D1 returns an error, and the handler turns it into a 500.

And `slip_upload.rs` sets **`deposit_source: None`** for every attendee upload —
correctly, because a slip's economic source is not known until it is verified.

So: **every attendee slip upload → `deposit_source = ''` → CHECK violation →
500.** Same for `slip_admin_upload.rs`. Both `insert_thb_deposit` and
`update_thb_deposit` carried the same bind.

**I applied 0047 to production at ~01:00 on 2026-09-23** as part of the deploy
in `docs/deploy_20260923_runbook.md`. Before that the column did not exist and
the previously-deployed worker never named it, so the pair was harmless apart.
The migration and the new worker together are what broke it. That is on me.

## 2. Why nothing caught it

- **The type system can't.** `Option<DepositSource> -> &'static str` is total.
  Every arm returns a `&str`; there is no illegal value to reject.
- **Unit tests can't.** No test ran real SQL against the real constraint. The
  domain tests for `DepositSource` test the *classifier*, not the *bind*.
- **The migration rehearsal didn't.** I replayed all four migrations against a
  copy of the production backup and checked the backfill row by row — but a
  backfill `UPDATE` only writes `'cash'`/`'credit'`/`'comp'`. **It never writes
  the `None` case, because only the application does.** The rehearsal proved
  the migration was safe and said nothing about the code that would write
  through it afterwards.
- **The staging rehearsal didn't.** `.plans/027` §F noted plainly that
  staging's `thb_deposits` was empty, so 0046/0047 there "grouped to zero
  rows". An empty table cannot exercise an INSERT path.

The shape to remember: **a constraint added by a migration is a new way for
existing code to fail, and the migration rehearsal is the wrong instrument for
finding it.** Nothing about replaying a migration exercises the writers that
will later write through it.

## 3. The fix

```rust
fn deposit_source_bind(source: Option<DepositSource>) -> D1Type<'static> {
    match source {
        Some(s) => D1Type::Text(s.as_str()),
        None => D1Type::Null,
    }
}
```

Used by **both** writers. `D1Type::Null` binds correctly on worker 0.8.1 —
`db/audit.rs`, `db/credit_ledger.rs` and `db/attendees/writes.rs` all rely on
it already.

Worth naming the trap: every *other* optional column in this module binds `""`
for absent, and that convention is documented in the module. It is correct for
them, because they are plain `TEXT` with no constraint. `deposit_source` is the
first constrained column in the file, so the house style silently became wrong
for exactly one column. The doc comment on `deposit_source_bind` now says so.

## 4. Verification

### 4.1 The constraint, against real SQLite

```
-- deposit_source = '' (what the code bound when None):
Error: CHECK constraint failed: deposit_source IS NULL OR deposit_source IN ('cash','credit','comp')
-- NULL (what it binds now):
   accepted
```

### 4.2 The real handler, real migrated schema, real request

`wrangler dev --local` with all 50 migrations applied, a real event config in
KV, a seeded attendee, and a genuine `POST /api/deposit/thb/upload`:

```
HTTP 200
{"success":true,"data":{"message":"slip uploaded, awaiting verification"}}

thb_deposits row: amount_thb=500, deposit_source IS NULL, slip_blake3 length 64
```

The slip lands, the source is NULL (not `''`), and the BLAKE3 fingerprint from
`.issues/130` is recorded.

### 4.3 What was NOT proven

**The handler-level A/B was not completed.** Re-running the identical request
against the pre-fix bind kept returning `400 attendee already has a deposit` —
the deposit is cached in KV under several keys and clearing D1 plus the two
obvious KV keys was not enough to reset the fixture. I stopped rather than keep
debugging test scaffolding while an attendee was blocked.

So the causal chain is proven link by link — the CHECK rejects `''` (4.1), the
code bound `''` (read directly), the upload path passes `None` (read directly),
and the fixed code now succeeds end to end (4.2) — but **there is no single
recording of the pre-fix handler returning 500 on the same request.** Stated
plainly rather than glossed.

### 4.4 Guards

`worker/tests/deposit_source_bind_guards.rs`, 4 tests: 0047 still carries the
CHECK; `None` binds `D1Type::Null` and the empty-string arm has not returned;
**both** writers go through the shared helper; and the upload handlers still
pass `None`, so if that ever changes someone re-reads this rather than deleting
the reasoning.

Gates: `clippy --workspace --all-targets -D warnings` exit 0 · 49 workspace
test binaries, 0 failures.

## 5. Impact on real attendees

Anyone who tried to upload a slip between the deploy (~01:00) and this fix got
a 500 and **no record was written** — the INSERT was rejected outright, so
there is no partial or corrupt row to clean up. They simply need to upload
again once this ships.

The owner knows of at least one: transferred that morning, could not upload.
There is no way to enumerate the others from data, because failed inserts leave
nothing behind. Worth a message to the RTM#6 list after deploying.

## 6. Follow-ups

- **`slip_admin_upload.rs`** has the same `deposit_source: None` and the same
  writers, so the organizer's own record-a-slip path was equally broken. Fixed
  by the same change; called out because it is a second user-visible surface.
- **Audit the other optional binds in this module** against any constraint
  added later. Right now `deposit_source` is the only constrained column, so
  the rest are fine — but the next `CHECK` will land in the same trap.
- **A smoke test that actually writes.** The deploy runbook verifies reads
  (health, a ticket payload, Content-Type). Nothing in it writes. A single
  authenticated write against staging would have caught this in seconds.

## Related

- `.issues/130` — `deposit_source` and the slip fingerprint this INSERT carries.
- `docs/deploy_20260923_runbook.md` — the deploy that applied 0047.
- `.plans/027` §F — the staging rehearsal that could not exercise this.

## 7. สรุปภาษาไทย

**อาการ:** อัปสลิปไม่ได้ ขึ้น `500 internal error` — **ทุกคน ทุกครั้ง**

**สาเหตุ — และเป็นความผิดพลาดของผมเองเมื่อเช้านี้**

migration `0047` ที่**ผมเพิ่งรันขึ้น prod ตอนตี 1** เพิ่มเงื่อนไข:
```sql
CHECK (deposit_source IS NULL OR deposit_source IN ('cash','credit','comp'))
```

แต่โค้ดตอนบันทึกส่ง**ค่าว่าง `''`** เมื่อยังไม่รู้ประเภท ซึ่ง **`''` ไม่ใช่ทั้ง NULL และไม่อยู่ในลิสต์**
→ SQLite ปฏิเสธทั้งคำสั่ง → 500

และหน้าอัปสลิปของผู้เข้าร่วม**ส่ง None เสมอ** (ถูกต้องแล้ว เพราะยังไม่รู้ว่าเป็นเงินสด/เครดิต/ฟรี จนกว่าจะตรวจสลิป)
→ **อัปสลิปพังทุกครั้ง** ตั้งแต่วินาทีที่ migration ลง

**ทำไมไม่มีอะไรจับได้:**
- **type system จับไม่ได้** — ฟังก์ชันคืน `&str` ได้ทุกกรณี ไม่มีค่าผิดกฎให้ปฏิเสธ
- **การซ้อม migration จับไม่ได้** — ผมซ้อมกับข้อมูล prod จริงและเช็คทีละแถว แต่ **backfill เขียนแค่ 3 ค่าที่ถูกต้อง ไม่เคยเขียนกรณี None** เพราะกรณีนั้นมีแต่โค้ดแอปที่เขียน
- **staging จับไม่ได้** — ตาราง `thb_deposits` บน staging ว่างเปล่า ตารางว่างทดสอบ INSERT ไม่ได้

**บทเรียน: การเพิ่ม constraint ใน migration = เปิดช่องให้โค้ดเดิมพังแบบใหม่ และการซ้อม migration เป็นเครื่องมือผิดประเภทสำหรับหาบั๊กนี้**

**แก้แล้ว:** เปลี่ยนให้ส่ง `NULL` จริง ๆ แทนค่าว่าง ทั้ง 2 จุดที่เขียนข้อมูล

**ทดสอบ:** พิสูจน์กับ SQLite จริงว่า `''` ถูกปฏิเสธและ NULL ผ่าน · **ยิง API จริงผ่าน handler จริงบน schema ที่ migrate ครบ → HTTP 200** บันทึกสำเร็จ `deposit_source` เป็น NULL

**สิ่งที่ยังพิสูจน์ไม่ครบ (บอกตรง ๆ):** ผม**ไม่ได้**บันทึกภาพ "โค้ดเก่า + request เดิม → 500" ไว้ เพราะ fixture ติด cache ใน KV ล้างไม่หมด ผมหยุดแทนที่จะไล่ debug ต่อ เพราะมีคนจ่ายเงินแล้วอัปสลิปไม่ได้อยู่

**ผลกระทบกับคนจริง:** ใครที่อัปสลิประหว่างตี 1 ถึงตอนนี้ จะได้ 500 และ**ไม่มีข้อมูลถูกบันทึกเลย** (คำสั่งถูกปฏิเสธทั้งอัน ไม่มีเศษข้อมูลค้าง) → **แค่อัปใหม่หลัง deploy ก็จบ** แต่**หาไม่ได้ว่ามีใครบ้าง** เพราะ insert ที่ล้มเหลวไม่ทิ้งร่องรอย → **ควรประกาศในกลุ่ม RTM#6 หลัง deploy**
