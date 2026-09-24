# 139 — The VIP waiver is blind when the event resolves from D1

**Status:** **fixed on `develop`, not deployed** (§6). The feature itself is
verified working (§2). Filed 2026-09-23 while verifying `f041baf`.
**Severity:** low today, but it fails in the expensive direction — a guest the
organizer promised would not pay is asked for ฿500.

## 1. The hole

`EventConfig::comp_emails` lives in **KV**. `register::signup` resolves the
event with `event_store::resolve_event_by_slug`, which tries KV first and then
falls back to D1:

```rust
// worker/src/event_store/read.rs
if let Some(kv) = events_kv { … return Ok(config); }   // KV: has comp_emails
if let Some(db) = d1 { … return Ok(row.to_event_config()); }  // D1: has NOT
```

`D1EventRow::to_event_config()` sets `comp_emails: Vec::new()` — because the
`events` table has no such column. So on a KV miss the waiver list is silently
empty and **every VIP is charged**.

**This is the same shape as `.issues/136`**, found and fixed hours earlier: a
D1 read path that cannot represent a field and substitutes something rather
than saying "not known". There it invented the attendee's name; here it invents
an empty guest list. The difference is that an empty list looks completely
ordinary.

## 2. How it was found, and what the feature does when KV is present

The guard tests and the four `apply_update` unit tests all passed while the
waiver did not fire at all — because the test fixture had the event only in D1.
That is the same lesson as `.issues/138`: **the unit tests and the write path
were verified separately, and only exercising the real endpoint found the
gap.**

With the KV index and config in place, a real `POST /api/public/register`
against a real worker, two identities, one on the list and one not:

| email | next step | `thb_deposits` row |
|---|---|---|
| `vip@example.com` (listed) | **`/ticket/…`** | **฿0, verified, `SYSTEM_STAFF_WAIVE`, `deposit_source='comp'`** |
| `normal@example.com` | `/deposit/…` | none |

So the feature is correct on the path production actually uses — events created
through the API are in the KV index. Three fixture mistakes were needed to get
there (event only in D1, then no KV index entry, then an **online** event where
nobody owes a deposit at all), and each one produced a plausible-looking
"feature does not work" result.

## 3. Why it is not fixed here

The fix is a `comp_emails` column on `events` plus a migration — and RTM#6 is
four days away with a production deploy already behind us today that caused an
outage. Adding a fifth migration to chase a KV-miss path is the wrong trade
this week.

## 4. What to do instead

- **Before RTM#6:** nothing. The KV path works; events in the index resolve
  from KV.
- **After:** either add the column, or make the D1 fallback refuse to answer
  for deposit-waiver purposes rather than returning an empty list. The second
  is smaller and arguably more honest: D1 genuinely does not know.
- **Either way**, the conversion site already carries a comment saying an empty
  vec means "unknown from this source, never nobody is waived". A comment does
  not stop the charge; it just stops the next reader from being surprised.

## Related

- `.issues/136` — the same read-path-invents-a-value shape, in `ticket_name`.
- `.issues/138` — the same verify-the-write-path lesson.

## 6. Fix (2026-09-23, same day) — no migration

§3 assumed the fix needed a column. It does not. The KV **index** is what
misses; the KV **config** is written by id on every admin save
(`handlers/events/update.rs` and `event_store::write::update_event` both call
`save_event_config` unconditionally, but only *update* an index entry that
already exists). So an event that is missing from the index but was ever
saved from the admin form still has its `comp_emails` in KV at `event:{id}`.

`event_store::read::with_kv_only_fields` reads that key after a D1 resolve and
copies `comp_emails` across. It runs in the D1 branch of both
`resolve_event_by_slug` (registration) and `resolve_event_or_fallback`. The KV
path pays nothing; the D1 fallback pays one KV read. A read failure keeps the
old behaviour (empty, logged) rather than failing registration for everyone.

**Verified A/B on a local worker** (`wrangler dev --local`, fresh state): event
in D1 only, list saved through `PUT /api/events/vip139` (this writes the KV
config and, correctly, no index entry), then `POST /api/public/register` as the
listed email:

| build | log | listed VIP's next step |
|---|---|---|
| pre-fix | `resolved event by slug from D1` | **`/deposit/…`** (charged) |
| fix | `resolved event by slug from D1` | **`/ticket/…`**, ฿0 `comp` row |
| fix, unlisted email | same | `/deposit/…` |

Same state, same email; only the code differed. Wrangler's custom build
recompiles on start, so the pre-fix run had the change stashed for the whole
server lifetime, not just the `cargo build`.

**Still not covered:** an event with no KV config at all. That event has never
been saved from the admin form with KV bound, so no list exists anywhere; the
empty list is then the truth, not a substitution.

## 5. สรุปภาษาไทย

**ช่องโหว่:** รายชื่อ VIP เก็บอยู่ใน **KV** · ตอนลงทะเบียน ระบบหาข้อมูลงานจาก KV ก่อน
ถ้าไม่เจอจะไปเอาจาก **D1** ซึ่ง**ไม่มีคอลัมน์นี้** → ได้ลิสต์ว่างเปล่า → **VIP ทุกคนโดนเก็บเงิน**

**เป็นรูปแบบเดียวกับ `.issues/136`** ที่เพิ่งแก้ไปเมื่อเช้า — ทางอ่านจาก D1 ที่เก็บข้อมูลนั้นไม่ได้
แล้ว "เดา" ค่าแทนที่จะบอกว่า "ไม่รู้" · ต่างกันตรงที่ลิสต์ว่างดูเหมือนปกติมาก

**เจอได้ยังไง:** เทสทั้งหมดผ่าน แต่ฟีเจอร์ไม่ทำงานเลยตอนยิง API จริง —
บทเรียนเดียวกับ `.issues/138` คือ**ต้องทดสอบทางเขียนจริง ไม่ใช่แค่ unit test**

**ผลทดสอบจริง (เมื่อมีข้อมูลใน KV ครบ):**

| อีเมล | ไปหน้าไหน | บันทึกมัดจำ |
|---|---|---|
| `vip@example.com` (อยู่ในลิสต์) | **หน้าตั๋ว** | **฿0 verified เป็น comp** |
| `normal@example.com` | หน้ามัดจำ | ไม่มี |

→ **ฟีเจอร์ถูกต้องบนเส้นทางที่ production ใช้จริง** (งานที่สร้างผ่าน API จะอยู่ใน KV index เสมอ)

**ทำไมยังไม่แก้:** ต้องเพิ่มคอลัมน์ใน D1 + migration อีกตัว · เหลือ 4 วันถึงงาน
และวันนี้ deploy ไปแล้วรอบนึงจนเกิดปัญหา — ไม่คุ้มที่จะเพิ่ม migration ตัวที่ 5 เพื่อไล่เคส KV miss

**ควรทำหลังงาน:** เพิ่มคอลัมน์ หรือให้ทาง D1 ปฏิเสธที่จะตอบเรื่องการยกเว้นมัดจำไปเลย
แทนที่จะคืนลิสต์ว่าง (อย่างหลังเล็กกว่าและซื่อตรงกว่า เพราะ D1 ไม่รู้จริง ๆ)

**อัปเดต (แก้แล้ว ไม่ต้องใช้ migration):** ที่ miss คือ *index* ใน KV ไม่ใช่ตัว config —
หน้าแอดมินเขียน config ลง KV ตาม id ทุกครั้งที่บันทึก · ตอนนี้ถ้า resolve จาก D1
ระบบจะอ่าน `event:{id}` จาก KV มาเติม `comp_emails` · ทดสอบ A/B แล้ว: โค้ดเก่า VIP ไปหน้ามัดจำ,
โค้ดใหม่ไปหน้าตั๋ว (฿0 comp) · **อยู่บน develop ยังไม่ deploy**
