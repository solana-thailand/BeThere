# 137 — The door roster says "Deposit pending" for people who owe nothing

**Status:** fixed and verified end to end 2026-09-23. Deployed (in prod tag `deploy/production/20260925T032335Z`; `issue_ledger.py` 2026-09-25 found every linked commit there); §6 has the plan.
**Found:** 2026-09-23, owner reported that staff on the admin In-Person menu
still showed "deposit pending".
**Severity:** high for the event itself. No data is wrong; the *screen the
organizer runs the door from* is wrong, and it is wrong in the direction that
makes them chase money nobody owes. RTM#6 is 2026-09-27.

## 1. The report

> "ในหน้า admin เมนู in-person สำหรับ staff ยังขึ้นว่า deposit pending อยู่"

Confirmed against production. On RTM#6:

| `attendees.deposit_status` | `thb_deposits` | who | rows |
|---|---|---|---:|
| `verified` | verified, ฿500 | manually verified by the organizer | 10 |
| **`none`** | **verified, ฿500** | `SYSTEM_ROLLING_CREDIT` | **7** |
| `none` | *no row* | not deposited yet | 3 |
| **`none`** | **verified, ฿0** | `SYSTEM_STAFF_WAIVE` | **2** |

The 2 staff and the 7 credit-covered attendees all have a **verified** deposit
in `thb_deposits` and `deposit_status = 'none'` on the attendee row.

## 2. Two independent defects

### 2.1 A zero is not a deposit

```rust
let has_deposit = attendee.deposit_amount.is_some();
```

`deposit_amount` maps from `attendees.deposit_amount_usdc`, and a staff comp
carries **`0`, not `NULL`** — so `Some("0")` is `is_some()`, `has_deposit` is
true, and the row falls through to "Deposit pending". The same zero makes an
attendee with **no deposit at all** read as pending.

### 2.2 The roster has never read `thb_deposits`

The badge consults exactly two things: `deposit_amount` (USDC) and
`deposit_verified`, which `reads.rs` derives from `attendees.deposit_status`.

**Nothing in the THB flow writes either.** The function that would —
`db::attendees::deposit::save_deposit_status_to_d1` — is marked
`#[allow(dead_code)]` and has **zero callers** anywhere in the repo. So the
entire THB deposit system was invisible to the roster: a staff comp, a
credit-covered registration and an unpaid attendee were indistinguishable.

The 7 credit rows escaped only because a *separate* annotation
(`used_credit`, from the credit ledger) happens to catch them. Staff comps had
nothing.

## 3. The fix

**`worker/src/db/thb_deposits.rs::settlement_by_attendee`** — one batched query
per roster page, in the same shape as the credit-ledger annotations beside it.
Named columns, not `SELECT *`: `slip_url` can hold a multi-megabyte base64 data
URL, and pulling whole rows for a 500-person roster to read three booleans is
the kind of thing that only shows up as a slow page at the door.

It returns `verified` / `refunded` / `source`, where `source` reads the
`deposit_source` column (migration 0047) and falls back to the legacy sentinels
in **the same arm order** as `ThbDeposit::source()` — a row that is both
credit-applied and ฿0 is Credit, not Comp.

**`AttendeeListItem`** gains `thb_source` / `thb_verified` / `thb_refunded`,
annotated best-effort: a failure there must degrade the badge, never the
roster. An organizer at the door needs the list of names far more than the
badge on it.

**`deposit_badge_for`** — pulled out of the row view so it can be tested, and
reordered **settled-first**:

| condition | badge |
|---|---|
| refunded (either source) | `Refunded` |
| `thb_source == comp` | **`Comp ✓`** ← new |
| `thb_source == credit` or `used_credit` | `Credit ✓` |
| `thb_verified` or `deposit_verified` | `Deposit ✓` |
| a slip is in but unverified, **or** a USDC amount > 0 | `Deposit pending` |
| otherwise | *no badge* |

A deposit that is refunded, comped or credit-covered is **done**, and calling
any of them "pending" is the failure this ordering prevents. `Comp ✓` is shown
distinctly from `Deposit ✓` on purpose: what differs between them is whether a
฿500 refund is owed, which is the single fact the organizer needs.

Also added `DepositSource::as_str()` in `domain`, because the SQL column, the
wire field and migration 0047's `CHECK` were three places agreeing on
`"cash"`/`"credit"`/`"comp"` by coincidence. A test asserts it matches the
serde representation, so they cannot drift.

## 4. Verification

### 4.1 The tests fail against the old code

8 truth-table tests in `admin.rs`. Run against the **pre-fix** logic, **6 of 8
fail** — including the reported bug:

```
an_attendee_with_no_deposit_gets_no_badge   left: Some("Deposit pending")  right: None
a_staff_comp_is_not_pending                 FAILED
a_credit_covered_registration_is_not_pending FAILED
refunded_outranks_every_other_state         FAILED
```

The 2 that pass are the USDC and online paths, which this change deliberately
leaves alone — they are the regression guards.

### 4.2 End to end, five states, a real worker

Seeded into a local D1 with migrations applied and queried through the real
`/api/attendees` endpoint:

```
cash-ok    thb_source='cash'   verified=True  -> Deposit ✓
cash-pend  thb_source='cash'   verified=False -> Deposit pending
credit-1   thb_source='credit' verified=True  -> Credit ✓
nothing    thb_source=None     verified=False -> (no badge)
staff-1    thb_source='comp'   verified=True  -> Comp ✓
```

### 4.3 Rendered in a browser, on the actual screen

The In-Person roster (`/admin` → Attendance → In-Person, `Alt+6`):

```
Credit Person    -> ["In-Person","Pending","Credit ✓","credit@ex.com"]
No Deposit       -> ["In-Person","Pending","none@ex.com"]
Paid Person      -> ["In-Person","Pending","Deposit ✓","cash@ex.com"]
Pending Person   -> ["In-Person","Pending","Deposit pending","pend@ex.com"]
Staff Person     -> ["In-Person","Pending","Comp ✓","staff@ex.com"]
```

("Pending" in each row is the **check-in** status badge, a different control.)

### 4.4 Gates

`cargo clippy --workspace --all-targets -D warnings` exit 0 · 48 workspace test
binaries, 0 failures · frontend `fmt --check`, wasm32 `clippy -D warnings`,
host tests all green · shellcheck clean.

## 4.5 Follow-up the same day: the signal I removed

Shipping §3 removed a badge the owner was relying on, and they spotted it
within hours: **an attendee who has submitted nothing showed no badge at all.**

That state was previously covered *by accident*. `deposit_amount` is
`Some("0")` for nearly every row, so the old `is_some()` check painted
everyone "Deposit pending" — wrong for comps and credit users, but right often
enough that it doubled as "who still owes money". Removing the false positive
removed the accident, and the true signal with it.

The gap was real: **there was never a badge for "in-person, deposit required,
nothing submitted"** — which is the single most actionable row on the screen.

Added: **`No slip yet`** (`badge-danger`), gated on the event's
`deposit_enabled` so an event without deposits accuses nobody.

**Why that wording.** Not "Not paid" or "Unpaid": the system knows a *slip* is
absent, not that the *money* is. `.issues/138` is exactly that case — people
had transferred and could not upload, and a roster calling them unpaid would
have been both wrong and accusatory on the same day.

The two action-needed states are now distinct, because the actions differ:

| badge | who is it waiting on |
|---|---|
| `Deposit pending` | **the organizer** — a slip is in, nobody has checked it |
| `No slip yet` | **the attendee** — nothing has been submitted |

Rendered, all four states on one roster:

```
Paid Person     -> Deposit ✓
Slip Sent       -> Deposit pending
Staff Person    -> Comp ✓
Unpaid Person   -> No slip yet
```

Tests grew from 8 to 10, including one asserting the two action states never
collapse into the same label.

## 5. What this does NOT fix

- **`save_deposit_status_to_d1` is still dead**, and `attendees.deposit_status`
  is still never written by the THB flow. The roster now routes around it
  rather than depending on it. Whether that column should be maintained or
  deleted is a separate decision — it is also what `refund_status` derives
  from, so THB refunds have the same blind spot (covered here by
  `thb_refunded`, not by fixing the root).
- **It does not let anyone be marked exempt in advance.** See §7.

## 6. Deploying it

Code-only — **no migration**. It is additive on the wire (three new
`#[serde(default)]` fields) so an old frontend against a new worker simply
ignores them, and a new frontend against an old worker sees them absent and
falls back to exactly today's behaviour. Safe in either order.

Production is on `05d7df99-f1cd-45af-a4c5-e6cfdbf054d0` (deployed earlier
today). This needs a second deploy to reach the door screen before Saturday.

## 7. The related thing this does NOT solve: marking a VIP exempt

There is **no way to exempt someone from the deposit in advance.** What exists:

- **"Admit, no refund owed"** — real and working, but it lives on the deposit
  admin page and acts on a *slip*, so it only appears for people who have
  already uploaded one. It cannot pre-exempt.
- **The staff list** — anyone `is_staff()` gets an automatic ฿0 comp at signup
  (`record_staff_comp`). It works, but it also grants staff privileges, so it
  is the wrong tool for a guest.

The clean answer is to let a **ticket tier** skip the deposit gate, now that
`attendees.ticket_name` exists (migration 0050, `.issues/136`). That is blocked
on the tier column actually being populated, which is blocked on the sync
hazard in `.issues/136` §6.6. Filed as a follow-up, not built here.

## Related

- `.issues/136` — the ticket tier column, and why the sync that would fill it
  is not safe to run.
- `.issues/131` — the comp action this badge finally makes visible.
- `.issues/130` — `deposit_source`, which this reads.

## 8. สรุปภาษาไทย

**อาการที่คุณเจอ:** หน้า admin เมนู In-Person ขึ้น "Deposit pending" ให้ staff ที่ไม่ต้องจ่าย

**เช็คกับ prod แล้ว งาน RTM#6:** staff **2 คน** และคนใช้ credit **7 คน** มีมัดจำที่ verified แล้วจริงในตาราง `thb_deposits` แต่ในตาราง `attendees` เป็น `deposit_status='none'`

**บั๊ก 2 จุดแยกกัน:**

1. **เลข 0 ถูกนับว่า "มีมัดจำ"** — staff comp เก็บค่าเป็น `0` (ไม่ใช่ค่าว่าง) โค้ดเช็คแค่ `is_some()` → เลยตกไปที่ "pending" · คนที่**ไม่มีมัดจำเลย**ก็โดนด้วย
2. **หน้า roster ไม่เคยอ่านตาราง `thb_deposits` เลย** — อ่านแค่ `deposit_status` กับยอด USDC ซึ่ง**ฝั่ง THB ไม่เคยเขียนทั้งคู่** (ฟังก์ชันที่ควรเขียนเป็น dead code ไม่มีใครเรียก) → ระบบมัดจำ THB ทั้งระบบมองไม่เห็นจากหน้านี้

**แก้แล้ว** เพิ่มการอ่าน `thb_deposits` เข้ามา (query เดียวต่อหน้า) แล้วเรียงลำดับป้ายใหม่แบบ **"จบแล้วมาก่อน"** — คืนเงินแล้ว / comp / ใช้ credit ถือว่า**จบ** ไม่ใช่ pending

**ป้ายใหม่ `Comp ✓`** แยกจาก `Deposit ✓` ตั้งใจให้ต่างกัน เพราะสิ่งที่ต่างคือ**ต้องคืน ฿500 ไหม** ซึ่งเป็นข้อมูลเดียวที่คุณต้องรู้หน้างาน

**ทดสอบแล้ว 3 ชั้น:** เทส 8 ตัว (**รันกับโค้ดเก่าแล้วพัง 6 ตัว** = เทสจับบั๊กได้จริง) · ยิง API จริง 5 สถานะถูกหมด · **เปิดหน้า admin ในเบราว์เซอร์จริง** เห็นป้ายถูกต้องครบ

**ตามมาอีกเรื่องในวันเดียวกัน:** หลัง deploy เจ้าของสังเกตว่า **คนที่ยังไม่ส่งสลิปเลย ไม่มีป้ายขึ้นเลย**

เดิมสถานะนี้ถูกครอบด้วย**ความบังเอิญ** — เพราะ `deposit_amount` เป็น `Some("0")` เกือบทุกแถว
ทุกคนเลยขึ้น "Deposit pending" หมด ผิดสำหรับ staff/credit แต่**บังเอิญถูกพอที่จะใช้ดูว่าใครยังไม่จ่าย**
พอแก้ของผิดออก ของที่บังเอิญถูกก็หายไปด้วย

**ความจริงคือไม่เคยมีป้ายสำหรับ "ต้องมัดจำ แต่ยังไม่ส่งอะไรมาเลย"** ซึ่งเป็นแถวที่สำคัญที่สุดบนหน้าจอ

**เพิ่มป้าย `No slip yet`** (สีแดง) และผูกกับ `deposit_enabled` ของงาน — งานที่ไม่เก็บมัดจำจะไม่ขึ้นป้ายนี้กับใคร

**ทำไมใช้คำนี้** ไม่ใช้ "ยังไม่จ่าย" เพราะ**ระบบรู้แค่ว่าไม่มีสลิป ไม่ได้รู้ว่าไม่มีเงิน**
— `.issues/138` คือกรณีนั้นเป๊ะ ๆ คนโอนเงินมาแล้วแต่อัปโหลดไม่ได้ ถ้าขึ้นว่า "ยังไม่จ่าย" จะทั้งผิดและกล่าวหาเขา

**ตอนนี้แยกชัด 2 สถานะที่ต้องลงมือต่างกัน:**

| ป้าย | รอใคร |
|---|---|
| `Deposit pending` | **รอคุณตรวจ** — สลิปเข้ามาแล้ว ยังไม่มีใครดู |
| `No slip yet` | **รอผู้เข้าร่วม** — ยังไม่ส่งอะไรมาเลย |

ทดสอบในเบราว์เซอร์จริง เห็นครบ 4 สถานะแยกกันชัดเจน · เทสเพิ่มจาก 8 เป็น 10 ตัว

**⚠️ เรื่อง VIP ยังแก้ไม่ได้ด้วยอันนี้** — ยังไม่มีวิธี "ยกเว้นล่วงหน้า" ให้ใคร ต้องรอให้ข้อมูลประเภทบัตรเข้าระบบก่อน ซึ่งติดปัญหา sync อยู่ (`.issues/136` §6.6)
