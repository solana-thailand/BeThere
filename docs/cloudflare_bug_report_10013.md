# Cloudflare Bug Report: Workers Versions API Error 10013

## Title

`wrangler deploy` fails with 500 / error code 10013 on `POST /versions` — consistently since Wrangler 4.x

## Issue Template: Bug Report

### Environment

| Item | Value |
|------|-------|
| **Wrangler version** | 4.95.0 |
| **OS** | macOS (Apple Silicon) |
| **Worker type** | Rust → WASM (workers-rs), ~3.5 MB uncompressed |
| **Cloudflare account** | `bb8f9ffa91e24d9ce850cbbc4fd45935` |
| **Worker name** | `bethere` |
| **Assets binding** | Yes — `[assets] directory = "../frontend-leptos/dist"` with SPA mode |
| **Bindings** | KV (2), D1 (1), Cron (1) |
| **Node** | v24.10.0 |

### What happened?

`npx wrangler deploy` consistently fails with a 500 error from the Cloudflare API. The error occurs on the `POST /accounts/{account_id}/workers/scripts/{script_id}/versions` endpoint.

The assets upload succeeds (via `assets-upload-session`), the WASM bundle is built successfully, but the final version creation step fails every time.

### Error output

```
✘ [ERROR] A request to the Cloudflare API (/accounts/bb8f9ffa91e24d9ce850cbbc4fd45935/workers/scripts/bethere/versions) failed.

  workers.api.error.directUploadDomainPolicyConflict [code: 10013]
```

### Steps to reproduce

1. Create a Rust worker with `[assets]` binding (SPA mode) and KV + D1 bindings
2. Build WASM bundle (~3.5 MB uncompressed, ~1 MB gzip)
3. Run `npx wrangler deploy`
4. Assets upload session succeeds (JWT obtained, files uploaded)
5. `POST /versions` returns 500 with code 10013

### Workaround

The legacy `PUT /workers/scripts/{name}` API works fine for deploying the worker code. Combined with the `assets-upload-session` API and including the assets JWT in the deployment metadata, the full deployment (worker + static assets) succeeds.

See `worker/deploy.sh` for the complete fallback implementation.

### Additional context

- Error code `10013` maps to `directUploadDomainPolicyConflict` which seems unrelated to the actual issue (there are no custom domain conflicts)
- The error has been **100% reproducible** over multiple days and wrangler versions
- The `PUT` API works fine — this is specifically a `/versions` API bug
- Other developers have reported similar issues: https://github.com/cloudflare/workers-sdk/issues/search?q=10013

### Expected behavior

`wrangler deploy` should succeed on the `/versions` endpoint without returning a 500 error.

---

## Submission URLs

- **New issue**: https://github.com/cloudflare/workers-sdk/issues/new/choose
- **Category**: Bug Report → wrangler
- **Labels**: bug, area:wrangler, area:deploy

## Key Info for Cloudflare Engineers

```
Account ID: bb8f9ffa91e24d9ce850cbbc4fd45935
Script name: bethere
API endpoint: POST /accounts/{id}/workers/scripts/bethere/versions
Error code: 10013 (directUploadDomainPolicyConflict)
HTTP status: 500
Wrangler: 4.95.0
Worker size: ~1 MB gzip (3.5 MB raw WASM)
Assets: Yes (SPA mode, ~14 files including JS, WASM, CSS)
Bindings: 2 KV, 1 D1, 1 Cron
```
