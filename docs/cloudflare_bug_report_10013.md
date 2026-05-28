# Bug Report: `wrangler deploy` fails with 500 / code 10013 on `POST /versions`

## 1 What versions & operating system are you using?

```
System:
    OS: macOS 14.8.7
    CPU: (4) x64 Intel(R) Core(TM) i5-8210Y CPU @ 1.60GHz
    Memory: 89.96 MB / 8.00 GB
    Shell: 5.9 - /bin/zsh
  Binaries:
    Node: 24.10.0 - /Users/ozone/.nvm/versions/node/v24.10.0/bin/node
    Yarn: 1.22.22 - /Users/ozone/.yarn/bin/yarn
    npm: 11.14.1 - /Users/ozone/.nvm/versions/node/v24.10.0/bin/npm
    pnpm: 10.25.0 - /Users/ozone/.nvm/versions/node/v24.10.0/bin/pnpm
  npmPackages:
    @cloudflare/unenv-preset: ^2.16.0 => 2.16.1
    wrangler: ^4.95.0 => 4.95.0
```

Additional context:

| Item | Value |
|------|-------|
| Worker type | Rust → WASM (workers-rs), ~3.5 MB uncompressed, ~1 MB gzip |
| Assets binding | Yes — `[assets] directory = "../frontend-leptos/dist"` with SPA mode (`not_found_handling = "single-page-application"`) |
| Other bindings | KV (2), D1 (1), Cron trigger (1) |

## 2 Please provide a link to a minimal reproduction

I was unable to create a minimal standalone reproduction because the error appears tied to the specific account/worker configuration. The bug is **100% reproducible** on my worker every `wrangler deploy` attempt.

To reproduce with a similar setup:

1. Create a Rust worker with `wasm-bindgen` targeting `bundler`
2. Add `[assets]` binding with SPA mode in `wrangler.toml`
3. Add KV and D1 bindings
4. Build a ~3.5 MB WASM bundle
5. Run `npx wrangler deploy`

The worker is open-source: https://github.com (private repo, can share access if needed)

**Cloudflare account details for log lookup:**

```
Account ID: [REDACTED — will provide privately if needed]
Script name: bethere
```

## 3 Describe the Bug

`npx wrangler deploy` consistently fails with a 500 error from the Cloudflare API on the `POST /accounts/{account_id}/workers/scripts/{script_id}/versions` endpoint.

The failure happens at the **final step** of deployment:
1. ✅ Custom WASM build succeeds
2. ✅ Assets upload session succeeds (`POST /workers/scripts/{name}/assets-upload-session`) — JWT obtained, files uploaded
3. ✅ Worker code bundled
4. ❌ `POST /versions` returns 500 with `workers.api.error.directUploadDomainPolicyConflict [code: 10013]`

This error is **100% reproducible** — it has failed on every single `wrangler deploy` attempt across multiple days and sessions.

**Observed behavior:** Deployment fails with code 10013 every time.

**Expected behavior:** `wrangler deploy` should successfully create a new version via the `/versions` endpoint.

**Notes:**
- Error code `10013` maps to `directUploadDomainPolicyConflict` which seems unrelated — there are no custom domain conflicts in this worker setup
- The legacy `PUT /workers/scripts/{name}` API works fine — this is specifically a `/versions` API bug
- I've built a workaround: upload assets via `assets-upload-session`, get JWT, then deploy via the PUT API with the assets JWT in metadata. Full workaround script: https://gist (can provide if helpful)
- Other developers appear to have hit the same error: https://github.com/cloudflare/workers-sdk/issues?q=10013

## 4 Please provide any relevant error logs

Full error output from `npx wrangler deploy`:

```
⛅️ wrangler 4.95.0
──────────────────

[custom build] Running: mkdir -p build/worker && CARGO_BUILD_JOBS=2 cargo build -p event-checkin-worker --target wasm32-unknown-unknown --release && wasm-bindgen ...
[custom build]     Finished `release` profile [optimized] target(s) in 10.37s

Uploaded bethere
✘ [ERROR] A request to the Cloudflare API (/accounts/{account_id}/workers/scripts/bethere/versions) failed.

  workers.api.error.directUploadDomainPolicyConflict [code: 10013]
```

The build succeeds, the upload succeeds, but the `/versions` call fails every time.
