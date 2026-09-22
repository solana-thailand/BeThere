# 134 — Slip QR decoding fits the bundle but not the CPU budget

**Status:** measured and decided 2026-09-22. Worker-side decode **rejected**;
the string parser is **kept and shipped** (`domain::slip_verify`).
**Found:** executing `.plans/027` item G, which required a size measurement
before any feature work.
**Severity:** none as a defect — this is a design decision with the numbers
attached. It blocks the real fix for `.issues/130`, which stays blocked.
**Branch:** `feature/134-slip-qr-bank-reference` (unpushed).

## 1. The question item G asked

`.issues/130` shipped BLAKE3 slip fingerprinting and said so plainly: hashing
bytes catches a forwarded image and nothing else. A convincing forgery passes.
The only thing on a Thai slip a forger cannot invent is the **bank transaction
reference** carried in its mini-QR.

`.plans/027` §G says decoding that QR needs no vendor and no owner decision —
only bundle size — and set a stopping condition: if `rqrr` + `image` pushes the
gzip bundle past item A's 70 % warn line, stop and write up the alternative.

So the measurement was taken. **Size passes. CPU does not.**

## 2. Bundle size — PASSES

`scripts/verify/worker_size_budget.sh`, which gzips wrangler's own
`deploy --dry-run` bundle (the bytes actually uploaded):

| | gzip bytes | % of 3 MiB free-plan ceiling |
|---|---:|---:|
| before | 1,574,137 | 50.04 % |
| with `image` (jpeg+png+webp) + `rqrr` | 1,831,331 | 58.21 % |
| **delta** | **+257,210** | **+8.17 pp** |

Warn line 2,202,009 (70 %), fail line 2,673,868 (85 %). The addition lands
370,678 bytes below the warn line. **The stopping condition the plan named did
not fire.**

The probe was wired into `slip_upload.rs` rather than left as an uncalled
function on purpose: LLVM dead-strips code nothing reaches, so measuring an
unlinked decoder would have reported roughly zero and read as a free win. This
is the same class of trap as `[[false-clean-probes-from-shell-aliases]]` — a
measurement that cannot fail is not a measurement.

`webp` is included because `validate_slip_url` accepts `image/webp`. A decoder
that silently could not read a format the upload path admits would be a control
that goes inert on a subset of real traffic.

## 3. CPU — FAILS, and this is the blocker

Cloudflare's free plan allows **10 ms CPU per request** (`.plans/010` §P0.2,
which also notes this has never been measured for any existing path).

Release profile, native, Apple M5 Pro — a *lower bound*, since wasm is slower
than native and the edge is slower than this laptop:

| slip image | total | JPEG decode | QR grid detection | vs 10 ms |
|---|---:|---:|---:|---:|
| 1080×1920 phone screenshot | **33.14 ms** | 3.66 | 29.48 | **3.3× over** |
| 828×1792 phone screenshot | 19.36 ms | 1.89 | 17.47 | 1.9× over |
| 720×1280 downscaled | 11.83 ms | 1.06 | 10.76 | 1.2× over |
| 500×500 cropped to just the QR | 4.36 ms | 0.41 | 3.95 | under |

Reproduce: `git checkout 36b8c02` then
`cargo test --release -p event-checkin-worker --lib slip_qr -- --nocapture`.
(Run it in `--release`. The same probe in the default debug profile reports
228 ms for the first row — a 7× exaggeration that would have made this look
far more hopeless than it is.)

**Three things the split makes clear:**

1. **89 % of the cost is QR grid detection, not image decode.** So "decode a
   smaller image" is not the escape hatch it looks like. Shrinking helps the
   3.66 ms half and barely touches the 29.48 ms half, and shrinking past the
   QR's module size leaves no QR to find. The 500×500 row is only cheap because
   it is *cropped to the QR* — which the server cannot do, because finding the
   QR is the expensive step it is trying to avoid.
2. **The realistic case is the failing one.** A slip is almost always a
   full-resolution phone screenshot. The rows that pass are the ones nobody
   uploads.
3. **It gets worse, not better.** Newer phones screenshot larger. A control
   whose cost scales with the attendee's handset is one that fails first for
   the attendees with the best phones.

The `MAX_PIXELS` cap in the probe does not rescue this: 1080×1920 is 2.07 MP,
well under a 4 MP cap, and still 3.3× over budget. The cap only prevents the
tail from being *catastrophically* over.

**If shipped, the failure mode is Cloudflare error 1102 on
`POST /api/deposit/thb/upload`** — not a degraded check, an attendee who cannot
submit a payment slip at all. That is strictly worse than the status quo, where
the check simply does not exist.

## 4. Decision

- **Rejected:** decoding slip QRs inside the worker, on the current plan.
  Committed as a probe (`36b8c02`) and reverted (`65e68f9`) so the measurement
  stays reproducible rather than becoming a claim in a document.
- **Kept and shipped:** `domain::slip_verify` (`d0c0a9c`) — payload →
  `SlipReference`, CRC-verified, no image dependency, no measurable size. Both
  published wire formats are covered by vectors with real CRCs: the bank
  variant and the TrueMoney variant (different sub-layout under the same root
  tag, lowercase CRC, no country tag).

The parser is worth having now regardless of where the decoder eventually
lives, because **whoever decodes the image, the server must re-parse what it is
handed.** A reference supplied by a client is a claim, not a fact.

Four properties the parser's tests pin down deliberately:

- The **CRC is verified before anything is returned.** QR error correction can
  return a *plausible wrong string* from a blurry photograph, and a wrong
  reference written into a `UNIQUE` column is worse than no reference: it burns
  the slot the real slip needs, and the person it locks out is the one who
  actually paid.
- **A PromptPay payment QR is refused.** It is valid EMVCo TLV and people
  photograph it at least as often as the slip; without this it would parse as
  "some reference" and pollute the column.
- **Trailing bytes are an error, not a stopping point** — otherwise a corrupt
  tail is silently truncated to whatever parsed before it.
- **Non-ASCII is refused up front**, which is what makes the byte indexing in
  the TLV walk safe rather than accidentally-safe.

`SlipReference::storage_key()` joins the bank code to the reference, because a
transaction reference is only unique *within* its issuing bank. That composite
is what a schema-level `UNIQUE` goes on — `.issues/127` is the record of what a
code-level uniqueness check costs when it is the only thing holding the
invariant.

## 5. What this leaves blocked

`.issues/130` §3 stands unchanged: the system still cannot tell a forgery from
a real slip, only a *re-used image* from a new one. Item G's premise — that the
QR half was ungated because it needed no vendor and no owner decision — turns
out to be half right. It needs no vendor. It does need a decision, just not the
one the plan expected.

## 6. The three ways this comes back

None of these are engineering calls, which is why nothing further was built.

1. **Workers Paid ($5/mo)** raises CPU from 10 ms to 30 s and the bundle
   ceiling from 3 MiB to 10 MiB, and both problems evaporate. `.plans/010`
   records that the owner "explicitly committed to free tier through Demo Day",
   so this is a standing decision, not an oversight. It is also the cheapest
   fix on this page by a wide margin.
2. **Decode in the frontend.** `frontend-leptos` is already wasm, the decode
   runs on the attendee's own CPU under no budget at all, and the browser can
   downscale and crop via canvas far more cheaply than `rqrr` can search. The
   worker then re-parses the submitted payload with the same
   `domain::slip_verify` code and stores the reference under a schema-level
   `UNIQUE`. **Cost:** ~257 KB moves to the frontend bundle, which has no gate
   on it today (`.plans/027` item A only measures the worker) — so this needs a
   frontend budget first, or it repeats exactly the blind-growth problem item A
   was written to end. **Caveat:** a client-decoded reference is attacker-
   controlled. It still raises the bar (forwarding a friend's slip now needs
   deliberate tampering, not a long-press), and the `UNIQUE` still holds, but
   it is not proof and must not be described as proof in any organizer-facing
   text.
3. **A second Worker** doing nothing but decode, called from the first. Each
   Worker gets its own CPU budget. But 33 ms of work does not fit a 10 ms
   budget by being moved — it would need the same paid plan, plus a second
   deploy target, plus a trust boundary between them. Listed for completeness;
   it is the worst of the three.

**Recommended:** 2, gated on adding a frontend size budget — but it should be
built only once the owner has ruled on (1), because if the plan moves to paid
the server-side decode is both simpler and trustworthy, and option 2's work is
wasted.

## 7. Verification

- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, with and
  without the probe.
- `cargo test --workspace` — 47 test binaries, 0 failures.
- `domain::slip_verify` — 7 unit tests, including both published payload
  vectors with their real CRCs, a single-digit tamper, a PromptPay payment QR,
  malformed inputs, and the CRC-16/CCITT-FALSE published check value (`0x29B1`
  for `"123456789"`), which guards the digest against being reimplemented with
  a reflected polynomial or the wrong init — both of which produce a
  self-consistent CRC that would reject every real slip.
- The probe's own correctness test recovers the exact payload from a
  screenshot-sized JPEG, not from a bare lossless QR.
- `bash scripts/verify/worker_size_budget.sh` — re-run after the revert:
  1,574,034 bytes gzip, **50.03 %**, delta **−87** against the committed
  baseline. The worker carries none of this.

**Not verified:** anything against a real bank slip. Every image in this work
is synthetic. Decode *rates* on real slips — compression artefacts, dark mode,
cropped screenshots, photographs of a screen — are unmeasured, and a poor rate
would weaken option 2 further. That measurement needs real slips, which are
attendee PII sitting in R2, and reading them is the owner's call.

## Related

- `.issues/130` — the fingerprint stopgap this was meant to replace.
- `.issues/127` — why the `UNIQUE` must be schema-level.
- `.issues/129` §7 — the vendor decision for actually resolving a reference.
- `.plans/027` §G — the item, and its size-only stopping condition.
- `.plans/010` §P0.2 — the 10 ms CPU cap, and the note that it was never
  measured. It has now been measured, for one path.

## 8. สรุปภาษาไทย

**คำถาม:** อ่าน QR เล็ก ๆ บนสลิปโอนเงินเพื่อเอา "เลขอ้างอิงธุรกรรม" ได้ไหม — นี่คือสิ่งเดียวบนสลิปที่ปลอมไม่ได้

**คำตอบ: ขนาดไฟล์ผ่าน แต่ CPU ไม่ผ่าน**

- **ขนาด bundle: ผ่าน** เพิ่ม 257 KB → 58.21% ของเพดาน 3 MiB (เส้นเตือนอยู่ที่ 70%)
- **CPU: ไม่ผ่าน** สกรีนช็อตมือถือ 1080×1920 ใช้ **33 ms** แต่ free plan ให้แค่ **10 ms** ต่อ request
  — และนี่คือวัดบน M5 Pro แบบ native ซึ่ง *เร็วกว่า* เครื่องจริงของ Cloudflare
- **89% ของเวลาคือการ "หา" QR ไม่ใช่การถอดรูป** → ย่อรูปก่อนไม่ช่วย เพราะย่อจนเล็กเกินก็ไม่เหลือ QR ให้หา
- **ถ้าฝืนขึ้น prod:** จะได้ error 1102 ตอนอัปโหลดสลิป = คนจ่ายเงินแล้วอัปสลิปไม่ได้เลย แย่กว่าไม่ทำ

**สิ่งที่เก็บไว้ใช้จริง:** `domain::slip_verify` — ตัวแปลงข้อความ QR เป็นเลขอ้างอิง (ตรวจ CRC ด้วย)
ไม่กินขนาดไฟล์ และยังไงฝั่ง server ก็ต้องแปลงซ้ำเองอยู่ดี เพราะเลขที่ client ส่งมา **เชื่อไม่ได้**

**ทางออก 3 ทาง (ต้องให้คุณตัดสิน ไม่ใช่เรื่อง engineering):**
1. **อัปเป็น Workers Paid ($5/เดือน)** — CPU 10 ms → 30 s, เพดาน 3 → 10 MiB, จบทั้งสองปัญหา
   (แต่คุณเคยบอกว่าจะอยู่ free tier จนถึง Demo Day)
2. **ถอด QR ที่ฝั่งหน้าเว็บแทน** (แนะนำ) — ใช้ CPU ของผู้ใช้เอง ไม่มีลิมิต
   แต่ต้องทำ "งบขนาดไฟล์ฝั่ง frontend" ก่อน และเลขที่ได้มาจาก client ยัง**ไม่ใช่หลักฐาน**
3. **แยกเป็น Worker ตัวที่สอง** — แย่ที่สุด ย้ายงาน 33 ms ไปอีกที่ที่ให้ 10 ms เท่ากัน ไม่ช่วยอะไร

**ยังไม่ได้ทดสอบ:** ยังไม่เคยลองกับสลิปจริงสักใบ รูปทั้งหมดเป็นรูปสังเคราะห์ —
สลิปจริงเป็นข้อมูลส่วนตัวของผู้เข้าร่วม ต้องให้คุณอนุญาตก่อน
