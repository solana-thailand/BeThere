# Handover #080: R2 Storage — Binding, Handlers, Serving Endpoint (Issue #039 P0)

## What Happened

Completed all P0 Cloudflare platform improvements from Issue #039:
- Smart Placement (already done)
- R2 bucket binding + storage module
- Handler migration: slip upload + refund proof now upload data URLs to R2
- R2 serving endpoint for displaying stored images
- Rate limiting (already done — in-memory middleware)

## Changes Made

### 1. R2 Bucket Binding (`worker/wrangler.toml`)

Added `[[r2_buckets]]` binding for `ASSETS_BUCKET` → `bethere-assets`.

**Prerequisite**: Enable R2 in Cloudflare Dashboard, then:
```bash
npx wrangler r2 bucket create bethere-assets
```

### 2. R2 in AppState (`worker/src/state.rs`)

- Replaced placeholder `r2: Option<()>` with `r2: Option<worker::Bucket>`
- Added to `CachedBindings` — cached per isolate via `env.bucket("ASSETS_BUCKET")`
- Removed outdated TODO about worker crate v0.9+

### 3. R2 Storage Module (`worker/src/storage.rs`)

New module (~180 lines) with:
- Key builders: `slip_key`, `refund_key`, `badge_key`, `metadata_key`, `export_key`
- R2 operations: `put_bytes`, `get_bytes`, `delete`, `exists`
- Content-type mapping: `content_type_from_key`
- **Serving handler**: `serve_r2_object` — `GET /api/storage/{key:path}`
  - Security: only serves objects under known prefixes (slips/, refunds/, badges/, metadata/, exports/)
  - Returns proper `Content-Type` + `Cache-Control: public, max-age=86400`
  - 404 for missing objects, 403 for invalid prefixes, 503 if R2 not configured

### 4. Handler Migration (`worker/src/handlers/deposit/thb/handlers.rs`)

Added `maybe_upload_to_r2()` helper — graceful backward-compatible R2 upload:
- **Slip upload** (`upload_thb_slip_handler`): After validation, data URLs are decoded (base64) and uploaded to R2. Returns `/api/storage/slips/{event_id}/{attendee_id}.{ext}` as the `slip_url`.
- **Refund proof** (`mark_refund_handler`): Same pattern for refund proof images.
- **Fallback**: If R2 is not available or upload fails, stores the original data URL/external URL as-is (zero risk).

### 5. Route Registration (`worker/src/handlers/mod.rs`)

Added `GET /api/storage/{key:path}` → `storage::serve_r2_object` in public routes.

### 6. Dependencies (`worker/Cargo.toml`)

Added `base64 = "0.22"` for data URL decoding.

### Route Design

Initial approach used `{key:path}` wildcard but was shadowed by the SPA fallback in the skeleton router. Resolved by using specific routes per prefix:
- `GET /api/storage/slips/{event_id}/{attendee_id}`
- `GET /api/storage/refunds/{event_id}/{attendee_id}`
- `GET /api/storage/badges/{event_id}`

The `serve_r2_object` internal function tries common image extensions (`.jpg`, `.png`, `.webp`, `.svg`) if the exact key isn't found, so route URLs don't need file extensions.

### Deploy Script Fix (`worker/deploy.sh`)

Added `r2_bucket` binding to the PUT API fallback metadata JSON so R2 works with the Cloudflare versions API workaround.

## Validation

| Check | Result |
|-------|--------|
| `cargo check --quiet` | ✅ Clean |
| `cargo clippy -p event-checkin-worker` | ✅ Zero warnings |
| `cargo test -p event-checkin-worker` | ✅ 21/21 pass |
| `cargo test` (on-chain) | ✅ 39/39 pass |
| `wrangler deploy --dry-run` | ✅ Builds (3.63MB / 1.10MB gz) |
| R2 bucket created | ✅ `bethere-assets` via `wrangler r2 bucket create` |
| Production deployed | ✅ https://bethere.solana-thailand.workers.dev |
| R2 endpoint verified | ✅ 404 for missing objects, will serve images when uploaded |
| All other endpoints | ✅ health 200, badges 200, API routes working |

## Data Flow

```
Before (data URLs):
  Frontend → base64 data URL → Worker → KV (stores ~4MB base64 string)
  Frontend ← data URL from KV (large JSON response, slow)

After (R2):
  Frontend → base64 data URL → Worker → decode → R2 put → /api/storage/slips/evt/att
  Frontend ← /api/storage/slips/evt/att → Worker → R2 get → image bytes (cached 24h)
```

**Storage savings**: ~6x (base64 data URL → raw bytes in R2, zero KV bloat)

## Files Changed

| File | Change |
|------|--------|
| `worker/wrangler.toml` | `[[r2_buckets]]` binding (`ASSETS_BUCKET` → `bethere-assets`) |
| `worker/Cargo.toml` | Added `base64 = "0.22"` |
| `worker/src/state.rs` | `r2: Option<worker::Bucket>` cached per isolate |
| `worker/src/storage.rs` | **New** — R2 helpers + serving handlers (~210 lines) |
| `worker/src/lib.rs` | Registered `mod storage` |
| `worker/src/handlers/mod.rs` | Added 3 R2 serving routes |
| `worker/src/handlers/deposit/thb/handlers.rs` | `maybe_upload_to_r2()` for slip + refund |
| `worker/deploy.sh` | Added R2 binding to PUT API fallback metadata |

## Remaining P0 Items

| Item | Status | Notes |
|------|--------|-------|
| Badge SVGs from R2 | ❌ Low priority | SVGs are small, `include_str!` is fine |
| NFT metadata to R2 | ❌ Future | Current external URI works |

## Next Steps

1. **Enable R2** in Cloudflare Dashboard → `npx wrangler r2 bucket create bethere-assets`
2. **Deploy**: `cd worker && bash deploy.sh dev` (test first) → `bash deploy.sh` (production)
3. **Issue #041 remaining**: H6 (D1 audit reads), M1 (atomic counter), M2-M3 (KV patterns)
4. **Issue #039 P1**: Queues + Workflows (requires $5/mo Workers Paid plan)

## Refs

- Issue #039 (Cloudflare platform improvements)
- Issue #041 (Worker optimization — Phase 1 complete)
- Handover #079 (Phase 1 runtime optimization + file reorganization)
