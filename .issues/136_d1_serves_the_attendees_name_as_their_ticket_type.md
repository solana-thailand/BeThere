# 136 — D1 serves an attendee's own name as their ticket type, so "Vipada" is a VIP

**Status:** cause found, reproduced, and the misinformation is fixed. The real
ticket type is **still missing** and needs a D1 column — owner's call, see §6.
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

**This removes wrong information. It does not restore right information.** The
Ticket row and both badges now render empty for D1-served attendees — which is
every attendee. That is not a regression: what they showed before was a name,
so the VIP filter was already returning the wrong people and the Walk-in badge
already never fired. Blank is strictly more truthful than wrong. But the
organizer does lose a filter that looked like it worked.

## 6. What it needs to actually work — owner's call

Restoring the real ticket type means **D1 needs the column**:

1. Migration `0050`: `ALTER TABLE attendees ADD COLUMN ticket_name TEXT;`
2. Populate it at **four** INSERT sites — `db/attendees/writes.rs` (×2),
   `db/attendees/management.rs`, `db/attendees/walkin.rs` — plus the
   Sheets→D1 sync, or the column is empty for everyone already registered.
3. Backfill the existing rows from the Sheet.
4. Read it: `ticket_name: self.ticket_name.clone().unwrap_or_default()`.
5. A guard that every writer sets it, because four writers is exactly the
   shape where one gets missed.

**Not started, on purpose.** RTM#6 is 2026-09-27, four days out, and
`.plans/027` §F.6 already has a production deploy pending with two migrations
in it (0049 must apply, 0048 is `DO NOT APPLY BEFORE 2026-09-28`) behind a
preflight gate that `.issues/084` says cannot currently be satisfied. Adding a
third migration and a backfill to that deploy, unattended, days before the
event, is not a call to make without the owner. The wrong information is gone
either way; this is about getting the right information back.

**Separately worth deciding:** `contains("vip")` is a weak test even against a
correct `ticket_name` — a ticket type called "VIP Companion" and one called
"Non-VIP" both match. If a real ticket type ever contains the word, it should
be an exact comparison against the event's ticket types, not a substring.

## 7. Verification

- Both new tests pass after the fix; both failed before it, with the message a
  reader needs.
- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0.
- `cargo test --workspace` — 47 test binaries, 0 failures.
- Checked that no other consumer relies on the old behaviour: the only other
  places that build an `Attendee` without a sheet row
  (`register/tests.rs:127`, `escrow/status.rs:497`) already use
  `String::new()`, so the fix matches the convention rather than inventing one.
- Runtime A/B on both affected endpoints, old build vs fixed build (§4.1), and
  the ticket page rendered in a real browser (§4.2).

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

**ที่ต้องให้คุณตัดสิน:** ถ้าจะให้ประเภทบัตรกลับมาถูกต้อง ต้อง**เพิ่มคอลัมน์ใน D1** (migration 0050)
+ แก้จุดเขียน 4 จุด + backfill ข้อมูลเก่า — **ผมยังไม่ทำ** เพราะ RTM#6 เหลือ 4 วัน
และตอนนี้มี migration ค้างอยู่แล้ว 2 ตัวใน deploy ที่ยังไม่ได้ขึ้น

**หมายเหตุ:** ถึงจะแก้คอลัมน์แล้ว การเช็คด้วย `contains("vip")` ก็ยังอ่อนอยู่ดี —
บัตรชื่อ "Non-VIP" ก็จะเข้าเงื่อนไข ควรเทียบแบบตรงตัวกับรายการประเภทบัตรของงาน

**ยังตรวจไม่ได้:** นับจำนวนคนที่โดนจริงบน prod ไม่ได้ เพราะ `wrangler d1 --remote` ติด error
**7403** อยู่ (คำสั่งที่ต้องรันไว้ในข้อ 4 แล้ว — นับอย่างเดียว ไม่ดึงชื่อใคร)
