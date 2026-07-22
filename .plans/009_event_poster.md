# Plan 009 — Event Poster (marketing hero image on `/e/{slug}`)

> **Status**: IMPLEMENTED & MERGED — PR #14 (`3081f71 feat(events): event marketing poster on /e/{slug} (Plan 009)`). Full-stack plumbing landed: D1 migration 0019, domain field on all 4 structs, persistence, R2 storage helpers, Organizer-gated upload/delete handlers, public API field, frontend types + admin form. Serde contract tests pass (7/7, verified 2026-07-08). **AC6/AC8 deviation resolved 2026-07-09**: operator chose option (b) and restored the 3-tier hero fallback (`poster → nft_image_url → Ticket icon`) in `event_hero.rs`, fixing the regression for existing events and matching the past-events listing card. wasm32 `cargo check` + `clippy` clean. The follow-up commit lands on `feature/event_recap`.
> **Type**: feature (full-stack)
> **Priority**: P2 (UX enhancement — better event-page marketing image; no fund/protocol impact)
> **Created**: 2026-06-18
> **Branch**: `feature/event_poster` (off `develop`)
> **Depends on**: nothing blocking. Reuses existing `ASSETS_BUCKET` R2 + `storage.rs` (no new infra).

---

## 1. Problem / Goal

The public event page (`/e/{slug}`) hero image is currently `nft_image_url` — the **NFT badge image** that gets minted into each attendee's cNFT. Organizers want to show a proper **marketing poster** on the event page instead, while keeping the NFT badge image for the actual mint.

Two issues with the status quo:
1. No poster concept exists anywhere (no D1 column, no domain field, no API field, no frontend type).
2. **`nft_image_url` must NOT be overloaded** — it feeds the on-chain NFT mint metadata (`worker/src/claim/mint.rs:640` and `:815` pass `image_url: &event.nft_image_url` into the Helius `mintCompressedNft` JSON-RPC). Overloading it with a poster would corrupt every claimed NFT's image.

### Today's data flow (hero image)

```
D1 column events.nft_image_url        (worker/migrations/0003_events_full_schema.sql:18)
  → D1EventRow.nft_image_url          (worker/src/db/events.rs:53, mapped at :156)
  → EventConfig.nft_image_url         (domain/src/models/event.rs:324)
  → GET /api/public/event/{slug}      (worker/src/handlers/public_event.rs:197)
  → PublicEventData.nft_image_url     (frontend-leptos/src/pages/public_event/types.rs:197)
  → event_hero(nft_image_url)         (frontend-leptos/src/pages/public_event/event_hero.rs:4)
  → <img src=… alt="Event Badge">
```

No `poster` / `cover` / `banner` field exists in any layer.

---

## 2. Scope

### In scope

- New per-event **poster** field, end-to-end: D1 column → domain model → persistence → API → frontend types → hero rendering.
- **R2-backed upload** (primary path) reusing the existing `ASSETS_BUCKET` + `storage.rs` — new `posters/` key prefix + serve route + admin upload endpoint.
- **Fallback hero logic**: prefer `poster_url`, fall back to `nft_image_url`, fall back to the Ticket icon.
- Admin event form: poster file-picker (upload) **plus** a URL override text field (mirrors how `nft_image_url` allows pasted URLs today).

### Out of scope

- Image resizing / transcoding. Upload as-is; the `<img>` + CSS handles display sizing. (Cloudflare Images is a future option, not needed now.)
- Multiple poster variants / responsive sources. One image per event.
- Landing-page event cards (`/`) — they already show `nft_image_url`; leave them on the NFT image for now (poster is for the dedicated event page). Can revisit separately.
- Changing the NFT badge image or mint flow. Untouched.

### Decisions (lock these before coding)

- **D1**: yes, a new `poster_url` column. No reusable field exists; R2/mint coupling makes `nft_image_url` off-limits.
- **Storage**: R2 `ASSETS_BUCKET`, `posters/{event_id}.{ext}` key prefix (mirrors `badges/`, `slips/`).
- **Upload auth gate**: `UserRole::Organizer` minimum (same as `duplicate_event` — `resolve_user_role`).
- **URL convention**: D1 stores the served *path* `/api/storage/posters/{event_id}` (extension-agnostic; `serve_r2_object` tries `.png/.jpg/.webp/.svg`), matching how badge URLs work.
- **Primary + override**: upload-to-R2 is primary; a pasted-URL text field remains as override (organizers with a CDN can paste).

---

## 3. Implementation

Work bottom-up (data layer → API → frontend) so each layer compiles/tests independently.

### 3.1 D1 migration — `worker/migrations/0019_event_poster.sql` (new)

```sql
-- 0019_event_poster.sql
-- Per-event marketing poster URL (served path, e.g. /api/storage/posters/{event_id}).
-- Mirrors the nft_image_url column added by 0003. Empty string = no poster →
-- event page hero falls back to the NFT badge image.
ALTER TABLE events ADD COLUMN poster_url TEXT NOT NULL DEFAULT '';
```

Mirrors `0003_events_full_schema.sql:18` (`nft_image_url`) exactly.

### 3.2 D1 row mapping — `worker/src/db/events.rs`

Add to `D1EventRow` struct (near line 87, after `calendar_subscribe_url`):

```rust
// Column added by migration 0019 (event poster)
pub poster_url: Option<String>,
```

Map it in `to_event_config()` (near line 156, next to `nft_image_url`):

```rust
poster_url: self.poster_url.clone().unwrap_or_default(),
```

`unwrap_or_default()` keeps it forward-compatible if the migration hasn't run yet (same pattern as the 0003 columns).

> **Note**: every `SELECT *` / explicit-column `SELECT` for the events table must include `poster_url`. Audit `worker/src/db/events.rs` and `worker/src/event_store/read.rs` for all event SELECTs and add the column. (Existing 0003 columns are the template for where SELECTs enumerate columns.)

### 3.3 Domain model — `domain/src/models/event.rs`

Add `poster_url: String` (with `#[serde(default)]`) to:
- `EventConfig` (near line 324, next to `nft_image_url`)
- `EventMeta` (near line 239/241, next to `nft_image_url`)
- `CreateEventRequest` (near line 711)
- `UpdateEventRequest` as `Option<String>` (near line 874)

Propagate in:
- `EventConfig::to_meta()` (near line 488) → `poster_url: self.poster_url.clone()`
- `EventConfig::from_global_config()` (near line 601) → add `poster_url: String::new()` param or default; check the call signature/convention used by other optional fields.
- The test fixture `make_event` (near line 1155) → `poster_url: String::new()`.

```rust
/// Marketing poster URL for the event page hero (served path or external URL).
/// Empty = fall back to nft_image_url.
#[serde(default, skip_serializing_if = "String::is_empty")]
pub poster_url: String,
```

### 3.4 Persistence — `worker/src/event_store/write.rs`

- **create** path (near line 230): `poster_url: req.poster_url.trim().to_string()`
- **update** path (near lines 444 and 708): add the optional update, mirroring `nft_image_url`:
  ```rust
  if let Some(ref v) = req.poster_url {
      config.poster_url = v.trim().to_string();
  }
  ```
- **INSERT/UPDATE SQL** for D1: add `poster_url` to the column list + values/params. Audit both the create and update statements.
- `from_global_config` fallback (near line 1035): `poster_url: String::new()`.

### 3.5 R2 storage helpers — `worker/src/storage.rs`

Add a `posters/` prefix + helpers (mirrors `badges/` exactly):

```rust
/// R2 key prefix for event marketing posters.
pub const PREFIX_POSTERS: &str = "posters/";

/// Build an R2 key for an event poster.
pub fn poster_key(event_id: &str, ext: &str) -> String {
    format!("{PREFIX_POSTERS}{event_id}.{ext}")
}
```

Add a serve handler (mirrors `serve_badge`, line 165):

```rust
/// GET /api/storage/posters/{event_id}
///
/// Serves an event marketing poster from R2 (tries common image extensions).
#[worker::send]
pub async fn serve_poster(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
) -> Response {
    // serve_r2_object already tries .png/.jpg/.webp/.svg fallbacks,
    // so pass a base key WITHOUT extension:
    serve_r2_object(&state, &format!("{PREFIX_POSTERS}{event_id}")).await
}
```

`put_bytes()` and `delete()` already exist — reuse them in the upload handler.

### 3.6 Upload endpoint (admin-gated) — new `worker/src/handlers/events/poster.rs`

Mirror `upload_thb_slip_handler` for the bytes→R2 pattern, but:
- **Auth**: `resolve_user_role(&claims.email, &state, Some(&event))` ≥ `UserRole::Organizer` (copy from `duplicate.rs:79-84`).
- **Route**: `POST /api/events/{id}/poster` (multipart image bytes).
- **Logic**:
  1. Resolve event (D1-first/KV fallback).
  2. Role check ≥ Organizer.
  3. Parse multipart → image bytes + detect extension from content-type.
  4. `storage::put_bytes(bucket, &poster_key(&event_id, ext), bytes, content_type)`.
  5. Set `config.poster_url = format!("/api/storage/posters/{event_id}")` and persist via `event_store::write` update.
  6. (Optional but recommended) `storage::delete` any prior poster with a different extension to avoid orphans.
  7. Return the served URL.
- **Size guard**: enforce a max (e.g. 5 MB) before put — R2 free tier + worker memory limits. Reject with `AppError::Validation` if exceeded.

Also add a `DELETE /api/events/{id}/poster` (clears `poster_url` → empty, deletes R2 object). Reuse the same auth gate.

Register both routes in `worker/src/handlers/mod.rs` near line 277 (next to `duplicate`):
```rust
.route("/events/{id}/poster", post(events::poster::upload_poster).delete(events::poster::delete_poster))
```

Register the serve route near line 127 (next to `serve_badge`):
```rust
.route("/storage/posters/{event_id}", get(crate::storage::serve_poster))
```

### 3.7 Public event API — `worker/src/handlers/public_event.rs`

Add to the `get_public_event` JSON response (near line 197):
```rust
"poster_url": config.poster_url,
```

Also add to `list_public_events` KV-fallback json! (near line 58) if poster should appear on cards — **out of scope per §2**, so skip unless revisiting.

### 3.8 Frontend API types — `frontend-leptos/src/pages/public_event/types.rs`

Add to `PublicEventData` (near line 197):
```rust
#[serde(default)]
pub poster_url: String,
```

### 3.9 Frontend hero fallback — `frontend-leptos/src/pages/public_event/event_hero.rs`

Change signature to accept both, prefer poster:
```rust
pub fn event_hero(poster_url: &str, nft_image_url: &str) -> AnyView {
    // Prefer the marketing poster; fall back to the NFT badge image.
    let url = if !poster_url.is_empty() { poster_url } else { nft_image_url };
    let has_image = !url.is_empty();
    if has_image {
        view! {
            <div class="pe-hero">
                <img src=url alt="Event poster" class="pe-hero-img" />
            </div>
        }.into_any()
    } else {
        view! {
            <div class="pe-hero">
                <span><Icon icon=IconName::Ticket class="icon-2xl" /></span>
            </div>
        }.into_any()
    }
}
```

Update the call site in `render_loaded_event` (`page.rs`) to pass both:
```rust
event_hero(&data.poster_url, &data.nft_image_url)
```

### 3.10 Admin event form — `frontend-leptos/src/pages/event_form.rs`

- Add `poster_url: String` to the local form state struct (near line 31) + init from `detail` (near line 237) + include in create/update bodies (near lines 539, 712).
- Add a poster section near the NFT section (around line 1040):
  - File-picker `<input type="file" accept="image/*">` → POST `/api/events/{id}/poster` (multipart) → set `form.poster_url` to the returned served path.
    - (For new/uncreated events, allow pasting a URL until the event exists; upload requires an `event_id`. Mirror whatever the NFT field does for the not-yet-created case.)
  - URL override text field bound to `form.poster_url` (mirrors the `nft_image_url` input at line 1176).
  - Preview `<img>` (mirrors the badge preview at line 1077).
  - "Remove" button → `DELETE /api/events/{id}/poster` + clear field.

> Tip: the existing NFT field is URL-paste-only (`get_self_hosted_nft_urls()` at line 144 + the text input at line 1176). The poster field should add the upload convenience on top.

---

## 4. Testing

### Unit / contract

- Add `poster_url` coverage to `worker/tests/serde_contract.rs` — assert it round-trips on `CreateEventRequest` / `UpdateEventRequest` / `EventConfig` and defaults to `""` when absent. Mirror the existing `nft_image_url` contract tests.
- Domain tests (`event-checkin-domain`): update any `make_event` fixtures; ensure `to_meta()` propagates `poster_url`.

### Manual (devnet / local worker)

1. Create event, upload a poster via the form → confirm `/api/storage/posters/{id}` serves it.
2. Visit `/e/{slug}` → poster shows in hero.
3. Clear the poster (DELETE) → hero falls back to `nft_image_url`.
4. Event with neither poster nor nft_image_url → hero shows the Ticket icon.
5. Organizer role can upload; non-organizer gets 403 on `POST /events/{id}/poster`.
6. Re-upload with a different extension → old object deleted (no orphan).
7. Over-5MB upload → rejected with validation error.

### CI

`.github/workflows/ci.yml` runs `cargo check + clippy (-D warnings) + test` on the workspace (`domain`, `worker`). Frontend (`frontend-leptos`) is excluded from the workspace + gitignored, so **CI does not compile the frontend** — verify the wasm build locally:
```
cargo check --manifest-path frontend-leptos/Cargo.toml --target wasm32-unknown-unknown
```
Also run frontend clippy locally (CI won't catch it; there are pre-existing frontend lints — only fix any *new* ones this change introduces).

---

## 5. Rollout

1. Branch `feature/event_poster` off `develop`.
2. Implement §3.1 → §3.10.
3. Local verify: `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, frontend `cargo check --target wasm32-unknown-unknown`.
4. Push → PR to `develop` (squash merge, repo convention). CI: `check + clippy + test (domain, worker)`.
5. **D1 migration** `0019_event_poster.sql` runs on deploy (Cloudflare D1 migrations via `wrangler d1 migrations apply`). Existing events get `poster_url = ''` → no behavior change (hero falls back to NFT image). Safe, no backfill needed.
6. No `main` hotfix needed — this is a feature, lands on `develop` normally.
7. Deploy is manual/operator-driven (no automated pipeline) — `wrangler deploy` + `wrangler d1 migrations apply`.

---

## 6. Files Touched

| Area | File | Change |
|------|------|--------|
| D1 migration | `worker/migrations/0019_event_poster.sql` | **new** — `ALTER TABLE events ADD COLUMN poster_url TEXT NOT NULL DEFAULT ''` |
| D1 row | `worker/src/db/events.rs` | `D1EventRow.poster_url` + `to_event_config()` map + add to all event SELECTs |
| Domain | `domain/src/models/event.rs` | `poster_url` on `EventConfig`, `EventMeta`, `CreateEventRequest`, `UpdateEventRequest` + `to_meta`, `from_global_config`, fixtures |
| Persistence | `worker/src/event_store/write.rs` | create/update field + SQL column |
| Persistence | `worker/src/event_store/read.rs` | SELECT column audit |
| Storage | `worker/src/storage.rs` | `PREFIX_POSTERS`, `poster_key()`, `serve_poster()` |
| Upload handler | `worker/src/handlers/events/poster.rs` | **new** — `upload_poster` + `delete_poster` (Organizer-gated) |
| Upload handler | `worker/src/handlers/events/mod.rs` | re-export `poster` module |
| Routes | `worker/src/handlers/mod.rs` | register `POST/DELETE /events/{id}/poster` + `GET /storage/posters/{event_id}` |
| Public API | `worker/src/handlers/public_event.rs` | add `poster_url` to `get_public_event` JSON |
| Tests | `worker/tests/serde_contract.rs` | `poster_url` round-trip + default tests |
| FE type | `frontend-leptos/src/pages/public_event/types.rs` | `PublicEventData.poster_url` |
| FE hero | `frontend-leptos/src/pages/public_event/event_hero.rs` | signature `(poster_url, nft_image_url)` + fallback |
| FE page | `frontend-leptos/src/pages/public_event/page.rs` | update `event_hero` call site |
| FE form | `frontend-leptos/src/pages/event_form.rs` | poster field + file-picker + URL override + preview + remove |

~14 files (2 new), mostly mechanical mirroring of the `nft_image_url` plumbing + a small upload handler + the hero fallback.

---

## 7. Acceptance Criteria

- [x] `events` table has `poster_url` column (migration 0019 applied).
      (Verified 2026-07-08: `worker/migrations/0019_event_poster.sql` present —
      `ALTER TABLE events ADD COLUMN poster_url TEXT NOT NULL DEFAULT '';`.)
- [x] `POST /api/events/{id}/poster` uploads to R2 `posters/{id}.{ext}` and persists served URL; Organizer-gated (non-organizer → 403).
      (Code-trace verified 2026-07-08: `worker/src/handlers/events/poster.rs::upload_poster` —
      L57-62 role check `if role < UserRole::Organizer → AppError::Forbidden`;
      L110 `storage::put_bytes(bucket, &poster_key(&event_id, ext), ...)`;
      L114-127 persists `format!("/api/storage/posters/{event_id}")` via `update_event`.)
- [x] `DELETE /api/events/{id}/poster` clears the field + removes R2 object.
      (Code-trace verified 2026-07-08: `delete_poster` L172-179 best-effort deletes all
      extension variants from R2; L182-194 sets `poster_url: Some(String::new())`.)
- [x] `GET /api/storage/posters/{event_id}` serves the image with correct `Content-Type`.
      (Code-trace verified 2026-07-08: route registered at `worker/src/handlers/mod.rs:155-156`
      (`/storage/posters/{event_id}` → `storage::serve_poster`); `serve_poster` at
      `worker/src/storage.rs:245` delegates to `serve_r2_object` which sets Content-Type
      from the stored metadata written by `put_bytes`.)
- [x] `GET /api/public/event/{slug}` includes `poster_url`.
      (Code-trace verified 2026-07-08: `worker/src/handlers/public_event.rs` L198
      `"poster_url": config.poster_url` in `get_public_event` JSON response. Also at L281, L386.)
- [x] `/e/{slug}` hero shows **poster** when set, else **nft_image_url**, else **Ticket icon**.
      (Verified 2026-07-09: operator chose option (b) — restore the 3-tier fallback.
      `event_hero.rs` now reads `poster_url → nft_image_url → Ticket icon`, the second
      param is no longer `_`-prefixed, and `alt` reflects the source ("Event poster"
      vs "Event badge"). wasm32 `cargo check` + `clippy` clean. This also matches the
      past-events listing card (`past_events.rs` L140-144) which already used the same
      `poster → nft` fallback, so the two surfaces are now consistent.)
- [x] NFT mint flow (`claim/mint.rs`) still uses `nft_image_url` — untouched, no regression.
      (Code-trace verified 2026-07-08: `worker/src/claim/mint.rs` L699 and L874 both pass
      `image_url: &event.nft_image_url` into the Helius mint JSON-RPC. No `poster_url`
      reference anywhere in `claim/`. Mint path fully isolated from the poster field.)
- [x] Existing events (no poster) behave exactly as before (fallback to NFT image).
      (Verified 2026-07-09: regression fixed by the AC6 option-(b) change — events with an
      NFT badge image but no poster now render that badge image again instead of the
      Ticket icon, restoring pre-Plan-009 behavior for historical events.)
- [x] Over-5MB upload rejected.
      (Code-trace verified 2026-07-08: `poster.rs` L29 `MAX_POSTER_BYTES = 5 * 1024 * 1024`;
      L68-75 rejects with `AppError::Validation` when `bytes.len() > MAX_POSTER_BYTES`;
      L216 `collect_body_bytes` also caps at collection time via `to_bytes(..., MAX_POSTER_BYTES)`.)
- [x] CI green; frontend wasm compiles; serde contract tests cover `poster_url`.
      (Verified 2026-07-08: `cargo test -p event-checkin-worker --test serde_contract poster_url`
      → 7 passed, 0 failed. Tests cover: create defaults-empty + round-trip; update
      defaults-None + round-trip-set + round-trip-clear; event_config defaults-empty +
      round-trip-non-empty. PR #14 merged → CI was green at merge.)

---

## 8. Risks / Notes

- **`nft_image_url` coupling** (the core risk): do NOT route the poster through `nft_image_url`. The mint reads it directly (`claim/mint.rs:640,815`). Keep them as separate fields.
- **Worker memory on upload**: multipart parsing holds bytes in memory. Cap upload size (5 MB) and reject early. Very large posters would OOM the worker.
- **R2 orphans on re-upload with a different extension**: `serve_r2_object` tries multiple extensions so display is fine, but stale objects accumulate. The upload handler should delete the prior object (best-effort) on replace.
- **Event not yet created**: file upload needs an `event_id`. For the create flow, either (a) require the event to be saved first then upload, or (b) allow URL-paste only until created. Match the existing NFT field's UX for this edge case.
- **Extension-agnostic URL**: storing `/api/storage/posters/{event_id}` (no extension) and relying on `serve_r2_object`'s extension fallback means the served path never changes even if the format changes. Good for cache stability; just ensure `content_type_from_key` covers the formats (it does: jpg/png/webp/svg).
- **Landing-page cards** intentionally left on `nft_image_url` (out of scope). If poster should appear there too later, add `poster_url` to the `list_public_events` response + card rendering.
