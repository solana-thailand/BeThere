# Production deploy runbook — 2026-09-23

> ## ✅ EXECUTED 2026-09-23 by Claude, with the owner's explicit go-ahead
>
> | | |
> |---|---|
> | **new production version** | **`05d7df99-f1cd-45af-a4c5-e6cfdbf054d0`** |
> | rollback target (previous) | `f5b99ef0-2843-4352-8c8b-9800020f5c4a` |
> | migrations applied | 0046, 0047, 0049, 0050 — all ✅ |
> | 0047 backfill on prod | cash 40 / credit 14 / comp 3 — **identical to the local rehearsal** |
> | worker bundle | 1,573,991 gzip, 50.03 % of the ceiling |
> | Content-Type | `/` → text/html, `.js` → text/javascript ✅ |
> | **previously-dead tickets** | **render** (2 checked in a real browser, 0 parse errors) |
> | RTM#6 ticket | renders: QR, venue + map link, name, deposit verified |
>
> Rollback, if ever needed:
> `npx wrangler rollback f5b99ef0-2843-4352-8c8b-9800020f5c4a`
>
> **Step 8 was deliberately NOT run — see the hazard note there.**

**Originally prepared for the owner to run; kept as the record of what was
done and why.**

This is not a general runbook — `docs/staging_deploy_runbook.md` is that. This
one is for *this* deploy, with the exact commands, the exact expected output,
and the exact rollback.

---

## Why this deploy matters

Production is on version **`f5b99ef0-2843-4352-8c8b-9800020f5c4a`**, deployed
**2026-09-19 16:47 UTC**. `develop` is **60 commits** ahead. RTM#6 is
**2026-09-27 — four days away.**

**The headline: 476 of 514 attendee tickets are dead in production right now.**
13 of the 14 events with attendees serve `null` for `event_location_map_url`,
and `#[serde(default)]` does not cover an explicit `null`, so the whole ticket
page fails to deserialize (`.issues/133`). RTM#6 itself renders **by accident**
— it is the only event with a map link set. The fix is in this deploy.

Also in it: `.issues/136` (an attendee's name was served as their ticket type;
1 of 516 was wrongly badged VIP, and all 516 saw a name in the Ticket column),
`.issues/132` (ticket announcements), and the A–F queue from `.plans/027`.

---

## What is DONE already

- [x] **Prod D1 backed up** →
      `backups/backup-20260923-pre-0049-0050.sql` (2.2 MB, 32 tables,
      **516 attendee rows**, 16 events, 57 thb_deposits). Verified by loading
      it into sqlite and counting, not by trusting the ✅.
      **It holds PII. `backups/` is now gitignored wholesale** — it previously
      only matched `backup-*.sql` by filename, so an export named anything else
      was one `git add -A` from a public repo.
- [x] Feature branch merged to `develop`, working tree clean.
- [x] Every gate green: `cargo fmt --all --check`, `clippy --workspace
      --all-targets -D warnings` exit 0, **48 workspace test binaries, 0
      failures**, frontend fmt + wasm32 clippy + tests, `shellcheck` 0.11.0
      over 32 scripts.

## What is NOT done, and is yours

- [ ] Everything below.

---

## Step 1 — confirm you are deploying what you think you are

```bash
cd ~/event-checkin
git status --short            # must be empty
git log --oneline -1          # note this SHA; you need it for step 7
cd worker && npx wrangler deployments list | tail -20
```

Expect the current prod version to still be `f5b99ef0…`. If it is not,
**stop** — someone deployed in between and this runbook's baseline is wrong.

## Step 2 — list migrations before applying (never apply blind)

```bash
cd ~/event-checkin/worker
npx wrangler d1 migrations list bethere-db --remote
```

> ### ⚠️ Correction to the plan: it is FOUR migrations, not two
>
> The owner's call was "0049 + 0050 only". **That is not achievable, and it is
> not what should happen.** Checked against production on 2026-09-23, the
> unapplied list is:
>
> | | what it does | additive? |
> |---|---|---|
> | `0046_thb_deposit_slip_hash.sql` | adds `slip_blake3` + index | yes |
> | `0047_thb_deposit_source.sql` | adds `deposit_source` + CHECK + index, **and backfills 57 real rows** | **no — it writes data** |
> | `0049_ticket_announcements.sql` | ticket announcement columns | yes |
> | `0050_attendees_ticket_name.sql` | adds `attendees.ticket_name` | yes |
>
> `.plans/027` recorded 0046/0047 as applied — but that was **staging**.
> Production never got them.
>
> **Two reasons this is not a choice:**
>
> 1. `wrangler d1 migrations apply` has no subset option. It applies every
>    pending migration or none.
> 2. **The Worker code in this deploy depends on both.**
>    `db/thb_deposits.rs` queries `slip_blake3` on the duplicate-slip path
>    (`.issues/130`), and the slip upload and comp handlers write
>    `deposit_source` (`.issues/129` / `.issues/131`). Deploying the new Worker
>    against a schema without those columns breaks slip upload — during the
>    week of the event.
>
> So: **apply all four, or deploy neither the code nor the migrations.**
> Applying all four is the right answer; see the rehearsal below.

If `0048_thb_deposits_unique.sql` appears in that list, **stop**: it lives in
`worker/migrations-pending/` precisely so it cannot be applied by accident. It
rebuilds a table holding 57 real deposit rows and is marked
`DO NOT APPLY BEFORE 2026-09-28`.

### The one that writes data — rehearsed against real production rows

`.plans/027` §F.6 flagged 0047 as "verified against a local fixture, not
against production data". **That gap is now closed.** All four migrations were
applied to a copy of the step-0 production backup (57 real `thb_deposits`
rows), and 0047's backfill was compared against the existing Rust classifier
`ThbDeposit::source()` row by row:

```
classification BEFORE (what the Rust classifier says today)
  cash 40 | credit 14 | comp 3
classification AFTER  (what the migration writes)
  cash 40 | credit 14 | comp 3
rows where the two DISAGREE: 0
```

All four applied cleanly in sequence. Row counts unchanged — attendees 516,
thb_deposits 57, events 16. `PRAGMA integrity_check` → `ok`.

**So 0047 changes how the classification is stored, not what it is.** Every
deposit keeps precisely the classification it has today.

## Step 3 — apply the migrations

```bash
cd ~/event-checkin/worker
CI=true npx wrangler d1 migrations apply bethere-db --remote
```

`CI=true` answers the interactive prompt non-interactively. This applies all
four. Step 0's backup is your undo.

## Step 4 — read the schema back; do not trust the ✅

```bash
npx wrangler d1 execute bethere-db --remote --json --command \
  "SELECT name FROM pragma_table_info('attendees') WHERE name='ticket_name'
   UNION ALL
   SELECT name FROM pragma_table_info('thb_deposits')
    WHERE name IN ('slip_blake3','deposit_source');"
```

Expect **three** rows: `ticket_name`, `slip_blake3`, `deposit_source`. Fewer
means a migration did not take, whatever the previous step printed.

And confirm 0047 wrote the classification it was supposed to — this should
match the rehearsal exactly (cash 40, credit 14, comp 3, allowing for deposits
taken since the backup):

```bash
npx wrangler d1 execute bethere-db --remote --json --command \
  "SELECT deposit_source, COUNT(*) FROM thb_deposits GROUP BY deposit_source;"
```

> **Known snag:** `d1 execute --remote` has been returning error **7403**
> ("account is not authorized") intermittently for days — it failed twice
> during preparation while `d1 export --remote` and `deployments list` both
> worked. If it 7403s here, that is the API, not your migration. Re-run it; if
> it keeps failing, verify via the app in step 7 instead and do not block the
> deploy on it.

## Step 5 — build the frontend, then deploy

```bash
cd ~/event-checkin
(cd frontend-leptos && bash build.sh)   # MUST run from INSIDE frontend-leptos
cd worker
bash deploy.sh --force --reason "ship .issues/133 — 476 of 514 prod tickets dead; plus .issues/136 ticket_name and the .plans/027 A–F queue"
```

**About `--force`:** the preflight gate requires a green 6-flow harness run in
the last hour, and `flow-harness/results/.last-green` has **never existed** —
the gate has not been satisfiable since it landed on 2026-09-11
(`.issues/084`). `worker/scripts/.preflight-bypass.log` records **29**
production deploys, every one of them forced. So this is the normal path here,
not a novel risk; `--reason` is mandatory and is appended to that audit log.
(That log stays untracked and local — deliberately.)

The deploy runs two gates of its own **before** uploading:
- the worker size budget gate — expect roughly **50 %** of the 3 MiB ceiling;
- Content-Type verification after upload.

**If the size gate fails, the deploy aborts and nothing shipped.** That is
working as designed.

## Step 6 — record the new version id immediately

```bash
npx wrangler deployments list | tail -20
```

**Write the new version id down.** Rollback is one command and needs it:

```bash
npx wrangler rollback f5b99ef0-2843-4352-8c8b-9800020f5c4a
```

That is the id you are rolling *back to* — the current production version.
**Note: a rollback reverts the Worker, not the D1 migrations.** 0046, 0049 and
0050 are purely additive, so a rolled-back Worker simply ignores them. **0047
also wrote data** — but only into its own new `deposit_source` column, which no
old code reads, and the rehearsal proved it agrees with the classifier the old
code uses. So a Worker rollback is still safe and complete. If you ever need
the data itself back, step 0's backup is the restore.

## Step 7 — smoke test, both directions

A 200 is not a pass. Check the thing that was broken.

```bash
# 1. the worker is alive
curl -s -o /dev/null -w '%{http_code}\n' https://bethere.solana-thailand.workers.dev/api/health   # 200

# 2. THE FIX. Pick a PAST event's attendee — one that was dead before.
#    Note ?event_id= is REQUIRED: without it the API answers for the ACTIVE
#    event and every probe reads falsely green.
curl -s "https://bethere.solana-thailand.workers.dev/api/public/ticket/$AID?event_id=$EID" \
  | python3 -c "import json,sys; d=json.load(sys.stdin)['data']; print(repr(d['event_location_map_url']))"
# BEFORE: null   → page dies
# AFTER : ''     → page renders
```

Ids for `$AID` / `$EID` are in the backup if `d1 execute` is still 7403ing:

```bash
sqlite3 /tmp/probe.db < ~/event-checkin/backups/backup-20260923-pre-0049-0050.sql
sqlite3 /tmp/probe.db "SELECT event_id, MIN(id) FROM attendees GROUP BY event_id;"
rm -f /tmp/probe.db     # it holds PII
```

```bash
# 3. Content-Type, not just status — a JS asset served as octet-stream is a
#    blank app that still returns 200. This has bitten this project before.
curl -sI https://bethere.solana-thailand.workers.dev/ | rg -i 'content-type'
```

**4. Open a real ticket page in a browser.** `cargo check`, clippy and a
`curl` 200 have all passed on a visibly broken page in this repo before. Look
at it.

## Step 8 — backfill the ticket tiers — ⚠️ DO NOT RUN BEFORE READING THIS

0050 adds the column; it does **not** fill it. The real tiers live in Google
Sheets and no migration can read a spreadsheet, so the sync is the only
backfill:

```
POST /api/events/{event_id}/sync
```

**NOT RUN on 2026-09-23, and this is a correction to an earlier draft of this
runbook, which called it "optional" and safe.** It is neither.

`sync_one_attendee` derives `deposit_status` from the **Google Sheet's** deposit
columns (`derive_deposit_status`, `sync.rs:319`), and `upsert_attendee_full`
writes it **unconditionally**: `deposit_status = excluded.deposit_status`
(`management.rs:212`) — no `COALESCE`, unlike the columns around it.

But **THB deposits are recorded in D1, not in the Sheet.** So the Sheet's
deposit columns lag D1 by construction, and running this sync would overwrite
real verified-deposit state with stale sheet-derived values — on an event with
฿500 refund obligations attached to each row, four days before it happens.

**The trade is bad:** the upside is a cosmetic VIP/Walk-in badge on the admin
list; the downside is corrupting deposit state. Nothing regresses by waiting —
an unsynced event reads an empty tier, which is exactly the behaviour before
0050 existed.

**When to do it:** after RTM#6, or before it only if you have first confirmed
that the Sheet's deposit columns match D1 for that event. Check with:

```bash
npx wrangler d1 execute bethere-db --remote --json --command \
  "SELECT deposit_status, COUNT(*) FROM attendees
    WHERE event_id='<event>' GROUP BY deposit_status;"
```

…and compare against the sheet before syncing. A safer long-term fix is to make
`deposit_status` a `COALESCE`-preserved column in `upsert_attendee_full` like
its neighbours, so the sync can never demote a deposit. Filed as a follow-up in
`.issues/136`.

---

## If it goes wrong

| symptom | do this |
|---|---|
| deploy aborted at the size gate | nothing shipped; read the per-file table it printed |
| deployed but the app is blank | `npx wrangler rollback f5b99ef0-2843-4352-8c8b-9800020f5c4a`, then check Content-Type — a JS asset stuck as `octet-stream` needs a `BUILD_TAG` bump in `frontend-leptos/src/lib.rs`, not a redeploy |
| tickets still dead after deploy | confirm you probed with `?event_id=`; without it you are reading the ACTIVE event |
| `d1 execute --remote` 7403s | API-side, intermittent. `d1 export --remote` was working as a fallback |
| migrations applied, deploy failed | safe — 0046/0049/0050 are additive and 0047 only fills its own new column, which the old Worker does not read |
| slip upload 500s after deploy | 0046/0047 did not apply. Check step 4; the new code needs `slip_blake3` and `deposit_source` |

## What this deploy does NOT include

- `migrations-pending/0048` (`.issues/127`'s UNIQUE) — needs a table rebuild,
  deferred past 2026-09-28 by design. It is the only migration in this repo
  that rebuilds a table, and it is deliberately not in `migrations/`.
- `chore/wrangler-4.135` (`.issues/069`) — held back deliberately: ship the
  payload on the toolchain it was tested on, then move the toolchain.
- Frontend asset precompression (`.issues/135` §6.3) — 380 KB / 21.5 % per
  first load, but its edge behaviour cannot be verified without a staging
  deploy. After RTM#6.
- Any change to `NOTIFICATIONS_ENABLED`. Still `0`. `.issues/128`.

---

## สรุปภาษาไทย

**ทำไมต้อง deploy:** prod ยังเป็นเวอร์ชันวันที่ 19 ก.ย. ช้ากว่า develop **57 commits**
และตอนนี้ **ตั๋วเสีย 476 จาก 514 ใบ** RTM#6 เหลือ **4 วัน**

**ที่ผมทำให้แล้ว:** backup prod D1 เรียบร้อย (2.2 MB, 516 แถว — เช็คด้วยการเปิดไฟล์จริง
ไม่ได้เชื่อเครื่องหมายถูก) · merge เข้า develop แล้ว · gate เขียวหมด ·
**แก้ `.gitignore` ให้กันทั้งโฟลเดอร์ `backups/`** เพราะเดิมกันแค่ชื่อไฟล์ที่ขึ้นต้นด้วย `backup-`
ไฟล์ PII 2.2 MB เกือบหลุดขึ้น repo สาธารณะ

**ที่คุณต้องทำ:** ทำตามขั้นที่ 1–8 ด้านบน

**⚠️ แก้ข้อมูลสำคัญ: เป็น migration 4 ตัว ไม่ใช่ 2 ตัว**

คุณเลือก "เอาแค่ 0049 + 0050" — **แต่ทำแบบนั้นไม่ได้ และไม่ควรทำ** เช็คกับ prod จริงแล้วพบว่า
**`0046` กับ `0047` ยังไม่เคยขึ้น prod เลย** (แผนเดิมจดว่า "ขึ้นแล้ว" — แต่นั่นคือ **staging**)

เหตุผลที่ไม่ใช่ทางเลือก:
1. คำสั่ง `migrations apply` เลือกเฉพาะบางตัวไม่ได้ — ขึ้นทั้งหมดหรือไม่ขึ้นเลย
2. **โค้ดที่กำลังจะ deploy ต้องใช้คอลัมน์จาก 0046/0047** (`slip_blake3` กับ `deposit_source`)
   ถ้า deploy โค้ดใหม่โดยไม่มีคอลัมน์ → **ระบบอัปสลิปพัง** ในสัปดาห์ที่มีงานพอดี

**ตัวที่เขียนข้อมูลจริงคือ 0047 — ผมซ้อมกับข้อมูล prod จริงแล้ว**
(เอา backup มาลองรันทั้ง 4 ตัวในเครื่อง):

```
ก่อน (สิ่งที่โค้ด Rust จัดประเภทอยู่ตอนนี้)  cash 40 | credit 14 | comp 3
หลัง (สิ่งที่ migration เขียนลงไป)           cash 40 | credit 14 | comp 3
แถวที่ไม่ตรงกัน: 0
```

จำนวนแถวไม่เปลี่ยน (attendees 516, thb_deposits 57, events 16) · `integrity_check` = ok
→ **0047 แค่เปลี่ยนวิธีเก็บ ไม่ได้เปลี่ยนผลลัพธ์** ทุกรายการยังถูกจัดประเภทเหมือนเดิมเป๊ะ

**จุดที่ต้องระวังเป็นพิเศษ:**
1. **ขั้นที่ 2** ถ้าเห็น `0048` โผล่มาในรายการ — **หยุดทันที** มันต้อง rebuild ตารางที่มีข้อมูลเงินจริง 57 แถว
2. **ขั้นที่ 5** ต้องรัน `build.sh` ของ frontend **ก่อน** `deploy.sh` เสมอ
3. **`--force` ไม่ใช่เรื่องผิดปกติ** — gate นี้ไม่เคยผ่านได้เลยตั้งแต่สร้างมา และ deploy ที่ผ่านมา
   **29 ครั้งก็ใช้ `--force` ทั้งหมด** แค่ต้องใส่ `--reason` ซึ่งจะถูกบันทึกไว้
4. **ขั้นที่ 7 ต้องใส่ `?event_id=`** ไม่งั้น API จะตอบของงานที่ active อยู่ แล้ว**ดูเขียวทั้งที่ยังพัง**
5. **เปิดหน้าเว็บดูด้วยตาจริง ๆ** — repo นี้เคยผ่าน test หมดแต่หน้าขาวมาแล้ว
6. **rollback ได้ด้วยคำสั่งเดียว** แต่ rollback แค่ตัว Worker ไม่ย้อน migration
   (ไม่เป็นไร เพราะ 0049/0050 แค่เพิ่มคอลัมน์ ของเก่าไม่สนใจมัน)

**⚠️ ขั้นที่ 8 — ผม *ไม่ได้* รัน และขอแก้ที่เคยเขียนไว้ว่า "ทำได้ปลอดภัย" — ไม่ปลอดภัย**

คำสั่ง sync จะอ่าน**สถานะมัดจำจาก Google Sheet** แล้วเขียนทับลง D1 **แบบไม่มีเงื่อนไข**
(`deposit_status = excluded.deposit_status` — ไม่มี COALESCE ต่างจากคอลัมน์ข้าง ๆ)

**แต่มัดจำ THB ถูกบันทึกใน D1 ไม่ใช่ใน Sheet** → ข้อมูลใน Sheet เก่ากว่าเสมอ
ถ้ารัน sync ตอนนี้ **สถานะ "จ่ายมัดจำแล้ว" ของจริงอาจถูกเขียนทับด้วยข้อมูลเก่า**
บนงานที่มีภาระคืนเงิน ฿500 ต่อคน และเหลืออีก 4 วัน

**ได้ไม่คุ้มเสีย** — ได้แค่ป้าย VIP บนหน้า admin แต่เสี่ยงข้อมูลเงิน
**รอได้ ไม่มีอะไรแย่ลง** (ไม่ sync = ช่องบัตรว่าง ซึ่งเหมือนตอนก่อนมี 0050 อยู่แล้ว)

**ควรทำหลังงาน RTM#6** หรือถ้าจะทำก่อน ต้องเช็คก่อนว่า Sheet กับ D1 ตรงกัน
