# Handover 105 — Commit/PR Sweep of Uncommitted Working Tree (R2 Fix + Participation Override + ImageLightbox)

> **Branch**: `feature/r2_lightbox_admin` (4 commits ahead of `develop`, pushed to `origin`)
> **Status**: ✅ **Pushed** + **PR #15 open** against `develop`. **NOT deployed** — prod-affecting, awaiting operator go-ahead + D1 backup.
> **Commits**: `a32215d → e94f6ec` on `feature/r2_lightbox_admin`
> **Predecessor**: handover #104 (audience aggregation + sync repair)
> **Created**: 2026-06-19

---

## 1. What Happened

Coming out of handover #104 / plan #010, the working tree on `main` held **1,330 uncommitted
lines across 18 files** from a prior session — at risk of loss and not covered by any plan or
handover. This session's job was to (a) figure out what those lines actually were, (b) validate
they compile, (c) split them into logical commits, and (d) push + open a PR. No deploy.

The 1,330 lines turned out to be **three independent features + one docs file**, none of them
broken. All were committed cleanly (every file maps to exactly one commit — no `git add -p`
needed) and both sides compile.

**Key correction during the session:** an initial `grep` for
`fn update_participation_type` in `worker/src/sheets/**` returned no matches, which (briefly)
suggested the new participation-override handler referenced two non-existent sheet functions and
wouldn't compile. That was a **false negative from the grep tool's index** — ripgrep confirmed
both `sheets::bg_sync::update_participation_type` (L471-510) and
`sheets::write::append::update_participation_type` (L339) exist with matching signatures. Always
cross-check a surprising "not found" with `rg` / `read_file` before raising an alarm.

---

## 2. Root Cause / Analysis of the Three Features

### 2.1 R2 storage bug fix (commit `a32215d`) — 🟠 medium, likely prod-impacting

The `worker` 0.8.x crate serializes `null` into R2 option fields that Cloudflare rejects, which
**silently broke both R2 reads and writes in prod** before this change:

- **`put`**: `HttpMetadata.cache_expiry = None` serializes to JS `null`, which R2 rejects
  ("cacheExpiry ... not of type 'date'"). The `PutOptionsBuilder` also unconditionally sends an
  `md5` field whose value was `null` (no checksum set) — also rejected.
- **`get`**: `Bucket::get` always sends `{ onlyIf: null, range: null }`; the `range: null` makes
  R2 throw **internal error 10001 on every get, even for missing keys**.

The fix:
- `put_bytes` no longer sends `http_metadata`; instead computes a real **MD5 digest** (new `md-5`
  dep) so the unconditional `md5` field is valid. `content_type` is kept as a logging arg only
  (serving derives it from the key extension).
- `get_bytes` now takes the **raw `js_sys::Object` R2 handle** (`state.r2_raw`) and calls
  `bucket.get(key)` with no options object, materializing the stream via
  `new Response(body).arrayBuffer()` (new `web-sys` dep with only the `Response` feature).
- `serve_r2_object` now **distinguishes 404 (genuinely missing) from 500 (R2 read failure)** and
  runs an independent `head`/`exists` probe to detect a broken get builder (logs "HEAD says object
  EXISTS but get returned nothing").

> Implication: poster loading on `/e/{slug}` and THB slip uploads were likely **broken in prod**
> before this commit. Highest-impact change in the batch — should be deployed + verified first.

### 2.2 Manual participation_type override (commit `016be49`) — 🟠 medium

Fills the gap left by the deposit-deadline auto-switch (which only writes the Sheet, not D1):
an attendee who chose deposit/in-person, never returned, and later confirms out-of-band that
they'll attend online had to be fixed by hand-editing the Google Sheet. Now:

- **`PATCH /api/attendee/{id}/participation-type`** — validates against
  `["In-Person", "Online"]`, writes **both** the Sheet (col I, via detached `wait_until`) and
  D1, and logs a new `ParticipationTypeChanged` audit entry with old/new values.
- New D1 helper `db::attendees::set_participation_type`.
- New `AuditAction::ParticipationTypeChanged` variant.
- Frontend: `api::update_participation_type` client + a generic `api_patch_json` HTTP helper +
  admin toggle button with a per-row spinner (`switching_ids`).
- Bundled into the same commit: the small admin sidebar **"View Google Sheet" link**
  (`crate::utils::google_sheet_url`), which shares `admin.rs` with the toggle.

### 2.3 Shared ImageLightbox + latent-space background (commit `7d3c07f`) — 🟡 low (UI only)

- New **`ImageLightbox`** component: reactive, always-in-DOM fullscreen overlay, visibility
  toggled via a `class:is-visible` on `opacity`/`visibility` (this is the **fade-out smoothness
  fix** — `style:display:none` killed the close animation). Includes the
  `min-width:0; min-height:0` **flexbox overflow fix** (image was bursting out of the card
  because flex children default to `min-width:auto`).
- New **`LightboxImage`** wrapper: self-contained thumbnail + lightbox for static images.
- **QR overlay, event poster, and slip preview all delegate** to the shared component (DRY; one
  click-to-dismiss + Escape pattern).
- New **"latent space" nebula/starfield animated background** on `/e/{slug}` (nebula mesh, four
  color orbs, aurora sweep, twinkling stars — GPU-friendly transform/opacity compositing).

### 2.4 Docs (commit `e94f6ec`) — 🟢

Adds `.plans/010_free_tier_optimization_consolidated.md` (the consolidated DONE-vs-PENDING
snapshot + data-safety risk framework that motivated this session).

---

## 3. Changes Made

- Discovered the 1,330 uncommitted lines were coherent and compiling (not broken half-finished work).
- Created `feature/r2_lightbox_admin` off `origin/develop` (gitflow). NB: `develop/feature/...`
  branch naming is **impossible** here — git rejects nesting under `refs/heads/develop` because
  `develop` is already a branch. Used `feature/...` to match the team's actual convention
  (`feature/audience_aggregation`, `feature/kv_write_elimination`).
- Committed in 4 logical chunks (see §9), each self-contained and independently compilable.
- Pushed + opened **PR #15** (`feature/r2_lightbox_admin` → `develop`).
- No deploy (prod data-safety rule — operator's call after D1 backup).

---

## 4. Files Modified (by commit)

**`a32215d` fix(worker): R2 null-serialization bugs**
- `worker/Cargo.toml` (+`md-5`, +`web-sys`/Response)
- `worker/src/storage.rs` (`put_bytes` MD5, `get_bytes` raw-handle, 404/500 split, head probe)
- `worker/src/state.rs` (`r2_raw: Option<js_sys::Object>` field + cache init)
- `worker/src/auth.rs` (test fixture: `r2_raw: None`)
- `Cargo.lock` (md-5 transitive deps only: block-buffer, crypto-common, digest, generic-array, md-5, typenum, version_check)

**`016be49` feat(attendee): participation_type override + sheet link**
- `worker/src/handlers/attendee.rs` (new `update_participation_type` handler)
- `worker/src/db/attendees.rs` (new `set_participation_type`; `upsert_attendee_full` reformat)
- `worker/src/audit_store.rs` (`ParticipationTypeChanged` variant)
- `worker/src/handlers/mod.rs` (new PATCH route)
- `frontend-leptos/src/api/attendee.rs` (`update_participation_type` client + `ParticipationTypeUpdate`)
- `frontend-leptos/src/api/mod.rs` (`api_patch_json` + `HttpMethod::Patch`)
- `frontend-leptos/src/pages/admin.rs` (toggle button + `switching_ids` + sheet link)

**`7d3c07f` feat(frontend): ImageLightbox + background**
- `frontend-leptos/src/components.rs` (`ImageLightbox`, `LightboxImage`, `LightboxSizing`)
- `frontend-leptos/src/pages/ticket/qr_section.rs` (delegate to `ImageLightbox`)
- `frontend-leptos/src/pages/public_event/event_hero.rs` (poster lightbox)
- `frontend-leptos/src/pages/public_event/page.rs` (latent-space bg layer)
- `frontend-leptos/src/pages/deposit/thb_payment.rs` (slip lightbox)
- `frontend-leptos/style.css` (lightbox styles + smoothness/overflow fix + `pe-bg-anim`)

**`e94f6ec` docs(plan): #010**
- `.plans/010_free_tier_optimization_consolidated.md`

---

## 5. Reflections

### What went well
- Caught that plan #010's P0.1 ("KV write elimination on branch, not deployed") was **stale** —
  the fix (`ebd0e97`, `404aeb7`) is already in `main` history; the `feature/kv_write_elimination`
  branch no longer exists. Saved a phantom "deploy the branch" task.
- Validated **before** committing: `cargo clippy -p worker --quiet` and
  `cargo check --target wasm32-unknown-unknown -p event-checkin-frontend` both EXIT 0. No commit
  was made on un-validated code.
- Clean commit split with no `git add -p` — every file maps to exactly one commit because the two
  cross-cutting files (`style.css`, `admin.rs`) were assigned to their dominant feature rather than
  split. The participation-toggle and the google-sheet link share `admin.rs`, so they were bundled
  (both admin-facing).

### What was struggled with
- A **false-negative `grep`** claimed the participation handler called non-existent sheet
  functions. Nearly raised a "worker won't compile" alarm. Corrected only after ripgrep +
  `read_file` outline confirmed both functions exist. Lesson: the `grep` tool's index can miss
  committed files — cross-check surprising "not found" results before alarming.

### What was solved
- Identified the R2 fix as the **highest-impact + highest-urgency** item in the batch (prod
  poster/slip access was likely broken), and ordered it as commit 1 so it can be cherry-picked
  to a hotfix independently of the rest.

---

## 6. Remain Work

### Immediate (operator action — all prod-touching or interactive)
- [ ] **D1 backup before any deploy**: `npx wrangler d1 export bethere-db --output backup-pre-demo.sql`
- [ ] **Decide PR strategy**: merge PR #15 as one, or cherry-pick `a32215d` (R2 fix) to a hotfix
      for fast prod turnaround. Commands in §7.
- [ ] **Deploy + verify the R2 fix** (P0): after deploy, confirm a poster loads on `/e/{slug}`
      (R2 read) and a THB slip upload succeeds (R2 write). If either still fails, check
      `wrangler tail` for the "HEAD says EXISTS but get returned nothing" diagnostic log.
- [ ] **Verify participation override**: flip one attendee In-Person ⇄ Online; confirm Sheet
      col I updates, D1 row updates, and a `ParticipationTypeChanged` audit entry appears.
- [ ] **Verify lightbox + background** on `/e/{slug}`: open/close poster, slip preview, and QR
      overlay; confirm fade animation + no image overflow; confirm latent-space background renders.
- [ ] **P0.2 CPU-time measurement** (from plan #010 — still genuinely undone):
      `npx wrangler tail`, then exercise `/api/checkin`, `/api/claim/{token}`,
      `/api/deposit/usdc/confirm`, `/api/deposit/refund`; read `cpuTime` per invocation
      (>7ms = flag, >10ms = already failing in prod).

### Carried over (unrelated, not blocking)
- [ ] KV write usage re-check: `python3 scripts/diag_kv_usage.py --days 3` (P0.1 code is already
      in `main`; this just confirms prod writes dropped to ~0/day).
- [ ] Handover #104's orphan-toast visual verify on audience export.

---

## 7. How to Dev/Test

### Checkout + build the branch
```bash
git fetch origin
git checkout feature/r2_lightbox_admin   # or origin/develop + cherry-pick a32215d for hotfix
cargo clippy -p worker --quiet            # EXIT 0 expected (do NOT add --all-targets; see #104)
cd frontend-leptos && cargo check --target wasm32-unknown-unknown -p event-checkin-frontend
```

### Hotfix the R2 fix alone (optional, for fast prod turnaround)
```bash
git checkout -b hotfix/r2-storage-fix origin/develop
git cherry-pick a32215d
git push -u origin hotfix/r2-storage-fix
gh pr create --base develop --head hotfix/r2-storage-fix \
  --title "fix(worker): R2 null-serialization bugs (hotfix)" --body "Cherry-pick of a32215d from PR #15."
```

### Deploy (with D1 backup first)
```bash
# 1. Backup
npx wrangler d1 export bethere-db --output backup-$(date +%Y%m%d).sql

# 2. Rebuild frontend cleanly (avoid stale-dist trap from #104)
cd frontend-leptos
cargo clean -p event-checkin-frontend
bash build.sh
rg "ParticipationTypeChanged" dist/*_bg.wasm   # sanity: fresh build contains latest code

# 3. Deploy
cd ../worker && bash deploy.sh

# 4. Verify on prod
# - GET /e/{slug} → poster renders (R2 read)
# - THB slip upload → succeeds (R2 write)
# - PATCH /api/attendee/{id}/participation-type → 200, Sheet col I + D1 updated
```

### Clippy gate (use the verified command)
```bash
cargo clippy -p worker --quiet
# NOT --all-targets — fails with wasm_bindgen_test errors in worker-0.8.1 dep test code (#104)
```

---

## 8. Issues Ref

- Predecessor: handover #104 (audience aggregation + sync repair)
- Related (R2 binding): handover #080 (R2 binding + rate limiting)
- Related (KV/D1 migration tail): handover #088, #089, #092, #093, #100
- Plan: #010 (free-tier optimization consolidated)
- Branch: `feature/r2_lightbox_admin` (4 commits: `a32215d → e94f6ec`, pushed to `origin`)
- **PR**: #15 — `feature/r2_lightbox_admin` → `develop` (open)
- **Prod URL**: https://bethere.solana-thailand.workers.dev
- Remote: `git@github.com:solana-thailand/BeThere.git`
- Production state: **unchanged** — branch pushed + PR open, **nothing deployed yet**.

---

## 9. Commit Plan

Commits on `feature/r2_lightbox_admin` (in order, oldest first):

1. `a32215d fix(worker): bypass worker 0.8.x R2 get/put null-serialization bugs`
2. `016be49 feat(attendee): manual participation_type override + admin sheet link`
3. `7d3c07f feat(frontend): shared ImageLightbox + latent-space event background`
4. `e94f6ec docs(plan): add #010 free-tier optimization consolidated view`

**Status**:
- ✅ Pushed: `git push -u origin feature/r2_lightbox_admin`
- ✅ PR #15 opened: `feature/r2_lightbox_admin` → `develop` (gitflow)
- ✅ Validated: worker clippy EXIT 0, frontend wasm check EXIT 0
- ⏳ Operator: D1 backup → deploy → verify R2 (poster/slip) + participation override + lightbox
- ⏳ Operator: P0.2 `wrangler tail` CPU-time measurement (interactive, can't be automated here)
