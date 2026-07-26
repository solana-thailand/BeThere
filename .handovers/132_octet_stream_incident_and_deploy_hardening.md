# Handover 132 — Prod octet-stream Incident + Deploy-Path Hardening

## 0. TL;DR

Shipping the admin-record-slip release (Handover 131) to production triggered a
**live outage**: `bethere.solana-thailand.workers.dev` served `index.html` and the
JS bundle as `application/octet-stream`, so browsers **downloaded a `.dms` file /
showed a blank page** instead of rendering — despite every asset returning HTTP 200.

Root cause was **not** the app code or the deploy method. Cloudflare Workers Assets
dedupes each asset by the **hash of its content**. A *failed-partway* first deploy
(aborted on a wasm-bindgen toolchain mismatch) registered the wasm-bindgen **JS-glue**
object on prod's CDN as `octet-stream`. Because later deploys re-uploaded byte-identical
glue, Cloudflare kept serving the poisoned object — even under a brand-new *filename*
hash (dedup is by *content*, not filename).

Fixed by changing the frontend build tag from a plain Rust `const` into a wasm-bindgen
`inline_js` import, which alters the JS-glue **content** → a fresh, never-poisoned CDN
object. Hardened the deploy path so this class of failure cannot ship silently again.

**Prod:** Version `7b7d0df0` — full release live, all Content-Types correct.
**PRs:** #31 (feature) → #32 (release) → **#33** (deploy hardening) → **#34** (release).
**Branches:** all merged; `develop` == `main` == `bf7ddea`.

---

## 1. What Happened (timeline)

1. **Release shipped** — merged PR #31 (admin record slip, Handover 131) to `develop`,
   then release PR #32 (`develop` → `main`, 23 commits incl. PRs #26–#30: participation_type
   canonicalization, SW `/api/*` cache fix, deposit 401 CTA, admin scroll-listener fix, OAuth
   `next` threading).
2. **First prod deploy FAILED** mid-way: the local `wasm-bindgen` CLI had drifted to `0.2.126`
   while the crate is pinned `0.2.118` (schema must match exactly). The deploy aborted **after**
   the asset-upload step had run — leaving a dangling, poisoned asset object on the CDN.
3. Fixed the toolchain (`cargo install -f wasm-bindgen-cli --version 0.2.118`) and **re-deployed
   successfully** (Version `e65bed23`). Smoke test checked only HTTP status + byte size — **not
   Content-Type** — so the breakage shipped unnoticed.
4. **User reported** the site downloading a `8cylkdM4.dms` file. Confirmed `/` served
   `content-type: application/octet-stream` (body was the real HTML).
5. **Restored prod immediately** via `wrangler rollback` to the last-good Version `a0cae457`.
6. **Diagnosed** (see §2). Two failed re-deploy attempts (`f4a6b140`, `2c439552`) proved a
   filename-hash change alone does NOT fix it — each rolled back.
7. **Fixed for real** (PR #33): `inline_js` glue-bust → new glue content hash `c94bdce`; deployed
   Version `7b7d0df0`, the built-in Content-Type verification passed, feature live.
8. **Made it reproducible**: merged #33 → `develop`, released #34 → `main` so a future
   `main` deploy rebuilds the *non-poisoned* glue.

---

## 2. Root cause (the subtle part)

**Cloudflare Workers Assets is content-addressed.** Each file is stored/served by a hash of
its bytes. Two builds whose `event-checkin-frontend-<hash>.js` glue is byte-identical map to
ONE stored object (same CDN `etag`), regardless of the filename.

- The glue's **filename** hash tracks the WASM binary. The glue's **content** only changes when
  the wasm-bindgen *interface* (an import/export or JS snippet) changes.
- Bumping a plain Rust const / adding a `leptos::logging::log!` changed the WASM → new filename
  hash (`6005e6b` → `734b9ae`) but left the glue bytes identical → **same** poisoned object.

**Proof it was environment/object-specific, not deploy-method:**
- Deployed the *identical* assets to **staging** (`bethere-staging`) via the *same* standard
  `wrangler deploy` → served `text/javascript` correctly.
- Same file (identical `etag 172c9bc7…`) served `octet-stream` on prod, `text/javascript` on
  staging. Same account, platform, wrangler (4.99), config — only the worker differs.
- Prod's *older* glue `a653e14b.js` (different content → different etag) served correctly.

**How the object got poisoned:** the failed first deploy fell into `deploy.sh`'s PUT-API
fallback, whose manual multipart asset upload sends each part as `Content-Type:
application/octet-stream`. That upload ran before the deploy aborted, registering the glue
content-object as octet-stream on prod. Staging never had a failed-partway deploy → clean.

---

## 3. Changes (PR #33 → #34)

### `frontend-leptos/src/lib.rs` — glue-bust
Build tag is now a wasm-bindgen `inline_js` import instead of a plain const:
```rust
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function __bethere_build_tag() { return '2026-07-26-1'; }")]
extern "C" { fn __bethere_build_tag() -> String; }
```
Called once in `App()` (`leptos::logging::log!("[bethere] frontend build {}", __bethere_build_tag())`).
Adds a binding to the JS glue, so **bumping `BUILD_TAG` changes the glue's content hash** →
a fresh CDN object that was never poisoned. Also logs the live build id to the console for
stale-cache diagnosis. New glue hash `c94bdce` (md5 changed from the poisoned build's).

### `worker/deploy.sh` — automated Content-Type verification + wasm-bindgen guard
- `verify_content_types()` — after **both** the standard and PUT-fallback deploy paths, curls the
  deployed origin and **fails the deploy** (exit 1) if `/` or the hashed JS bundle serves
  octet-stream, with rollback + `BUILD_TAG`-bump remediation. Retries to ride out edge
  propagation. (The prior status-only check let the breakage ship.)
- `check_wasm_bindgen_version()` — aborts **before any build** if the installed CLI ≠ the
  `Cargo.lock`-pinned crate, printing the exact `cargo install` fix. This also **prevents the
  failed-partway deploy** that poisoned the object in the first place.

### `.github/workflows/ci.yml` — `worker wasm32 build` job
CI previously checked only the native target. New job builds the worker to
`wasm32-unknown-unknown --locked --release`, catching wasm-only breakage before deploy.

---

## 4. Plan / Code / Test — Verification

```
CI (PR #33, #34): build-test (domain, worker) PASS + worker wasm32 build PASS
Frontend: cargo check PASS (inline_js change)
deploy.sh: bash -n PASS; verify_content_types tested both paths (match → ✅, octet-stream → fail+exit1)
wasm-bindgen guard tested: match → exit 0; mismatch (fake CLI on PATH) → fix message + exit 1
```

**Prod smoke (Version `7b7d0df0`):**
```
/                                 → text/html          ✅
event-checkin-frontend-c94bdce.js → text/javascript    ✅
…_bg.wasm                          → application/wasm    ✅
sw.js / style.css / manifest.json → correct             ✅
/api/health                        → 200
/api/deposit/thb/admin-upload      → 401 (feature live, auth-gated)
```
Browser walkthrough of the admin-record-slip flow: **done by the user** (feature confirmed working).

---

## 5. Reflection / Struggles / Solved

- **My two real mistakes:** (1) the post-deploy smoke test checked HTTP status + byte size but
  **not Content-Type**, so the octet-stream shipped; (2) I continued past a *failed-partway*
  first deploy without verifying the CDN/asset state was clean.
- **Dead-end that cost time:** a staging repro showed the standard `wrangler deploy` path serving
  the same assets correctly, which (correctly) exonerated the deploy method but (incorrectly) led
  me to a "transient edge-cache" theory. Only the identical-`etag`-different-worker comparison
  nailed it as per-content-object poisoning.
- **Why filename-hash busting failed:** dedup is by content. Had to change the glue *content*
  (interface), not just the WASM. `inline_js` is the reliable lever (a plain const/log is not —
  the optimizer can leave the glue identical).

---

## 6. Remain Work

### Blocking — none. Release is live and reproducible from `main`.

### Optional / follow-ups
1. **`deploy.sh` PUT-fallback Content-Type** — the fallback still uploads multipart parts as
   `application/octet-stream` (the suspected original poison vector). The wasm-bindgen guard now
   prevents the failed-partway path that used it, and `verify_content_types()` catches the symptom,
   so this is defense-in-depth, not urgent. Relates to `.issues/057_headers_not_applied_put_api_fallback.md`.
2. **CI does not build the frontend** — `frontend-leptos` is excluded from the cargo workspace, so
   `cargo check --workspace` never validates it. The `lib.rs` glue-bust was compile-checked locally
   only. A frontend CI job would close this gap.
3. **PR #18** (`docs/claude-tool-calling`) — open since 2026-07-12; decide merge/close.
4. Larger backlog: `#052` (1024-line refactor), `#053` (KV→D1), `#054` (Dioxus), `#060` (event series nav).

---

## 7. Issues / Refs

- **PR #31** admin record slip (feature) · **#32** release to main · **#33** deploy hardening ·
  **#34** release to main
- **Handover 131** — the admin-record-slip feature this release shipped
- **`.issues/057`** — PUT-API fallback headers (related to the poison vector)
- Memory: `cloudflare-assets-content-type-poisoning`, `deploy-smoke-test-content-type`,
  `bethere-deploy-and-rollback`

---

## 8. Key invariants for future deploys

1. **Always smoke-test `Content-Type`, not just HTTP 200.** `deploy.sh` now does this
   automatically — do not remove `verify_content_types()`.
2. **Never continue past a failed-partway deploy** without confirming the CDN/asset state is clean;
   a dangling asset upload can poison a content object.
3. **To bust a poisoned glue: bump `BUILD_TAG` in `frontend-leptos/src/lib.rs`** (an `inline_js`
   change), rebuild, redeploy. A filename-hash change alone is NOT enough — Cloudflare dedupes by
   content.
4. **Keep the wasm-bindgen CLI == the `Cargo.lock` crate** (currently `0.2.118`). The guard enforces
   it; the fix is `cargo install -f wasm-bindgen-cli --version <locked>`.
5. **Last-known-good prod version for rollback:** update this per release. As of this handover the
   live good version is `7b7d0df0`; the pre-incident good version was `a0cae457`.
