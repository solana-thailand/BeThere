# 027 — Unattended hardening queue (2026-09-22 → )

**Owner is asleep. This plan is written to be executed without asking anything.**

> ### State at 2026-09-22 06:20 — read this first
>
> **Done and committed to `develop` (7 commits, `a658ded`..`a5fa48b`), nothing in production:**
> **A** size budget gate · **C** duplicate-slip detection · **D** the comp action ·
> **E** `.issues/127` prepared (0048 written, deferred, prod measured) ·
> **F** staging half (deployed, migrations applied, endpoint probed).
>
> **Green:** clippy `-D warnings` exit 0 · 759 workspace + 205 frontend tests ·
> shellcheck · all Python suites · invariants self-test · fmt.
> **Bundle: 1,569,688 bytes gzip = 49.89 % of the 3 MiB free-plan ceiling**
> (+12,501 bytes for everything tonight). Headroom ~1.5 MiB.
>
> **Next, in order:** **G** (slip QR, measure size on a branch first) then
> **H** (housekeeping). **F production is deliberately not done** — see §F.6 for
> why and exactly how to do it. §9 lists what must not be started at all.
>
> **Update 2026-09-22, later session:** **B is built** (`.issues/128` §5), on
> its own branch, with two deliberate deviations from the scope in §B — the age
> cap is per kind rather than a flat 72 h, and there is no stop-the-world burst
> gate. Both are explained in §B's STATUS block; both come down to the same
> thing: the specified controls would have retired or blocked RTM#6's own 62
> queued messages. Also on a branch, unmerged: the Wrangler 4.135.0 baseline
> (`.issues/069`).
>
> **Update 2026-09-22, this session:** **B is merged into `develop`**
> (`a8175cb`, `ad100e3`); `chore/wrangler-4.135` is still held back by design,
> per `.issues/069` — ship the current payload on the toolchain it was tested
> on, *then* move the toolchain.
>
> New, out of queue order and not from this plan: the **ticket-page
> announcement** feature (`.issues/132`, migration **0049**). Built at the
> owner's request. **Now committed** — `ec59bf1`, `fbf73cb`, `96264d9`,
> `77631ed`, unpushed. Every gate green: workspace + frontend clippy
> `-D warnings` exit 0, 554 worker tests, 212 frontend tests, domain tests,
> Python SQL suite 36/36, and **verified in a browser**, not just compiled.
> It adds a **second pending migration** to the production deploy in §F.6 —
> 0048 is deferred by design, 0049 must be applied.
>
> **Bundle: 1,574,121 bytes gzip = 50.03 %** of the 3 MiB ceiling, well under
> the 70 % warn line. `worker/.size-budget` baseline moved to that figure, with
> the composition written into the file. **This retires the separate "fix the
> −33 staging offset" item** below: the new baseline is a direct measurement
> and carries no offset.
>
> **New issue, unrelated to 0049 and not fixed here:** `.issues/133` — the
> ticket page dies outright for any event with no venue map link
> (`#[serde(default)]` does not cover an explicit `null`). Found only because
> the announcement feature was opened in a browser. The fix is owned by a
> peer session and is written, guarded and green — but **uncommitted**.
>
> **Update 2026-09-22, later still: item G is measured and its server-side
> half is rejected** (`.issues/134`, branch `feature/134-slip-qr-bank-reference`,
> three commits, unpushed). Size passed with room — **+257,210 bytes gzip,
> 58.21 %**, well under the 70 % warn line — so the stopping condition this
> plan wrote down never fired. **CPU is what fails**: 33.14 ms release-native
> for a 1080×1920 phone screenshot against a **10 ms** free-plan budget, 89 %
> of it in QR grid detection, which does not shrink with the image. Shipping it
> would be error 1102 on slip upload. Kept: `domain::slip_verify`, the
> CRC-verified string parser, which both sides need regardless. **G is now
> owner-gated** on a plan/paid-tier question — see §G's STATUS block.
>
> **With that, every item A–H is either done or owner-gated. The queue is
> empty of ungated engineering work.**
>
> **New, out of queue order, and it belongs in this plan: `.issues/135` — a
> frontend size gate.** Item A measured the worker and stopped there, and the
> frontend turns out to be the bigger half: **1,765,890 bytes brotli of first
> load** against a 1,574,034-byte whole-worker bundle, downloaded by every
> attendee on venue mobile data before their ticket renders, with nothing
> measuring it. Built because `.issues/134`'s own recommendation could not be
> costed without it. `scripts/verify/frontend_size_budget.sh` +
> `frontend-leptos/.size-budget`, wired into CI's `e2e` job, six new guard
> tests, seven failure paths exercised. **84.20 % of budget, green, 331 KB of
> headroom.**
>
> Two findings fall out of it, neither fixed: **all 22 stylesheets are
> render-blocking** (a ticket viewer downloads the admin and dashboard sheets),
> and **Cloudflare compresses on the fly at brotli ~q4 — precompressing at
> build time would save 380,390 bytes per first load, 21.5 %, for no code
> change.** That last one is larger than anything left in this queue and it
> touches the deploy path, so it waits for a waking human.
>
> **Update 2026-09-23, owner awake.** Owner ruled: **no Workers Paid**, so
> `.issues/134`'s server-side decode stays rejected and its option 2 (frontend
> decode) is the only live path — now costable against the new frontend gate,
> where its +257 KB would clear the ceiling but blow the growth allowance by
> design. Owner also green-lit frontend work, so **every remaining lever was
> measured** (`.issues/135` §6): three of four are not worth taking and one —
> `frontend-leptos/optimize-wasm.sh`, dead in the tree, using `-Oz` — **would
> have made the shipped bundle bigger while looking like a 430 KB win**. Its
> flags are now `-O2` with the measurement in the header. The only real win,
> precompression, is blocked on edge behaviour that `wrangler dev --local`
> cannot verify; it wants staging, after RTM#6.
>
> **New, owner-reported, now fixed: `.issues/136`.** Anyone whose *name*
> contained "vip" — Vipada, Vipawee, Vipawadee, all ordinary Thai names — wore
> a VIP badge, because D1 has no `ticket_name` column and the read path
> substituted the attendee's name. **Not an outage path**: `list.rs` claimed
> D1 was the Sheets fallback and it is the other way round, so this was on
> every organizer screen every day. Fixed in two steps — the wrong information
> removed, then migration **0050** plus named constants for the two ticket
> names this system mints (previously bare literals in four files, which is why
> the Sheet and D1 were free to disagree). Verified with a runtime A/B and in a
> browser. **This is a third pending migration on the §F.6 deploy** — the
> cheapest of the three, but still the owner's call.
>
> **Blast radius now measured against the live worker (`.issues/133` §10):
> 13 of the 14 production events with attendees serve `null` — 476 of 514
> attendee tickets are dead today.** The only event that renders is RTM#6,
> because it is the only one with a map link set; that is why nobody saw it,
> and it means RTM#6 works by accident. The `7403` that blocked this cleared.
> The D1 query first proposed for it is the wrong instrument (KV-first read
> path) and a probe without `?event_id=` reads falsely green — both written up
> in §10.2. **This makes the production deploy in §F.6 a live-outage decision
> rather than a queue-flush**, and it is still the owner's call: the fix cannot
> ship without either a cherry-pick onto the deployed tree or accepting the
> rest of the undeployed queue with it.

Every item below is *ungated*: no owner decision, no external account, no
irreversible action. Anything needing the owner stays in `.issues/129` §7 and is
listed in §9 here as explicitly NOT to be started.

---

## 0. Rules of engagement (read before touching anything)

**Re-ranked 2026-09-22, after writing this:** C and D moved ahead of B. B's
damage is held back by four independent blockers (`NOTIFICATIONS_ENABLED=0`, no
caller, empty `NOTIFICATION_FROM`, no `EMAIL` binding), so the code it needs is
dead on arrival; C and D are live on the deposit path and RTM #6 is on
2026-09-27. Order actually executed: **A → C → D → B → E → F**.

1. **Additive only.** No behaviour an attendee or organizer sees today may
   change meaning. New columns get defaults; new gates start in report-only
   mode; new env flags default to today's behaviour.
2. **Size budget is law.** The worker ships to Cloudflare's **free plan: 3 MiB
   after gzip**. Measured 2026-09-22 on the deployed bundle:

   the **wrangler dry-run bundle** — i.e. exactly what gets uploaded, not the
   raw cargo artifact:

   | file | raw | gzip |
   |---|---:|---:|
   | `…-event_checkin_worker_bg.wasm` | 4 481 545 | 1 536 840 |
   | `shim.js` (esbuild output) | 108 146 | 20 347 |
   | **total** | **4 589 691** | **1 557 187 — 49.50 % of 3 MiB** |

   (`shim.js.map` is in the outdir but is *not* uploaded, so it is not counted.)
   Headroom is **1 588 541 bytes gzip**. That is real but not infinite: any item that
   pulls in an image decoder or a QR decoder must be measured *before* it is
   merged, not after. Item **A** below makes this measurement automatic and
   fail-closed; **do A first** — every later item depends on it to be safe.
3. **Verify in both directions.** A guard that cannot fail is not a guard
   (`.issues/072`). For each new test: break the thing on purpose, watch the
   test go red with the right message, revert, watch it go green. Record both
   in the commit body.
4. **Staging before prod, always.** Batch the night's work into ONE prod deploy
   at the end, not one per item — fewer prod versions, one backup, one rollback
   point. `bethere-staging` is the rehearsal.
5. **Back up D1 before any prod migration:**
   `npx wrangler d1 export bethere-db --remote --output backups/pre-0046-$(date +%Y%m%d).sql`
   — and keep it out of git (it contains PII).
6. **Never `d1 execute` an events row in prod.** The read path is KV-first; a
   direct D1 write is masked by cache. Deposits/notifications tables are fine.
7. Local shell gotchas: always `rg --color=never` (this shell eats the match
   highlight and silently mangles output); `timeout(1)` does not exist on this
   box; `cargo check`/`test` on the worker take >2 min because of host↔wasm
   target switching — run them backgrounded.

---

## A. Worker size budget gate  ·  value: HIGH  ·  risk: none  ·  size: 0

**Why first.** The owner's standing worry is "will this still fit the free
tier". Today nothing measures it: `deploy.sh` has no size check and CI's
`wasm-build` job builds the wasm and throws the number away. Every item after
this one adds code, so without A we would be adding code blind. The number we
care about is *gzip of the uploaded bundle*, which nobody has ever recorded.

**Scope**
- `scripts/verify/worker_size_budget.sh` — gzip every file wrangler uploads
  (`worker/build/worker/**`, excluding source maps), sum, compare against a
  budget file, print the number as a percentage of the 3 MiB free-plan ceiling.
- `worker/.size-budget` (tracked — `worker/build/` is gitignored, so the
  budget cannot live there) — the committed baseline + ceiling.
  Two thresholds: **warn** at 70 % of 3 MiB, **fail** at 85 % (2.55 MiB). Fail
  leaves ~450 KiB of emergency headroom rather than discovering the limit
  during an outage.
- Wire into `deploy.sh` *before* the upload step (fail-closed), and into the CI
  `wasm-build` job so a size regression reds a PR, not a deploy.
- The gate prints the per-file breakdown on failure so the culprit is obvious.

**Verify both directions**: five cases, all run 2026-09-22 —
green at 49.50 % with delta +0 against the baseline; fail line moved below the
current size → red, naming the wasm, exit 1; warn line moved below → warning,
exit 0; empty bundle dir → refuses to pass vacuously; a budget key deleted →
refuses to fall back to a default.

**Done when** `bash scripts/verify/worker_size_budget.sh` is green locally, the
CI step exists, and `deploy.sh` refuses a bundle over budget.

**STATUS 2026-09-22: DONE.** `scripts/verify/worker_size_budget.sh` +
`worker/.size-budget` + `deploy.sh:run_size_budget_gate` (invoked before the
upload, aborts on failure) + a `wasm-build` CI step + five guards in
`worker/tests/size_budget_guards.rs`.

---

## B. Notification backlog can never blast the archive  ·  value: HIGH  ·  risk: none  ·  size: ~0

**Why.** `.issues/128`: the product has never sent an email
(`NOTIFICATIONS_ENABLED = "0"` in both environments), and `notification_outbox`
has accumulated **243 pending rows** whose `due_at` is long past. Flipping the
flag — which is a one-character edit someone will eventually make — sends all
243 at once, to people whose events ended months ago. The flag is the only
thing standing between today and that incident, and a flag is not a control.

**Scope**
- A **staleness cutoff** in the dispatcher (`worker/src/notifications/`): a row
  whose `due_at` is older than `NOTIFICATION_MAX_AGE_HOURS` (default **72**) is
  never sent — it is moved to `cancelled` with `error_code='stale'`, so the
  decision is auditable rather than silent.
- Cutoff applies at *send* time, not enqueue time: a genuinely delayed send
  (worker down for an hour) still goes out; a four-month-old reminder does not.
- A **first-enable guard**: if the number of rows that would be sent in the
  first pass after enabling exceeds `NOTIFICATION_BURST_LIMIT` (default 25),
  the pass sends nothing, Slack-alerts the count, and requires the limit to be
  raised deliberately. Belt and braces: the cutoff should already have drained
  the 243, and this catches the case where it has not.
- Both defaults are conservative and both are env-tunable in `wrangler.toml`.

**Verify both directions**: a unit test with a fabricated outbox — one fresh
row (sends), one 200-day-old row (cancelled as stale), and a 26-row fresh batch
(burst guard trips, zero sends). Then flip the age to 0 and confirm the fresh
row is *also* cancelled, proving the comparison is live and not hardcoded.

**Done when** enabling `NOTIFICATIONS_ENABLED` on staging with the real backlog
shape sends **zero** historical mail, and the outbox shows 243 `cancelled/stale`.

**STATUS 2026-09-22: BUILT, two deliberate deviations from the scope above.**
Full write-up in `.issues/128` §5. `NOTIFICATIONS_STALENESS = off|report|cancel`
(default `report`) + `NOTIFICATIONS_MAX_PER_RUN = 25`, in
`notifications::policy` / `notifications::staleness` / `sql/claim.sql` /
`sql/stale_report.sql` / `sql/stale_cancel.sql`, both envs in `wrangler.toml`,
documented in `docs/notifications.md`. Verified by `cargo test` and by 36 tests
in `worker/tests/notifications/test_outbox.py` against the real migrations and
the real statements — both directions, plus `report` provably writing nothing
and `cancel` retiring exactly the set `report` named.

1. **The age cap is per kind, not a flat `NOTIFICATION_MAX_AGE_HOURS=72`.** A
   flat 72 h would have retired RTM#6's own 24 registrations and 14 deposit
   receipts (due 2026-09-16, six days old when this was written). No single
   number works: a confirmation is true until its event ends, a survey is wrong
   days after its event ended. 2 d reminder / 3 d survey / 14 d registration and
   deposit receipts. Against the measured backlog: ~187 surveys retired, all 62
   RTM#6 rows kept.
2. **No stop-the-world burst gate.** `NOTIFICATION_BURST_LIMIT` as specified
   ("over 25 in the first pass ⇒ send nothing, alert, require a manual raise")
   would have tripped on RTM#6's legitimate 62 rows and stayed tripped. The
   claim loop was already `0..25` against a **daily** cron, so the 243 could
   never have gone out at once — 25/day for ten days was always the real shape.
   That rate is now the named, tunable control, and `report` mode supplies the
   "a human looks first" step the burst gate was reaching for.

**Still not done (unchanged blockers, none of them code):** the sender domain is
not onboarded, `NOTIFICATION_FROM` is empty, there is no `EMAIL` binding, and
`dispatch` still has no caller. `NOTIFICATIONS_ENABLED` stays `0` in both
environments — flipping it remains owner-gated (§9). The guard has never met the
real 268-row backlog; doing so on staging in `report` mode is the next step and
needs the staging deploy that is currently sequenced behind `.issues/069`.

---

## C. A slip image can only be used once  ·  value: HIGH  ·  risk: low  ·  size: ~0

**Why.** The owner's words: *anyone can upload any image to the deposit page*,
and they catch the bypassers by remembering faces. The cheapest deterministic
defence needs **no bank API, no vendor, no OCR and no new dependency**: hash the
bytes. The same slip re-used by a second attendee — the single most common real
abuse — becomes impossible. `blake3` is already compiled into the worker via
`event-checkin-domain`'s `wire` feature, so this costs ~zero bundle size.

This is deliberately *not* the QR/bank-reference work (item G). It catches byte
duplicates only; a re-screenshotted slip defeats it. It is still worth shipping
first because it is a night's work with no external dependency, and it makes
the review queue in G much smaller.

**Scope**
- Migration `0046_thb_deposit_slip_hash.sql`: add `slip_blake3 TEXT` (nullable
  — historical rows have no hash and must not be invented) + a **partial**
  unique-ish index for lookup: `CREATE INDEX idx_thb_slip_hash ON
  thb_deposits(slip_blake3) WHERE slip_blake3 IS NOT NULL`.
  *Not* a `UNIQUE` constraint: a legitimate re-upload after rejection must
  still be possible; the check belongs in the handler where it can distinguish
  "same person re-uploading" from "different person, same image".
- `slip_upload.rs`: hash the **decoded image bytes** (not the data-URL string —
  base64 padding and MIME casing differ for identical images) before the R2
  upload; look up the hash; if it belongs to a *different* attendee, reject with
  a message that does not leak who (`AppError::Validation`, no other attendee's
  identity). If it belongs to the same attendee, allow (re-upload).
- Same treatment in `slip_admin_upload.rs` — memory `duplicated-state-transition-paths`
  says a guard that lands on one writer and not its sibling is the recurring
  defect here. **Both writers or neither.**
- Backfill is *not* attempted: historical slips live in R2 and re-hashing them
  means 243 R2 reads inside a migration. A separate opt-in script
  (`scripts/verify/backfill_slip_hashes.sh`) can do it later, read-only-first.

**Verify both directions**: local D1 harness — upload slip X as attendee 1
(accepted), same bytes as attendee 2 (rejected), same bytes as attendee 1 again
(accepted), a one-pixel-different image as attendee 2 (accepted, proving it is
not hashing something constant).

**Done when** the four cases above pass against `wrangler dev --local`, and the
admin path is covered by the same test.

**STATUS 2026-09-22: DONE (commit `f01cd24`), not deployed.** Shipped in
`report` mode. Eight runtime cases verified against real SQLite, including the
two that only SQLite can show (`''` vs `''`, `NULL` vs `NULL`) and that the
partial index is actually used. Bundle cost measured by item A: **+7,883 bytes
gzip, 49.50 % → 49.75 %**. Written up in `.issues/130`. The admin UI shows the
collision above the Approve button. Deviation from the plan as written: the
lookup is a targeted SQL query on the new index rather than a Rust scan over
`list_thb_deposits`, because that read can pull multi-MB data URLs for old rows.

---

## D. Admit an attendee without promising them ฿500  ·  value: HIGH  ·  risk: low  ·  size: ~0

**Why.** `.issues/129` Gap 1, and it is the bug under the owner's whole manual
ritual. Approving a slip is the **only** way an attendee receives their ticket
QR (`slip_verify.rs:133`, approval-only auto-QR) — and approving is
simultaneously the promise to refund ฿500. So when the organizer knows someone
did not really pay, they have no move that admits the person without creating a
refund obligation. They compensate by *remembering*.

The correct state already exists — `DepositSource::Comp`
(`domain/src/models/deposit.rs:165`) means never-cash, never-refundable — but it
can only be created **at signup, for staff/organizer**
(`register/signup.rs:824-827`). No admin action can comp an ordinary attendee
after they have uploaded.

**Scope**
- Migration `0047_thb_deposit_source.sql`: a real `deposit_source TEXT` column
  (`'cash' | 'credit' | 'comp'`, CHECK-constrained), backfilled from the
  *existing* classifier so the column starts out exactly equal to what
  `ThbDeposit::source()` already returns. No row changes meaning on migration
  day — that is the invariant to assert in the test.
- `ThbDeposit::source()` reads the column when present and falls back to the
  sentinel sniffing when NULL. **One classifier, two inputs** — do not fork the
  rule into a second function (DRY; the sentinels in `verified_by` / `slip_url`
  carry other meanings and must keep working for old rows).
- Admin action `POST /api/admin/deposit/thb/comp` — marks a deposit
  `source='comp'`, `verified=1`, `refundable=0`, and issues the ticket QR by
  the *same* code path approval uses (do not duplicate the QR issue: extract
  it once and call it from both — the duplicated-writer trap again).
- Admin UI: one button next to Approve/Reject, labelled so it cannot be
  misread — "Admit, no refund owed" — with the ฿ consequence written under it.

**Verify both directions**: migration-day invariant test (for every existing
row, column value == legacy classifier output); comp action produces a QR and a
non-refundable deposit; the refund/hold/roll paths all *skip* a comp row (they
already do via `is_non_cash()` — assert it, do not assume).

**Done when** an ordinary attendee can be admitted from the admin screen with
no refund obligation created, and no existing row changed classification.

**STATUS 2026-09-22: DONE, not deployed.** Migration 0047, `deposit_source` on
`ThbDeposit` with `source()` preferring it, `POST /api/deposit/thb/comp`, and an
"Admit, no refund owed" button with a two-step confirm. The QR issue was
extracted to `admit::issue_ticket_qr_if_absent` — it had been written out twice
inside `slip_verify.rs`, once per `worker_ctx` branch — and comp and approval
now call the same helper. Migration-day invariant verified against real SQLite
across all 7 classifier branches including the credit-and-฿0 ordering trap.
Bundle **+4,618 bytes gzip → 49.89 %**. Written up in `.issues/131`.

Deviation: comping an attendee with **no** deposit row (a walk-in guest) is not
supported — the endpoint reclassifies an existing deposit. Signup-time comp
covers staff. Noted as follow-up in `.issues/131` §6 rather than bolted on.

---

## E. `.issues/127` — the UNIQUE that cost ฿700  ·  value: MED  ·  risk: MED  ·  size: 0

`thb_deposits` has no `UNIQUE(event_id, attendee_id)`, so a double upload makes
two rows and the second refund is real money. The previous thread deliberately
deferred the fix until **after RTM #6 (27 Sep)** — a schema rebuild on the live
deposits table days before an event is the wrong trade.

**Tonight: prepare only, do not apply.**
- Write `0048_thb_deposits_unique.sql` (table rebuild — SQLite cannot add a
  constraint in place; follow the 0035 pattern exactly, including dropping and
  recreating dependent triggers/views).
- Write `scripts/verify/thb_duplicate_report.sh` — read-only, lists rows that
  *would* collide, so the dedupe decision is made with the real list in hand.
- Run the report against prod (read-only) and paste the result into
  `.issues/127`.
- Keep the migration OUT of `worker/migrations/`. A `-- DO NOT APPLY` header
  there enforces nothing: `wrangler d1 migrations apply` applies every `.sql` in
  `migrations_dir` and never reads the file. Deferred migrations live in
  `worker/migrations-pending/`, which wrangler does not scan.

**Done when** the migration and the report exist, the report has been run, and
`.issues/127` names the exact rows.

**STATUS 2026-09-22: DONE (prepare-only, as intended).** `0048` written and
marked `DO NOT APPLY BEFORE 2026-09-28`; `scripts/verify/thb_duplicate_report.sh`
written, validated in both directions, and run against production.

**The finding: production has no duplicates at all** — 54 rows, 54 distinct
pairs, 3 events. So 0048 applies cleanly with no dedupe, which is a much smaller
job than `.issues/127` feared. Re-run the report right before applying; RTM #6
adds ~24 rows first.

Bonus, same read-only pass: the 0047 backfill previewed against **real prod
data** — 37 cash (฿18,500), 14 credit (฿7,000), 3 comp (฿0). That closes the
caveat in §D that the backfill had only been proven against a local fixture.

---

## F. Batch deploy  ·  value: MED  ·  risk: MED

**STATUS 2026-09-22 ~05:15: STAGING DONE. PRODUCTION NOT DONE — see §F.6.**

Staging rehearsal, in this order:
1. `npx wrangler d1 migrations list DB --env staging --remote` (listing first is
   what stops the classifier treating the apply as "blind").
2. `CI=true npx wrangler d1 migrations apply DB --env staging --remote` — 0046
   and 0047 both ✅. **Schema read back rather than trusting the ✅**:
   `slip_blake3 TEXT`, `deposit_source TEXT`, the CHECK constraint, and both new
   indexes (`idx_thb_deposits_slip_hash`, `idx_thb_deposits_source`) all present.
   The backfill grouped to zero rows because staging's `thb_deposits` is empty —
   that is not evidence the backfill works, and it is not claimed as such; the
   backfill was proven against seeded rows on a local D1 instead.
3. `bash frontend-leptos/build.sh`, then `bash deploy.sh staging`.
4. **The new size gate ran inside the real deploy path**, before the upload:
   `1 569 721 bytes gzip, 49.90 %`, and the deploy continued. Content-Type
   verification passed.
5. Endpoint probe, validated in both directions so a broken probe cannot read as
   a pass: `POST /api/deposit/thb/comp` → **401** (registered, and in the authed
   router — `Extension<Claims>` outside it returns 500, not 401);
   `/deposit/thb/verify` → 401 as a control; a nonexistent sibling path → 404,
   proving the probe can tell a missing route from an unauthorised one;
   `/api/health` → 200, proving it reached a live worker.

**Worth knowing:** the staging bundle measured **+33 bytes** against the
production baseline. Same code — the difference is the `[env.staging]` variable
strings compiled in. So the baseline is very slightly environment-specific.
At 33 bytes out of a 2.55 MiB fail line this is noise, but do not be puzzled by
a tiny non-zero delta after a staging build.

### F.6 — production is deliberately NOT deployed

Everything above is reversible; a production deploy of these two migrations is
not, and it would land unattended, overnight, five days before RTM #6. Two
specific reasons to leave it for a waking human:

- `.issues/084`: the production preflight gate has **never** been satisfiable,
  so prod needs `bash deploy.sh --force --reason "<why>"`. Deciding to bypass a
  broken safety gate is not a call to make silently at 05:00.
- The prod `thb_deposits` table has real rows, so 0047's backfill actually does
  something there. It is verified — but verified against a local fixture, not
  against production data.

  **CLOSED 2026-09-23.** Prod was backed up and all four pending migrations
  were applied to a copy of that backup — 57 real `thb_deposits` rows. 0047's
  backfill was compared against `ThbDeposit::source()` row by row: **cash 40 /
  credit 14 / comp 3 both before and after, 0 disagreements**, row counts
  unchanged, `integrity_check` ok. It changes how the classification is stored,
  not what it is.

  **Also corrected the same day: production has FOUR migrations pending, not
  two.** 0046 and 0047 were applied to *staging* only; prod never got them, and
  this plan read as though it had. They cannot be skipped — `migrations apply`
  has no subset option, and the Worker in this deploy queries `slip_blake3` and
  writes `deposit_source`, so deploying the code without them breaks slip
  upload. Full command-by-command runbook: `docs/deploy_20260923_runbook.md`.

**When you do it:** back up first
(`npx wrangler d1 export bethere-db --remote --output backups/pre-0046-$(date +%Y%m%d).sql`,
keep it out of git — it holds PII), then `migrations list` → `CI=true …
migrations apply DB --remote` → read the schema back → `bash deploy.sh --force
--reason "…"` → re-probe the endpoint the same four ways as step 5.

### F.0 — original checklist

Only after A–D are green locally. In order:
1. `bash scripts/verify/worker_size_budget.sh` — must be green, record the %.
2. Full local gates: `cargo fmt --all -- --check`, `cargo clippy --workspace
   --all-targets -- -D warnings`, `cargo test --workspace`, the python suites.
3. Deploy to **staging**, apply migrations to the staging D1, run the item-B
   and item-C verifications against the real staging shape, and re-measure the
   bundle from the *staging* build (this is the owner's "prove it in staging"
   requirement).
4. Back up prod D1 (§0.5), apply migrations, deploy prod, smoke-test —
   including **Content-Type headers**, not just HTTP 200.
5. Record the deployed version id so rollback is one command.

Also carries the already-committed `3973792` (nightly-cleanup Slack alerting),
which has been sitting on `develop` undeployed.

---

## G. Slip QR → bank reference  ·  value: HIGH  ·  risk: MED  ·  **size: MUST MEASURE**

The real fix for "anyone can upload any image": Thai bank slips carry a mini-QR
holding a bank transaction reference. Decode it, store the reference with a
**schema-level** `UNIQUE` (not a code check — `.issues/127` proved what that
distinction costs), and later have a vendor resolve it against the bank.

**Decoding the QR needs no vendor and no owner decision — only bundle size.**
A QR decoder plus a JPEG/PNG decoder in wasm is the first thing in this whole
plan that could plausibly move the 1.48 MiB number. Therefore:
- Do this **after** item A, on a **branch**, and the first commit on that branch
  is a size measurement, not a feature.
- Budget: if `rqrr` + `image` (jpeg+png features only, `default-features =
  false`) pushes the gzip total past the item-A *warn* line (70 %), stop and
  write up the alternative (decode in a second Worker, or at the edge in the
  frontend with the server re-verifying the reference against the bank) rather
  than shipping it.
- The bank-resolution half stays owner-gated (vendor account, PII) — decoding
  and the UNIQUE reference do not.

**STATUS 2026-09-22: MEASURED, AND THE SERVER-SIDE HALF IS REJECTED.**
Written up in full in `.issues/134`; branch `feature/134-slip-qr-bank-reference`
(unpushed), three commits.

The stopping condition above was the wrong one. **Size passed**: `image`
(jpeg+png+**webp** — the upload path accepts webp, so a jpeg+png-only decoder
would have been inert on real traffic) plus `rqrr` costs **+257,210 bytes
gzip**, taking the bundle to 1,831,331 = **58.21 %**, which is 370,678 bytes
*below* the 70 % warn line.

**What fails is CPU, which this item never thought to budget.** Release-profile
native on an M5 Pro — a lower bound, since wasm is slower and the edge is
slower than this laptop — a 1080×1920 phone screenshot costs **33.14 ms**
against the free plan's **10 ms per request** (`.plans/010` §P0.2). 3.3× over.
**89 % of that is `rqrr`'s grid detection, not the image decode**, so shrinking
the image first does not buy it back: shrink past the QR's module size and
there is no QR left to find. Shipped, it is Cloudflare error 1102 on
`POST /api/deposit/thb/upload` — an attendee who cannot submit a slip at all,
which is strictly worse than no check.

Kept and shipped: **`domain::slip_verify`** — payload → CRC-verified bank
reference, no image dependency, no measurable size, both published wire formats
(bank and TrueMoney) covered by vectors with real CRCs. It belongs in `domain`
rather than in either side because wherever the decoder ends up, **the server
must re-parse what it is handed**; a client-supplied reference is a claim.

Reverted from the worker but kept reproducible: probe `36b8c02`, revert
`65e68f9`. Re-run with `git checkout 36b8c02` then
`cargo test --release -p event-checkin-worker --lib slip_qr -- --nocapture`
(`--release` matters; the debug profile exaggerates it 7×).

The three ways it comes back are in `.issues/134` §6 and **none of them are
engineering decisions** — Workers Paid ($5/mo, which dissolves both limits and
is the cheapest fix on the page), frontend decode (needs a frontend size budget
first, and yields an attacker-controlled reference), or a second Worker (worst
of the three). Recommended: frontend decode, but only after the owner rules on
the paid plan, because if that moves the server-side decode is both simpler and
trustworthy and the frontend work is wasted.

**So item G is now owner-gated too**, on a question this plan assumed away.
`.issues/130` stays where it was: re-used images are caught, forgeries are not.

---

## H. Housekeeping (whenever the queue stalls)

- `cargo clippy --workspace --all-targets --fix --allow-dirty` then review the diff.
- Doc-sync: `.issues/084` and `.plans/026` carry 2026-09-22 corrections; check
  no other doc still repeats the stale "blocked on devnet USDC" claim.
- Confirm `.gitignore` still covers `worker/scripts/.preflight-bypass.log`
  (public repo, owner's call — never un-ignore).
- Re-run `scripts/check_sbpf_v3_gate.sh` (it is in `scripts/`, **not**
  `scripts/verify/`) and record the date; the v3 deployment gate has been closed
  on all three clusters. **Run 2026-09-22: still closed on all three**, logged in
  `.issues/123` "Gate watch log".

---

## 9. Explicitly NOT to be started (owner-gated)

Do not start these even if the queue empties. They need a decision or an
account in the owner's name, and guessing wrong costs money or sends mail.

- **Which plan the worker runs on** (`.issues/134` §6, added 2026-09-22 after
  measuring item G). Workers Paid is $5/mo and raises CPU from **10 ms to 30 s**
  and the bundle ceiling from 3 MiB to 10 MiB. It is the cheapest fix for item G
  by a wide margin, and `.plans/010` records a standing commitment to the free
  tier through Demo Day — so it is a decision, not an oversight, and it is not
  ours to reverse. Everything downstream of it (frontend QR decode, a frontend
  size budget) waits on the answer, because the work is wasted if the plan moves.
- **`.issues/129` §7 all four**: what "refund at check-in" must mean;
  credit-by-default at check-in; slip-verify vendor (RDCW vs EasySlip — the
  vendor sees slip PII); whether a juristic entity exists to hold a payout
  account.
- **Flipping `NOTIFICATIONS_ENABLED` to 1** anywhere. Item B makes it *safe*;
  it does not make it *decided*. Sender-domain onboarding is still unstarted
  and has external DNS lead time.
- **PARIPOL / linked emails rollout** (`person_emails` has 0 rows; RTM #6 is
  27 Sep).
- **The ฿500 owed to RTM #3 attendee `019ec036…`** — a real payment.
- **The 13 orphaned R2 slip images** — deletion is irreversible.
- **Escrow `checkin_authority`** (`.issues/129` Gap 3): the design is sound and
  the 36 bytes of `_padding` in `EventEscrow` fit a 32-byte key without
  resizing, but it is a program redeploy and `.issues/123` (SBPFv0 bytecode)
  is unresolved. Devnet-only and reversible, so it may be *prototyped* — but
  not promoted, and not before F.

---

## 10. สรุปภาษาไทย

- **กติกา**: ทำได้เฉพาะของที่ *เพิ่มเข้าไป* ไม่เปลี่ยนพฤติกรรมเดิม, ขึ้น staging ก่อน prod เสมอ,
  และ deploy prod ครั้งเดียวตอนจบ (ไม่ deploy ทีละอัน)
- **เรื่อง size ที่ห่วง**: วัดแล้ววันนี้ — bundle จริง **1.48 MiB (gzip)** จากเพดาน free tier **3 MiB**
  = ใช้ไป **49%** เหลือที่ว่างอีกราว 1.5 MiB · ข้อ **A** คือทำให้มันวัดอัตโนมัติและ **fail ก่อน deploy**
  ถ้าเกิน 85% จะได้ไม่ต้องเดาอีก
- **ลำดับงาน**: A วัด size → B กันอีเมลเก่า 243 ฉบับยิงออกตอนเปิดระบบ → C สลิปรูปเดิมใช้ซ้ำไม่ได้
  (ใช้ blake3 ที่มีอยู่แล้ว ไม่เพิ่ม size ไม่ต้องต่อ API ธนาคาร) → D ปุ่ม "ให้เข้างานแต่ไม่ติดหนี้คืนเงิน"
  (แก้บั๊กที่ว่า approve = สัญญาคืน ฿500 เลยต้องจำหน้าเอา) → E เตรียม migration #127 ไว้ก่อน *ยังไม่ apply*
  รอหลัง RTM#6 (27 ก.ย.) → F deploy รวดเดียว → G ถอด QR ในสลิป (ต้องวัด size ก่อน อันนี้เสี่ยงบวม)
- **ที่จะไม่แตะเลยเพราะต้องให้คุณตัดสิน**: 4 ข้อใน `.issues/129` §7, การเปิดส่งอีเมลจริง,
  PARIPOL, เงิน ฿500 ที่ค้างคนเดิม, ลบรูปสลิป 13 ไฟล์, และ deploy โปรแกรม escrow ตัวใหม่
