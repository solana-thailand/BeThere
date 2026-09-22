# 135 — Nothing measured the frontend bundle, and it is the bigger half

**Status:** gate built, wired into CI, baseline recorded — 2026-09-22.
**Found:** while writing up `.issues/134`, whose recommended path (move the QR
decoder to the frontend) could not be costed because there was no number to
cost it against.
**Severity:** medium. No outage; a blind spot in the thing attendees wait for.
**Branch:** `feature/134-slip-qr-bank-reference` (unpushed).

## 1. What was true before

`.plans/027` item A built a size gate for the worker: `deploy.sh` refuses an
over-budget bundle, CI reds a PR, and `worker/.size-budget` carries a committed
baseline so growth is visible per commit.

The frontend had none of that. Nothing in the repo measured it, CI built it
(the `e2e` job runs `trunk build`) and threw the number away, and `deploy.sh`
shipped whatever came out.

That matters more than it sounds, because **the frontend is the bigger half**:

| | compressed | what it is |
|---|---:|---|
| worker bundle | 1,574,034 gzip | runs at the edge, measured since 2026-09-22 |
| frontend first load | **1,765,890 br4** | **downloaded by every attendee, unmeasured** |

Measured against the deployed site, not inferred: the wasm arrives with
`content-encoding: br` at **1,675,993 bytes**.

## 2. What "first load" means here

index.html plus every local asset it references — **37 files**. Not a
worst-case estimate: all **22 stylesheets are `<link rel=stylesheet>` in the
document head**, so they are render-blocking and every visitor pays for all of
them. An attendee opening a ticket downloads the admin, scanner, dashboard and
quiz sheets before their ticket renders.

That is a finding, not a design. It is not fixed here — `.plans/027` §G's
lesson was that measuring first and deciding second is the right order — but it
is the obvious first place to look for headroom, and the gate's warn text now
says so.

## 3. The gate

`scripts/verify/frontend_size_budget.sh` + `frontend-leptos/.size-budget`,
wired into CI's `e2e` job (the only job that builds the SPA).

Two things it does differently from the worker gate, both deliberate:

### 3.1 It measures brotli at quality 4, and that number was earned

Cloudflare serves these assets `br`, so gzip is not what anybody waits for. But
brotli defaults to quality 11, and Cloudflare compresses **on the fly at
roughly q4**. The same wasm:

| | bytes |
|---|---:|
| raw | 5,478,875 |
| gzip -9 | 1,864,525 |
| **brotli q4** | **1,646,742** |
| brotli q11 | 1,288,797 |
| **actually served by Cloudflare** | **1,675,993** |

q4 lands within 2 % of what is served. **q11 would have understated the whole
bundle by ~28 % and still called itself a measurement** — the same shape of
error as `[[false-clean-probes-from-shell-aliases]]`, where the probe runs,
returns a number, and the number is about something else.

q11 is printed anyway, as a second column, because the gap is actionable:
**precompressing the assets at build time would save 380,390 bytes per first
load — 21.5 % — for no code change.** Not done here; it is a deploy-path change
and `.plans/027` §F.6 already has a production deploy pending five days before
RTM#6. Filed as the first item in §6 below.

### 3.2 The primary threshold is growth, not a ceiling

There is no Cloudflare limit on the frontend. Inventing one and calling it a
limit would be dishonest, so the budget file says plainly that `CEILING_BYTES`
= 2 MiB is a *product* budget — roughly 5.6 s of download on the 3 Mbps a
crowded venue gives you, before a pixel renders — and that raising it is
allowed but raising it quietly is what the file prevents.

The mechanism that actually catches things is the delta against the committed
baseline: **warn at +25 KiB, fail at +100 KiB**, cleared by re-running with
`--update-baseline` and saying in the commit what grew.

100 KiB is not arbitrary. `.issues/134` option 2 is **+257,210 bytes**. It must
trip this gate loudly rather than landing as a 15 % bundle increase nobody
costed — and a guard test asserts `MAX_GROWTH_BYTES` stays below that figure,
so the allowance cannot later be widened until it swallows the decision.

## 4. Current reading

```
first load : 1765890 bytes br4 (84.20% of the 2097152-byte product budget)
precompress: 1385500 bytes at br11 — 380390 bytes an attendee pays for nothing
baseline   : 1765890 bytes (recorded 2026-09-22), delta +0 bytes
growth     : warn +25600 / fail +102400 bytes
budget     : warn 1887436 / fail 2097152 bytes (90% / 100%)
✅ Headroom to the product budget: 331262 bytes br4.
```

Green, with 331 KB of room — and note what that means for `.issues/134` option
2: +257 KB would fit under the ceiling while blowing the growth allowance.
Exactly the signal intended. It is a decision, not a wall.

## 5. Verification — both directions

A gate that can only pass is not a gate, so each failure path was exercised,
not assumed:

| case | result |
|---|---|
| clean tree | ✅ exit 0 |
| empty dist dir | ❌ exit 1, "No index.html" |
| dist with only index.html | ❌ exit 1, "pass vacuously" |
| a budget key deleted | ❌ exit 1, names the key, no default |
| dist older than `src/` | ❌ exit 1, refuses to measure the previous build |
| baseline 0 (all growth) | ❌ exit 1, names the growth limit |
| `MAX_GROWTH_BYTES` widened to 300,000 | guard test FAILS with the .issues/134 reason |

Also verified, because it is the one thing that could red CI spuriously: a
plain `trunk build` dist (what CI produces) and a `build.sh` dist (what the
baseline was recorded from) measure **1,765,894 vs 1,765,890 — 4 bytes apart**.

Other gates: `shellcheck` 0.11.0 clean over all 32 scripts (the repo gate
auto-discovers, so the new script was picked up without wiring);
`cargo clippy --workspace --all-targets -D warnings` exit 0;
`size_budget_guards` 10/10.

**Not verified:** the CI step itself has never run — it is wired but this
session cannot execute GitHub Actions. The 4-byte check above is the closest
local proxy, and it is why the step uses `--dir` rather than `--build`.

## 6. What the gate was then used to measure (2026-09-23)

With the gate in place, every plausible lever on the bundle was measured rather
than argued about. **The headline is that three of the four are not worth
taking, and one of them would have made things worse while looking like a
430 KB win.**

The wasm is **90.2 % code section, 8.6 % data** — no embedded blob to delete,
and the `name` custom section is already stripped to 1,267 bytes. There is no
accident here to find; it is genuinely that much compiled Leptos.

### 6.1 `wasm-opt` — the repo's own dead script would have hurt

`frontend-leptos/optimize-wasm.sh` exists, uses `-Oz`, and is **invoked from
nowhere** — not `build.sh`, not `deploy.sh`, not CI. Measured on the shipped
wasm:

| | raw | **br4 (served)** | vs baseline |
|---|---:|---:|---:|
| trunk output | 5,478,875 | **1,646,742** | — |
| `-Oz` (what the script uses) | 5,048,846 | 1,650,371 | **+3,629 worse** |
| `-O2` | 5,334,606 | **1,637,846** | **−8,896 better** |
| `-O3` | 5,262,260 | 1,643,638 | −3,104 |
| `-Os` | 5,248,050 | 1,645,387 | −1,355 |
| `-O4` | 5,266,407 | 1,653,103 | +6,361 worse |

**`-Oz` shrinks the raw wasm by 430,029 bytes and makes the bytes an attendee
downloads bigger.** Optimizing for size makes the code less compressible — the
repeated patterns brotli feeds on are exactly what `-Oz` folds away. Anyone who
wired that script up on the strength of the raw number would have shipped a
regression and called it a 7.8 % win. Same lesson as §3.1, different tool:
measure the number the user experiences.

`-O2` is the only level that helps and it is worth **8.9 KB**, a third of the
warn line. Not taken: it adds a build step and a binaryen dependency to the
deploy path for 0.5 %.

### 6.2 Stripping logging — 19 KB, and it costs debuggability

`frontend-leptos/Cargo.toml` documents it: *"Disable in release builds to strip
all log strings from WASM. Build release without logging: `trunk build
--no-default-features`"*. Nothing passes that flag — `build.sh`, `deploy.sh`
and CI all build with `console_log` on, confirmed in the live rustc invocation
(`--cfg feature="console_log"`).

Measured: `--no-default-features --features release_no_log` →
**1,746,777 br4, −19,113 bytes.**

**Not taken, and this is a judgement call rather than a measurement.** 19 KB is
1.1 % of first load, and the price is every browser-console diagnostic at an
event that is four days away. If the owner wants it after RTM#6 the number is
here; doing it now trades a debugging tool for one percent.

### 6.3 Precompression — the only real win, and it is blocked

**380,390 bytes, 21.5 %.** Five times larger than everything else on this page
combined. It is not a code change: it is the q4→q11 gap from §3.1.

**Workers Static Assets has no first-class support for it.** There is no
"upload a `.br` sibling" mechanism; the documented workaround is to serve
pre-compressed bytes with an explicit `Content-Encoding: br` header. That is
possible here — `frontend-leptos/_headers` is already used for cache control —
but it means:

- **No content negotiation.** `_headers` applies a header to a path
  unconditionally, so a client that did not send `Accept-Encoding: br` gets
  brotli bytes labelled brotli and fails. Every browser that can run wasm has
  supported brotli since ~2017, so this is small — but it is not zero, and the
  failure mode is a blank app, not a slow one.
- **SRI still has to hold.** `index.html` carries
  `integrity="sha384-…"` on the wasm preload and on all 22 stylesheets.
  Per spec SRI hashes the *decoded* body, so it should survive — "should"
  being the operative word.
- **The edge behaviour cannot be verified locally.** Whether Cloudflare passes
  our `Content-Encoding` through untouched, strips it, or re-compresses is the
  actual risk, and `wrangler dev --local` cannot answer it. Only a staging
  deploy can.

**Not attempted.** Changing how the largest asset is served, without being able
to verify the edge behaviour, four days before RTM#6, risks a blank app for
every attendee to save 21 % of load time. That trade is not close. It is a good
change for the week *after* the event, verified on staging first
([[bethere-deploy-and-rollback]]).

### 6.4 CSS — measured, and smaller than it looks

All 22 stylesheets are render-blocking, which reads like an obvious win. It is
not: the CSS is **~75 KB br4, about 4 %** of first load, and the page renders
nothing until the 1.65 MB wasm has downloaded and booted — so unblocking CSS
that arrives long before the wasm does not move first paint. Making the
staff-only sheets (admin, scanner, quiz, dashboard, event-form) async would
remove 7 render-blocking resources and **zero bytes**.

Worth doing as hygiene, not as a performance fix, and not while the wasm
dominates the critical path this completely.

## 7. Remaining

- [ ] **Precompression (§6.3)** — the 380 KB. Needs a staging deploy to verify
      edge behaviour. The single highest-value frontend change available.
- [ ] **Split the 22 render-blocking stylesheets** (§6.4) — hygiene, ~0 bytes
      while the wasm dominates. Low priority, now that it is measured.
- [ ] **Delete or fix `frontend-leptos/optimize-wasm.sh`** (§6.1). It is dead
      code that, if ever wired up, makes the shipped bundle bigger. Leaving a
      loaded footgun in the tree with an encouraging name is the risk.
- [ ] De-duplicate the two gates. They share their parse-don't-source logic,
      their `--update-baseline` handling and their reporting, and two copies of
      anything drift (`[[duplicated-state-transition-paths]]`). Not done
      tonight on purpose: the worker gate is wired fail-closed into `deploy.sh`
      with a production deploy pending, and its guard test greps the script's
      own text, so an extraction has to move the guard too. Instead
      `both_size_gates_keep_the_properties_that_make_them_trustworthy` asserts
      the four properties on both scripts at once, so the duplication cannot
      drift silently while it waits.

## Related

- `.plans/027` item A — the worker gate this is the sibling of.
- `.issues/134` — why this was needed now, and the +257,210 bytes it must catch.
- `.issues/072` — a rule that can only ever pass is not a rule.

## 8. สรุปภาษาไทย

**ปัญหา:** ฝั่ง worker มีตัววัดขนาด bundle ตั้งแต่ 22 ก.ย. แต่**ฝั่งหน้าเว็บไม่เคยมีใครวัดเลย** —
ทั้งที่ฝั่งหน้าเว็บ**ใหญ่กว่า**: first load **1.77 MB** (br) เทียบกับ worker ทั้งตัว 1.57 MB
และผู้เข้าร่วมทุกคนต้องโหลดมันบนเน็ตมือถือในงาน ก่อนจะเห็นตั๋วตัวเอง

**สิ่งที่พบระหว่างทาง:**
- **CSS 22 ไฟล์ถูกโหลดพร้อมกันหมดทุกหน้า** (render-blocking) — คนเปิดดูตั๋วต้องโหลด CSS ของหน้า
  admin, scanner, dashboard, quiz ไปด้วย
- **Cloudflare บีบอัด brotli ที่ระดับ ~4 ไม่ใช่ 11** (วัดกับของจริงบน prod) — ถ้าวัดที่ 11
  ตัวเลขจะ**ต่ำกว่าความจริง 28%** แล้วยังเรียกตัวเองว่า "วัดแล้ว"
- **ถ้าบีบอัดไฟล์ไว้ล่วงหน้าตอน build จะประหยัดได้ 380 KB ต่อการโหลด 1 ครั้ง (21.5%)
  โดยไม่ต้องแก้โค้ดเลยสักบรรทัด** — ใหญ่กว่าฟีเจอร์ไหน ๆ ในคิว แต่แตะ deploy path
  เลยขอให้คุณตื่นมาทำเอง

**สิ่งที่ทำแล้ว:** สร้าง gate + baseline + ต่อเข้า CI + เทสกันการเสื่อม 10 ตัว
ทดสอบ**ทั้งทางผ่านและทางไม่ผ่าน** 7 เคส

---

**แล้ววัดทุกทางที่จะลดขนาดได้ (23 ก.ย.) — สรุปคือ 3 ใน 4 ทางไม่คุ้ม และ 1 ทางจะทำให้แย่ลง**

- **`optimize-wasm.sh` ในโปรเจกต์นี้ (ใช้ `-Oz`) — ถ้าเอาไปใช้จริงจะแย่ลง**
  มันลดขนาดไฟล์ดิบได้ 430 KB แต่**ขนาดที่ผู้ใช้โหลดจริงเพิ่มขึ้น 3,629 bytes**
  เพราะ `-Oz` ตัดรูปแบบซ้ำ ๆ ที่ brotli ใช้บีบอัดออกไป
  (สคริปต์นี้ไม่มีใครเรียกใช้เลย — ไม่อยู่ใน build.sh, deploy.sh หรือ CI)
  ระดับเดียวที่ช่วยคือ `-O2` แต่ได้แค่ **8.9 KB** ไม่คุ้มกับการเพิ่มขั้นตอน build
- **ปิด log ตอน release** (Cargo.toml เขียนไว้เองว่าควรทำ แต่ไม่มีใครใส่ flag) — ได้ **19 KB**
  **ยังไม่ทำ**: แลกกับการเสีย console log ตอนดีบักหน้างาน ซึ่งเหลืออีก 4 วัน ไม่คุ้ม
- **บีบอัดล่วงหน้า = 380 KB (21.5%) — ทางเดียวที่คุ้มจริง แต่ติดอยู่**
  Workers Assets **ไม่รองรับโดยตรง** ต้องใช้วิธีอ้อมผ่าน `_headers`
  ซึ่ง (ก) ไม่มีการเจรจา encoding — client ที่ไม่รับ brotli จะพัง (ข) ต้องพึ่ง SRI ทำงานถูก
  (ค) **ตรวจสอบพฤติกรรมฝั่ง Cloudflare edge ในเครื่องไม่ได้ ต้อง deploy ขึ้น staging เท่านั้น**
  **ยังไม่ทำ**: เสี่ยงหน้าเว็บขาวทั้งงาน เพื่อแลกกับโหลดเร็วขึ้น 21% — ไม่คุ้มตอนนี้
  ควรทำ**หลัง**งาน RTM#6 และทดสอบบน staging ก่อน
- **CSS 22 ไฟล์** — วัดแล้วเป็นแค่ **4%** ของทั้งหมด และหน้าเว็บก็ยังไม่แสดงอะไรอยู่ดี
  จนกว่า wasm 1.65 MB จะโหลดเสร็จ → แก้แล้วได้ **0 bytes** เป็นแค่การจัดระเบียบ ไม่ใช่การเร่งความเร็ว

**ตอนนี้:** 84.20% ของงบ เหลือที่ว่าง 331 KB — พอดีกับที่ `.issues/134` ทางเลือกที่ 2
(+257 KB) จะ**ผ่านเพดานแต่ทะลุเส้นการเติบโต** = บังคับให้เป็นการตัดสินใจ ไม่ใช่การแอบโต
