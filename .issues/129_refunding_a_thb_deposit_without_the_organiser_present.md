# 129 — Refunding a THB deposit at check-in, with nobody from the team in the room

**Status:** open · design, no code change yet
**Raised:** 2026-09-22 by the owner
**Severity:** high — this is the manual step that gates every event, and the one
that has actually lost money (`.issues/126`, `.issues/127`)

## The ask

> *"ตอนนี้รับเป็นเงินไทยอยู่ แล้วอยากจะคืนเงินเมื่อคนนั้นมา checkin ได้เลย แต่ตัวผมไม่อยู่ตรงนั้นได้ไหม
> ... ปกติก็คือ จะต้องมาดูหลังจบงาน แล้วดูว่าใครเช็คอินมาบ้าง เก็บเป็น credit ไว้อยู่หรือเปล่า คุยก่อนว่าจะเก็บไว้
> หรือคืนเลยดี แล้วตอนคืนก็ต้องเปิดแอปธนาคารมาโอนให้ ... เพราะระบบยังมีเคสที่ คนอัปอะไรก็ได้มาในหน้า deposit
> ถ้าผมดูด้วยตาเปล่า หรือคุยกับคนๆนั้นมาก่อน ก็จะจำเองว่าคนนี้ bypass มาไม่ได้จ่ายเงินจริง"*

## 1. Three problems wearing one coat

The single manual step is three decisions, and they have three different right
answers. Conflating them is *why* somebody has to be present.

| | question | who answers it today | automatable? |
|---|---|---|---|
| **V** | did this person really pay ฿500? | the owner, by eye | **yes — deterministically, and not with AI** |
| **D** | refund now, or keep as credit? | a conversation after the event | **yes — already built, and the attendee decides** |
| **P** | move ฿500 from my bank to theirs | the owner's banking app | **only with a payout rail — this is the real blocker** |

Separate them and two of the three stop needing a human at all.

## 2. What the code does today (verified 2026-09-22)

**V — verification is an eyeball, and approval means two different things.**
`worker/src/handlers/deposit/thb/handlers/slip_verify.rs` takes an admin
approve/reject. Approval also auto-generates the attendee's ticket QR
(`slip_verify.rs:133`, approval-only).

That is the conflation the owner describes. **Approving the slip is the only way
the attendee gets a ticket, and approving is also the promise to refund ฿500.**
So when the owner knowingly waves in someone whose slip is junk, the only action
available records a refundable ฿500 — and the fact that no cash arrived lives in
the owner's head, nowhere in the system.

The right state already exists: `DepositSource::Comp`
(`domain/src/models/deposit.rs:165`, classified in `source()` at `:179`) means
*never cash, never refundable, never holdable as credit*. But it can only be
created **at signup**, and only for staff/organiser/super-admin of that event
(`worker/src/handlers/register/signup.rs:824-827`, sentinels
`STAFF_COMP_WAIVED` / `SYSTEM_STAFF_WAIVE`). There is no admin action that comps
an ordinary attendee whose slip did not hold up. **Gap 1.**

**D — already solved, and already in the attendee's hands.**
`thb/handlers/hold_credit.rs` is attendee-initiated: they choose credit over
cash themselves, and the ledger is the truth. No conversation required.

**P — there is no payout rail.**
`thb/handlers/refund.rs` requires the admin to supply `refund_proof_url` — a
screenshot of a transfer they made by hand in their banking app. The product
records that a refund happened; it has never moved money. **Gap 2.**

## 3. V: the slip is not an unstructured image, so do not point AI at it first

Plan 026 §0 already says AI proposes and the program disposes. For the slip
there is a sharper version of that rule: **a Thai bank slip carries a mini-QR
whose payload is a bank transaction reference**, and services that resolve that
reference against the bank network exist and are cheap
([RDCW Slip Verify](https://slip.rdcw.co.th/),
[EasySlip](https://easyslip.com/api-verified-slip/), both advertise
bank-sourced confirmation and duplicate detection). A screenshot someone made in
an image editor has no reference the bank will confirm.

So the check is deterministic, in this order:

1. The QR decodes to a bank transaction reference — else `needs_review`.
2. The reference is **unique**, as a `UNIQUE` constraint in the schema, not a
   check in code. `.issues/127` is the proof that a uniqueness rule living in
   code and not in the schema does not actually hold.
3. The reference resolves at the bank to: amount == the required deposit,
   receiver == our account, timestamp inside the registration window — else
   `rejected`, with the reason shown to the attendee.

Vision/OCR is the **fallback** for a slip whose QR will not decode, and it
produces a review-queue proposal, never a deposit. This is plan 026 §B with the
order corrected: deterministic first, model only where determinism ran out.

Doing this closes the "upload anything" class, and it removes the owner from
**V** entirely.

## 4. P: the payout, ranked

| | option | owner in the room? | lead time | depends on |
|---|---|---|---|---|
| 1 | **Credit by default at check-in** | no | days | nothing — the machinery is already deployed |
| 2 | Card auth-hold, void on check-in | no | weeks | a gateway + a card-holding audience |
| 3 | Payout API (2C2P mass payout / Opn) | no | weeks–months | a juristic entity and onboarding |
| 4 | Move the money on-chain | no | in-window | §5 |

**Option 1 is the one that pays off this month.** At check-in the deposit
becomes rolling credit automatically; cash-out becomes a *request* the owner
batches later, off the critical path, at a desk instead of at a door. Everything
needed is deployed — `hold_credit.rs`, `credit_ledger`, the wallet UI shipped in
`.issues/120`. What changes is the default and who initiates it.

Options 2 and 3 are business decisions, not engineering ones. 3 is the only real
THB automation, and it needs a company; 2 narrows the audience to card holders
in a PromptPay-first market.

## 5. What Solana solves — and two things it does not

The owner's instinct is right about the shape: **the attendee signs their own
refund.** `bethere-escrow/src/instructions/refund.rs:29` — `pub attendee: Signer`.
The organiser is not a party to the transaction. Nobody needs to be present,
because nobody needs to *act*. That is a structurally better answer than any
amount of THB automation.

Two facts contradict "refund the moment they check in", though, and both were
verified in the source today:

1. **`refund` requires `clock >= event_end`** (`refund.rs:74`). A checked-in
   attendee cannot be refunded at the door — they can refund any time *after*
   the event ends, with no deadline. If "instantly at check-in" is a product
   requirement, that is a **program change**, not a wiring change. The version
   that needs no program change is still good: from the moment the event ends,
   the attendee takes their own money back from their phone, and the owner is
   never in the loop.
2. **`mark_checked_in` demands the organiser's own key**
   (`mark_checked_in.rs:13,15` — `pub organizer: Signer`, `has_one(organizer)`).
   There is no delegated door role. So today, "the owner is not there" means
   "somebody at the door is carrying the key that can also `claim_forfeited` and
   `close_event`". **Gap 3.**

   `EventEscrow` has a 36-byte `_padding` and a `version` byte
   (`bethere-escrow/src/state.rs:20,48`). A 32-byte `checkin_authority`,
   defaulting to the organiser, **fits without resizing the account** — the
   smallest change that makes a delegated door safe.

On-chain state as of today: the program is live on **devnet**
(`C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`, `Executable: true`) and absent
on mainnet. The harness attendee wallet `7ABX2ZyPogms6dvb3f8mTACy3SZUui25DhqYmSY9LSNC`
holds **9.99998 devnet USDC / 4.9966 SOL**, so `.issues/084`'s "blocked on
devnet USDC" is stale.

## 6. Recommended order

1. **Credit-by-default at check-in, and a post-upload comp action** (Gap 1).
   Days, no external dependency. Removes the owner from the room for the common
   case and gives the "they did not really pay" memory a home in the schema.
2. **Slip QR + `UNIQUE` reference + bank verify** (§3). Days, one API key.
   Closes the bypass class and removes the owner from **V**.
3. **`checkin_authority`** (Gap 3), then decide instant-vs-after-event refund.
   Program change; belongs in the hackathon window with plan 026 §A.
4. **Payout rail** (§4 option 3) only if cash-out volume ever justifies it.

## 7. Owner decisions this needs

1. **What does "refund at check-in" have to mean?** Cash in their hand at the
   door, or "from that moment they can take it themselves, without me"? Only the
   second is reachable without changing the on-chain program.
2. **Credit by default** — acceptable, or must cash stay the default?
3. **Slip-verify vendor** — RDCW or EasySlip. Needs an account in the owner's
   name, and the vendor sees slip data (names, account numbers, amounts). A PII
   decision, so it is the owner's.
4. **Is there a juristic entity** that could hold a payout account? The answer
   decides whether §4 option 3 exists at all.

## 8. สรุป (ไทย)

- ปัญหาเดียวที่เล่ามา จริง ๆ เป็น **สามปัญหา**: (V) เช็คว่าจ่ายจริงไหม, (D) เก็บเป็นเครดิตหรือคืนเงิน,
  (P) โอนเงินจริง — แยกออกจากกันแล้ว สองข้อแรกไม่ต้องใช้คนเลย
- **V ไม่ต้องใช้ AI** — สลิปธนาคารไทยมี mini-QR ที่เป็นเลขอ้างอิงธุรกรรม ยิงไปถาม
  RDCW/EasySlip ได้ว่าธนาคารยืนยันไหม รูปที่ตัดต่อมาจะไม่ผ่าน · ใช้ AI อ่านเฉพาะตอน QR เสียเท่านั้น
  และผลที่ได้เป็นแค่ "ข้อเสนอ" เข้าคิวรีวิว ไม่ใช่เงิน
- **บั๊กที่เจอวันนี้:** การกด approve สลิป เป็นทางเดียวที่คนจะได้ QR เข้างาน แต่การ approve
  ก็แปลว่าสัญญาว่าจะคืน ฿500 ด้วย — เคส "รู้ว่าไม่ได้จ่ายจริงแต่ให้เข้างาน" เลยไม่มีที่เก็บในระบบ
  ต้องจำเอง · ระบบมี `DepositSource::Comp` อยู่แล้วแต่สร้างได้ตอนสมัครเท่านั้น
- **P คือคอขวดจริง** — ทางที่ได้ผลเดือนนี้คือ **ตั้งค่าเริ่มต้นเป็นเครดิต** ตอนเช็คอิน
  แล้วให้ "ขอถอนเป็นเงินสด" เป็นคำขอที่ค่อยทำทีหลังรวดเดียว ของพร้อมหมดแล้ว ไม่ต้องต่อ API ธนาคาร
- **Solana ถูกทาง** เพราะ `refund` ให้ **ผู้เข้าร่วมเซ็นเอง** เจ้าของงานไม่ต้องอยู่เลย **แต่**
  (1) โปรแกรมบังคับ `clock >= event_end` — คืนทันทีที่หน้างานยังทำไม่ได้ ต้องแก้โปรแกรม
  (2) `mark_checked_in` ต้องใช้ key ของ organizer เอง — ถ้าไม่ไป ต้องยื่น key ที่ปิดงาน/ริบเงินได้ให้คนอื่น
  ทางแก้: ใส่ `checkin_authority` ลงใน `_padding` 36 ไบต์ที่เหลืออยู่ ไม่ต้องขยาย account
