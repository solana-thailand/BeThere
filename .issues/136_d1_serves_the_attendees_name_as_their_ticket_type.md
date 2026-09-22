# 136 — D1 serves an attendee's own name as their ticket type, so "Vipada" is a VIP

**Status:** **fully fixed.** Cause found, reproduced, wrong information removed,
and the real ticket tier restored via migration `0050` — built and verified
end to end 2026-09-23. **Not deployed**; see §6.4.
**Found:** 2026-09-23, from the owner's report that someone who registered came
out with VIP status.
**Severity:** medium-high. No data loss and nothing is destroyed, but it is
wrong information on the screen the organizer runs the door from, it is
**not** an edge case, and it lands on real people by their name.
**Branch:** `feature/134-slip-qr-bank-reference` (unpushed).

## 1. The report

> "มีคนลงทะเบียนเข้ามาแล้วเป็นสถานะเป็น VIP เลยไม่แน่ใจว่ามีบัคตรงไหนไหม"

There is. It is not in registration.

## 2. What actually happens

`worker/src/db/attendees/reads.rs`, in **both** D1 → `Attendee` conversions:

```rust
name:        self.name.clone().unwrap_or_default(),
ticket_name: self.name.clone().unwrap_or_default(),   // ← the bug
```

**The `attendees` table has no `ticket_name` column at all.** The struct's own
doc comment says fields absent from D1 are "filled with defaults" — and
`first_name` and `last_name` directly above do exactly that (`String::new()`).
`ticket_name` instead got a copy of the attendee's **name**.

The admin list then decides who is a VIP like this
(`frontend-leptos/src/pages/admin.rs:91`):

```rust
fn is_vip_ticket(ticket_name: &str) -> bool {
    ticket_name.to_lowercase().contains("vip")
}
```

So any attendee whose **name** contains `vip`, case-insensitively, is rendered
with a `VIP` badge and matches the VIP filter pill. That is not a contrived
string: **Vipada (วิภาดา), Vipawee (วิภาวี), Vipawadee (วิภาวดี), Vipa (วิภา)**
are ordinary Thai given names, and Vipul is an ordinary Indian one.

## 3. Why it is the normal path, not an outage path

This is the part that makes it worth fixing rather than noting.

`worker/src/handlers/attendee/list.rs` carried this comment:

```rust
// 1. Fetch sheet-based attendees (D1 fallback when Sheets is unavailable/rate-limited)
```

**That has the direction backwards.** `sheets::get_attendees_for_event` →
`get_attendees_inner` is **D1-FIRST**: it queries D1 and only falls through to
Google Sheets when D1 returns *nothing* or errors. `get_attendee_by_id` is
D1-first too, and says so in its own doc ("Phase 2b: tries D1 first").

So for every event that has attendees in D1 — all of them — the attendee list,
the ticket page and the scanner are all served from D1, and all of them have
been showing the attendee's name in the ticket-type slot. The comment is why
this could look like it only mattered during a Sheets outage. It has been
corrected in the same commit.

Affected surfaces, all of them normal traffic:

| surface | what it shows |
|---|---|
| admin attendee list | `VIP` badge + VIP filter pill, keyed on the person's name |
| admin attendee list | the `Walk-in` badge never fires — it tests `eq_ignore_ascii_case("Walk-in")`, and nobody is named Walk-in |
| admin CSV export | the `Ticket` column contains names |
| ticket page | a "Ticket" row showing the holder their own name |
| scanner | a badge with the attendee's name beside their name |

Registration itself is innocent: both sheet-append paths write a real value
(`"Self-Registered"`, and `"Walk-in"` for walk-ins). The Sheet is correct. Only
the D1 read invented this.

## 4. Reproduction

Two tests in `worker/src/db/attendees/reads.rs`, written before the fix and
confirmed failing against the old code:

```
a_d1_attendee_does_not_get_their_name_as_a_ticket_type ... FAILED
  left: "Vipada Srisuk"
 right: "Vipada Srisuk"
an_ordinary_thai_name_does_not_become_a_vip_badge ... FAILED
  "Vipada Srisuk" must not read as a VIP ticket
```

The second one asserts the *exact* predicate `admin.rs:is_vip_ticket` applies,
over four real name shapes, so it fails for the reason the organizer would see
rather than for an internal one.

### 4.1 Runtime A/B against the running worker

The unit tests prove the conversion. This proves the *symptom*, on the two
endpoints the organizer actually reads, against a real `wrangler dev --local`
worker with one seeded row (`name = 'Vipada Srisuk'`, no ticket type anywhere)
— old build and fixed build, same request, same row:

| | admin list `ticket_name` | `is_vip` | ticket page `ticket_name` |
|---|---|---|---|
| **before** | `'Vipada Srisuk'` | **true** | `'Vipada Srisuk'` |
| **after** | `''` | false | `''` |

Run both ways deliberately, not just the green one: a probe that has never been
seen to fail is not evidence ([[fail-open-controls-need-format-ab]]). The "old"
column is a real rebuild of the pre-fix code, not a recollection.

### 4.2 The page was opened, not just curled

This repo has a history of pages that pass `cargo check`, clippy and a
`curl` 200 while rendering nothing ([[verify-frontend-by-opening-it]]), and
this fix *removes* a field the ticket page renders — so it was rendered:

```
HTTP status      : 200
rendered chars   : 364
contains "VIP"   : false
ticket-info-rows : ["NAME\nVipada Srisuk","EMAIL\nv***@example.com","TYPE\nin_person"]
console errors   : only the Cloudflare RUM beacon failing CORS on localhost
```

The layout degrades cleanly: `in_person_view.rs` guards the row with
`if !ticket_name.is_empty()`, so the "Ticket" row is **omitted**, not rendered
as an empty row with a dangling label. Email is masked in the render, which is
worth noting as working correctly rather than assumed.

**Not reproduced against production.** `wrangler d1 execute --remote` is
returning the recurring **7403** ("account not authorized") again, so the
"how many real attendees are affected" count could not be taken. The query to
run when it clears — it reads no names, only a count:

```bash
npx wrangler d1 execute bethere-db --remote --json --command \
  "SELECT COUNT(*) AS total,
          SUM(CASE WHEN LOWER(name) LIKE '%vip%' THEN 1 ELSE 0 END) AS name_reads_as_vip
     FROM attendees;"
```

## 5. What was fixed

`ticket_name: String::new()` at both conversion sites — the honest value, and
the one `first_name`/`last_name` beside it already use, and the one
`register/tests.rs` and `escrow/status.rs` already use when they synthesize an
`Attendee`.

Both sites, deliberately: they are the same defect copied, and a fix landing on
one of a pair while the sibling keeps the bug is a recurring shape in this repo
([[duplicated-state-transition-paths]]).

**This removed wrong information without restoring right information** — the
Ticket row and both badges rendered empty for every D1-served attendee, which
is everyone. Not a regression (what they showed was a name, so the VIP filter
was already returning the wrong people and the Walk-in badge already never
fired), but the organizer lost a filter that looked like it worked.

**§6 restores it.** This section is kept as the record of the two-step: the
mitigation is independently correct and could ship alone if the migration in §6
is judged too much this close to RTM#6.

## 6. Restoring the real ticket tier — built

The first fix removed wrong information without restoring right information.
This is the rest of it.

### 6.1 The column

`worker/migrations/0050_attendees_ticket_name.sql` —
`ALTER TABLE attendees ADD COLUMN ticket_name TEXT;`

**Nullable, no default, deliberately.** Three states have to stay
distinguishable:

| value | meaning |
|---|---|
| `NULL` | nobody has told us — a row written before 0050, or one no writer should invent a tier for |
| `'Self-Registered'` / `'Walk-in'` | this system minted it |
| anything else | the organizer's own tier — `VIP`, `Speaker`, `Sponsor`. **This is the case that was lost.** |

A `NOT NULL DEFAULT ''` would collapse "never set" into "set to nothing", and
the backfill could then no longer tell which rows it still had to visit.

SQLite `ADD COLUMN` is O(1) metadata, so this is safe to apply days before
RTM#6 — unlike `.issues/127`'s `UNIQUE`, which needs a table rebuild and is why
that one sits in `migrations-pending/0048`.

### 6.2 The writers, and why they stopped being able to disagree

The two ticket names this system mints were **bare string literals repeated
across four files**. That is the root cause, not a detail: nothing named them,
so nothing could check that the Sheets writer and the D1 writer for the *same
flow* agreed. They now live in
`domain/src/models/attendee/ticket_name.rs` as `TICKET_NAME_SELF_REGISTERED`
and `TICKET_NAME_WALK_IN`, and every writer binds the constant.

| writer | ticket_name | why |
|---|---|---|
| `db::attendees::upsert_attendee` (self-registration) | `TICKET_NAME_SELF_REGISTERED` | matches the Sheets append in the same flow |
| `db::attendees::try_insert_walkin` | `TICKET_NAME_WALK_IN` | matches `sheets::write::append`; the admin Walk-in badge compares against this exact string |
| `db::attendees::upsert_attendee_full` (sheet sync) | passed through from the sheet row | **this is the backfill** |
| `db::attendees::upsert_post_event_attendee` | **nothing, on purpose** | post-event registrants are leads, excluded from capacity and check-in, and never receive a ticket. Inventing a tier would badge a row that is not an attendee. A guard test pins this down so it reads as a decision. |

On conflict, `ticket_name` is **never overwritten** by the self-registration
path and is `COALESCE`d on the sync path. A repeat registration must not demote
someone the organizer has since promoted to `VIP`, and a sync of a sheet with
no ticket column must not blank a tier that was already recorded.

### 6.3 The backfill is the sync, because SQL cannot reach a spreadsheet

The real tiers live in Google Sheets. No migration can read them. So
`sync_one_attendee` now carries `a.ticket_name` into `upsert_attendee_full`,
and **running `POST /api/events/{id}/sync` for an event is the backfill.**

Until that is run for an event, its old rows read `NULL` → empty — which is
exactly the post-first-fix behaviour, and still strictly better than the name
they used to show. Nothing regresses by waiting.

### 6.4 What is still owner-gated

**The deploy.** Nothing here is in production. This adds a **third** pending
migration to the deploy `.plans/027` §F.6 already describes (0049 must apply,
`migrations-pending/0048` must not before 2026-09-28), four days before RTM#6.
0050 is the cheapest of the three — one O(1) metadata change, no rebuild, no
backfill inside the migration — but the decision to deploy at all is yours.

**Running the sync per event** after deploying, to backfill. It reads the
sheet, so it is an external call, and which events to sync is your call.

### 6.5 Still worth deciding: `contains("vip")` is a weak test

Even with a correct `ticket_name`, `frontend-leptos/src/pages/admin.rs`
flags a VIP with a substring match. A tier called `VIP Companion` and one
called `Non-VIP` both match. Not changed here — it needs to compare against the
event's actual ticket types, and that is a product decision about what tiers
exist. `domain::is_system_ticket_name` is the start of the vocabulary it would
need.

## 7. Verification

### 7.1 The first fix

- Two unit tests in `reads.rs`, written before the fix and confirmed failing.
- Runtime A/B on both affected endpoints, old build vs fixed build (§4.1).
- The ticket page rendered in a real browser (§4.2).

### 7.2 The full fix — three tier states, end to end

Migration applied to a clean local D1, three attendees seeded, real
`wrangler dev --local` worker, both endpoints:

```
=== admin list (D1-first) ===
  name='Vipada Srisuk'  ticket_name=''         VIP=False Walk-in=False
  name='Somchai P.'     ticket_name='VIP'      VIP=True  Walk-in=False
  name='Walk In Person' ticket_name='Walk-in'  VIP=False Walk-in=True
```

Three things that all had to be true at once, and now are:

1. **Vipada is not a VIP** — the reported bug.
2. **Somchai is** — the feature that the first fix removed is back. Without
   this row the "fix" would just be the blanking, passing quietly.
3. **The Walk-in badge fires** — it never could before. It compares
   `eq_ignore_ascii_case("Walk-in")` against a field that held a person's name,
   so it has been dead since D1 became the read path.

And rendered, not curled:

```
real VIP tier  status=200 rows=["NAME\nSomchai P.","EMAIL\nr***@example.com","TICKET\nVIP","TYPE\nin_person"]
no tier        status=200 rows=["NAME\nVipada Srisuk","EMAIL\nv***@example.com","TYPE\nin_person"]
pageerrors=none
```

The `TICKET / VIP` row appears when there is a tier and the row is **omitted**
when there is not — not rendered as an empty row with a dangling label.

### 7.3 Gates

- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0.
- `cargo test --workspace` — 0 failures.
- `worker/tests/ticket_name_guards.rs` — 6 guards, two of which were confirmed
  failing before the code that satisfies them existed.
- `migration 0050` applied cleanly to a fresh store alongside all 47 others.
- `shellcheck` clean.

**Not verified:** the walk-in and self-registration *write* paths were not
fired live. Both call Google Sheets, and `wrangler dev --local` does **not**
sandbox that — `.dev.vars` holds live service-account credentials, so firing
them would really write to a real spreadsheet
([[local-d1-verification-harness]]). They are covered by the guard tests and by
the compiler, not by a live run. Also unverified: the production row count, as
§4 notes — `wrangler d1 --remote` is still returning 7403.

## Related

- `.issues/133` — the other bug found on the ticket page this week; same
  surface, different cause.
- `[[duplicated-state-transition-paths]]` — why both conversion sites were
  fixed together.

## 8. สรุปภาษาไทย

**มีบั๊กจริง และไม่ได้อยู่ที่หน้าลงทะเบียน**

**สาเหตุ:** ตาราง `attendees` ใน D1 **ไม่มีคอลัมน์ `ticket_name` เลย** แต่โค้ดตอนอ่านข้อมูลเอา
**ชื่อคน** มาใส่ในช่อง "ประเภทบัตร" แทน แล้วหน้า admin ตัดสินว่าใครเป็น VIP ด้วยการเช็คว่า
"ประเภทบัตร" มีคำว่า `vip` อยู่ไหม

→ **ใครก็ตามที่ชื่อมีคำว่า "vip" จะกลายเป็น VIP ทันที**
เช่น **วิภาดา (Vipada), วิภาวี (Vipawee), วิภาวดี (Vipawadee), วิภา (Vipa)** — ชื่อไทยธรรมดาทั้งนั้น

**และมันไม่ใช่เคสหายาก:** คอมเมนต์ในโค้ดเขียนไว้ว่า "ใช้ D1 เป็นตัวสำรองเวลา Sheets ล่ม" —
**ซึ่งกลับหัวกลับหาง** ของจริงคือ **อ่าน D1 ก่อนเสมอ** แล้วค่อยไป Sheets ถ้า D1 ไม่มีข้อมูล
แปลว่าหน้า admin, หน้าตั๋ว และหน้าสแกน **แสดงชื่อคนเป็นประเภทบัตรมาตลอด** (แก้คอมเมนต์แล้ว)

**แก้แล้ว:** เปลี่ยนเป็นค่าว่าง ทั้ง 2 จุด (เป็นบั๊กเดียวกันที่ถูกก๊อปไว้ 2 ที่)
พร้อมเทส 2 ตัวที่**ยืนยันว่าพังจริงก่อนแก้**

**ทดสอบจริงกับ worker ที่รันอยู่** (seed คนชื่อ "Vipada Srisuk" เข้าไป 1 คน):

| | หน้า admin `ticket_name` | ขึ้น VIP ไหม | หน้าตั๋ว |
|---|---|---|---|
| **ก่อนแก้** | `'Vipada Srisuk'` | **ขึ้น** | `'Vipada Srisuk'` |
| **หลังแก้** | `''` | ไม่ขึ้น | `''` |

และ**เปิดหน้าตั๋วในเบราว์เซอร์จริง**ด้วย — หน้าแสดงผลปกติ ไม่มีคำว่า VIP
และแถว "Ticket" **หายไปทั้งแถว** (ไม่ได้เหลือแถวว่าง ๆ ค้างไว้)

**แต่ยังไม่ได้ข้อมูลที่ถูกต้องกลับมา:** ตอนนี้ช่อง Ticket และป้าย VIP/Walk-in จะว่างเปล่า
— ซึ่ง**ดีกว่าแสดงผิด** แต่คุณจะเสียตัวกรอง VIP ไป

**ทำต่อจนจบแล้ว — เอาประเภทบัตรจริงกลับมา (migration 0050):**

- **เพิ่มคอลัมน์ `ticket_name` ใน D1** แบบ nullable (แยกให้ออกระหว่าง "ยังไม่มีใครบอก"
  กับ "ตั้งเป็นค่าว่าง" — ถ้าใช้ `NOT NULL DEFAULT ''` จะแยกไม่ออก แล้ว backfill จะไม่รู้ว่าต้องเติมแถวไหน)
- **ชื่อบัตรที่ระบบสร้างเอง** (`Self-Registered`, `Walk-in`) เคยเป็น**ข้อความดิบซ้ำกันอยู่ 4 ไฟล์**
  — นี่คือต้นตอจริง เพราะไม่มีชื่อเรียก เลยไม่มีใครเช็คได้ว่าฝั่ง Sheet กับฝั่ง D1 เขียนตรงกันไหม
  ตอนนี้ย้ายไปเป็นค่าคงที่ใน `domain` แล้ว + มีเทสบังคับว่าทั้งสองฝั่งต้องใช้ตัวเดียวกัน
- **backfill = รันคำสั่ง sync** เพราะข้อมูลจริงอยู่ใน Google Sheet ซึ่ง SQL เอื้อมไม่ถึง
- **ไม่เขียนทับตอน conflict** — คนที่คุณเลื่อนเป็น VIP แล้ว ลงทะเบียนซ้ำจะไม่ถูกลดกลับ

**ทดสอบจริงครบ 3 กรณีพร้อมกัน:**

| | `ticket_name` | ขึ้น VIP | ขึ้น Walk-in |
|---|---|---|---|
| วิภาดา (ไม่มีบัตรระบุ) | `''` | **ไม่ขึ้น** ✅ | ไม่ขึ้น |
| สมชาย (บัตร VIP จริง) | `'VIP'` | **ขึ้น** ✅ | ไม่ขึ้น |
| คน walk-in | `'Walk-in'` | ไม่ขึ้น | **ขึ้น** ✅ |

ข้อ 2 สำคัญมาก — ถ้าไม่ทดสอบ การ "แก้" ก็จะเป็นแค่การลบทิ้งทั้งหมดแล้วดูเหมือนผ่าน
และข้อ 3 คือป้าย Walk-in ที่**ไม่เคยทำงานเลย**ตั้งแต่เปลี่ยนมาอ่าน D1 ตอนนี้กลับมาแล้ว
เปิดหน้าตั๋วในเบราว์เซอร์ด้วย — แถว `TICKET / VIP` ขึ้นเมื่อมีบัตร และ**หายไปทั้งแถว**เมื่อไม่มี

**ที่ยังต้องให้คุณตัดสิน:**
1. **การ deploy** — ยังไม่ขึ้น prod เลย อันนี้จะเป็น migration ตัวที่ **3** ที่ค้างอยู่ใน deploy
   (0050 เบาที่สุดในสามตัว: แค่เพิ่มคอลัมน์ ไม่ต้อง rebuild ตาราง ไม่มี backfill ในตัว migration)
2. **รัน sync ของแต่ละงาน** หลัง deploy เพื่อ backfill — มันอ่าน Google Sheet จริง คุณเลือกว่างานไหนบ้าง
3. **`contains("vip")` ยังอ่อนอยู่** — บัตรชื่อ "Non-VIP" ก็จะเข้าเงื่อนไข ควรเทียบตรงตัวกับรายการบัตรของงาน

**หมายเหตุ:** ถึงจะแก้คอลัมน์แล้ว การเช็คด้วย `contains("vip")` ก็ยังอ่อนอยู่ดี —
บัตรชื่อ "Non-VIP" ก็จะเข้าเงื่อนไข ควรเทียบแบบตรงตัวกับรายการประเภทบัตรของงาน

**ยังตรวจไม่ได้:** นับจำนวนคนที่โดนจริงบน prod ไม่ได้ เพราะ `wrangler d1 --remote` ติด error
**7403** อยู่ (คำสั่งที่ต้องรันไว้ในข้อ 4 แล้ว — นับอย่างเดียว ไม่ดึงชื่อใคร)
