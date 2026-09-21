# 130 — A payment slip could be submitted twice and nothing noticed

**Status:** shipped in `report` mode 2026-09-22, not yet deployed.
**Found:** 2026-09-22, implementing `.issues/129` §C.
**Severity:** medium-high — it is a ฿500 refund obligation per occurrence, and the
only control was the organizer's memory.

## 1. What was true before

`POST /api/deposit/thb/upload` accepted any JPEG/PNG/WebP under 5 MB and stored
it. Nothing about the image was checked — not the amount, not the payee, not
whether the same image had already been submitted by somebody else.

That is the mechanism behind a thing the owner described on 2026-09-21: they
re-check each person's details at refund time because *anyone can upload any
image to the deposit page*, and they remember by eye who bypassed. There was no
field, flag or record anywhere in the system holding that knowledge. It existed
only in their head, and it had to be re-derived at every refund.

It compounds with `.issues/129` Gap 1: approving a slip is the **only** way an
attendee receives their ticket QR (`slip_verify.rs:133`), and approving is
simultaneously the promise to refund ฿500. So an organizer who suspected a slip
had two choices — admit the person and owe them ฿500, or refuse them entry.

## 2. What now happens

Migration `0046` adds `thb_deposits.slip_blake3`: BLAKE3 of the **decoded image
bytes**, lowercase hex.

- Both upload writers record it — the attendee path (`slip_upload.rs`) and the
  admin path (`slip_admin_upload.rs`). A guard test holds both in place; a hash
  recorded on only one path leaves a hole the size of the other handler.
- It is taken **before** `maybe_upload_to_r2`, while the bytes are still in the
  request. Afterwards `slip_url` is a per-attendee storage path, so a hash taken
  later would be unique by construction and would match nothing, forever. Also
  guarded.
- `GET /api/deposit/thb/pending` returns `duplicate_slip_hashes` — fingerprints
  carried by two or more *distinct* attendees, computed over every deposit for
  the event including already-approved ones, so a new slip matching an approved
  one is still flagged.
- The admin screen shows a warning on the flagged row, above the Approve button,
  naming the consequence: *approving is what promises the refund*.

## 3. What it does not do

Worth stating plainly, because the flag must not be read as "verified":

- **It does not prove a payment happened.** A convincing forgery with no bank
  record passes. Only the transaction reference in the slip's mini-QR answers
  that, and resolving it needs a vendor account in the owner's name
  (`.issues/129` §7.3, owner-gated).
- **It does not catch a re-screenshot.** One different byte is a different hash.
  A perceptual hash would catch it and would also produce false positives on two
  genuine transfers of the same amount from the same bank app. A false positive
  blocks a paying attendee at the door, so exactness is the right first trade.
- **It says nothing about the 243 slips already uploaded.** They have no hash
  and will never have one unless somebody re-reads them out of R2. `NULL` means
  *not known*, never *not a duplicate* — four tests exist to keep that reading.

## 4. Why it ships in `report` mode

`THB_SLIP_DUPLICATE_MODE` is `report` in both environments:

| value | attendee upload | admin upload |
|---|---|---|
| `off` | hash, store, do not compare | same |
| `report` **(default)** | hash, store, record collision, **allow** | same |
| `reject` | **refuse the upload** | still allows — see below |

`report` first because this check can turn a paying attendee away at the door,
and RTM #6 is on 2026-09-27. It earns `reject` by running over a real event and
producing no false positives — not before. Anything unrecognised also parses as
`report`, so a typo in the variable name can neither silently disable the check
nor silently start rejecting people.

The admin path never rejects, in any mode. An organizer uploading on someone's
behalf is making a judgement with context the hash does not have, and blocking
them mid-event is worse than the duplicate. That asymmetry is a test, not a
comment, so nobody "fixes" it by accident.

## 5. Verification

Unit: 5 cases on the fingerprint (same bytes through different data URLs hash
alike; one byte apart does not; not constant; non-uploads yield `None`; mode
parsing). 5 on the duplicate grouping (two attendees one image → reported; one
attendee two rows → not; unhashed rows never match each other; unhashed rows
neither mask nor join a real collision; empty event). 6 source guards on the
wiring (both writers record; both hash before R2; the SQL keeps all four
narrowing clauses; attendee rejection is mode-gated; admin never rejects; the
migration stays additive and non-UNIQUE).

Runtime: see §6 — pending.

## 6. Remaining

- [ ] Run the four-case behavioural check against `wrangler dev --local`
      (upload as A → accept; same bytes as B → flagged; same bytes as A again →
      accept; one-pixel-different as B → accept).
- [ ] Deploy to staging, then production, with the migration.
- [ ] After one real event in `report` mode with no false positives, decide
      `reject`.
- [ ] `.issues/129` Gap 1 — the comp action, so a suspected slip has an outcome
      other than "admit and owe ฿500" or "refuse entry". **This issue is only
      half a fix without it.**

## Related

- `.issues/129` — the design this came out of; §7 holds the owner decisions.
- `.issues/127` — the `UNIQUE (event_id, attendee_id)` that is still missing,
  deferred to after RTM #6. Different column, same lesson: a constraint in code
  is not a constraint.
- `.plans/027` item C.

## 7. สรุปภาษาไทย

- **ปัญหา:** หน้าอัปสลิปรับรูปอะไรก็ได้ ไม่มีการตรวจอะไรเลย คนละคนส่งรูปเดียวกันได้
  และระบบไม่รู้ — คนที่รู้คือเจ้าของงานที่จำหน้าคนเอา
- **ที่ทำ:** เก็บ **BLAKE3 ของไบต์รูป** ลง `thb_deposits.slip_blake3` (migration 0046)
  ทั้งทางที่ผู้เข้าร่วมอัปเองและทางที่แอดมินอัปแทน · หน้าแอดมินจะขึ้นเตือนสีเหลืองบนแถวนั้น
  ว่า "รูปนี้ซ้ำกับของคนอื่นในงานนี้ — เช็คก่อนกด Approve เพราะกด Approve = สัญญาคืนเงิน"
- **โหมดเริ่มต้นคือ `report`** (เตือนอย่างเดียว ไม่บล็อกใคร) เพราะ RTM#6 วันที่ 27 ก.ย. ใกล้แล้ว
  ถ้าพลาดจะกลายเป็นกันคนที่จ่ายจริงเข้างาน · จะเปลี่ยนเป็น `reject` ต่อเมื่อผ่านงานจริง 1 งานแล้วไม่มี false positive
- **สิ่งที่ยังทำไม่ได้:** พิสูจน์ว่าจ่ายเงินจริงไหม (ต้องอ่าน QR ในสลิป + ต่อ API ธนาคาร — รอคุณตัดสินใจ)
  และจับไม่ได้ถ้าเขาแคปหน้าจอใหม่ (ไบต์เปลี่ยน = แฮชเปลี่ยน)
