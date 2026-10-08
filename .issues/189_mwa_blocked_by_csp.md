# 189 · The CSP blocks the Mobile Wallet Adapter, so Android wallets never register

**Status:** deployed (2026-10-09, `event-checkin-8a`): prod version `4d0203f0` at main `71720705` (tree = develop `9d37fb8f`), migrations 0058 + 0059 applied to prod and staging; staging `b5ca12fd`. Earlier: in progress. Fixed on `fix/mwa-self-host` (branch from `develop`
`4d9cf386`), not merged, not deployed. Verified locally with headless Chrome
and an Android UA; not yet verified on a real Android phone with a wallet app.

**Found:** 2026-10-08, prod, headless Chrome with an Android user agent
(session `event-checkin-8a`).

## Repro (prod, before the fix)

Open `https://bethere.solana-thailand.workers.dev/` with an Android UA
(`Mozilla/5.0 (Linux; Android 14; Pixel 8) … Mobile Safari/537.36`). The
console shows:

```
Loading the script 'https://esm.sh/…' violates the following Content Security Policy directive …
[mobile_wallet] Failed to register MWA: TypeError: Failed to fetch dynamically imported module …
```

No MWA wallet (Phantom, Solflare, Seed Vault) is added to
`navigator.wallets`, so the Android wallet picker never offers one. The page
otherwise works, and the error is caught and only logged, so nothing else
surfaced it.

## Cause

`frontend-leptos/js/mobile_wallet.js` loaded the library with
`await import("https://esm.sh/@solana-mobile/wallet-standard-mobile@0.5.3")`.
The CSP `script-src` (`worker/src/middleware/headers.rs`, mirrored in
`frontend-leptos/_headers` `/*`) never listed `https://esm.sh`. A dynamic
`import()` is a script load, so Chrome refuses it. No test compared the
origins that `js/*.js` loads scripts from with `script-src`.

## Fix

Self-host the library; `esm.sh` is not added to the CSP (a third-party module
CDN in `script-src` lets it serve any code under our origin).

- `frontend-leptos/vendor/mwa-wallet-standard-mobile-0.5.3.js`: one
  self-contained ES module, esbuild 0.28.2 (`--bundle --format=esm --minify
  --platform=browser`) of `@solana-mobile/wallet-standard-mobile@0.5.3` and
  the 11 packages it pulls in. 135,666 bytes raw, 35,209 brotli q11. No
  `eval`/`new Function`, no imports, no remote scripts. The Local Network
  Access mitigation (`loopback-network` permission check) is in the bundle.
- `…-0.5.3.LICENSE.txt`: the package list with versions and licences
  (Apache-2.0 and MIT), each licence verbatim, and a recipe that rebuilds the
  file byte-identically (checked: same sha256 from a fresh install).
- `index.html`: two `copy-file` lines next to jsQR's; the staff shell is
  generated from `index.html` (`staff_shell_html.py`), so both shells ship it.
- `mobile_wallet.js`: `MWA_LIB_URL = "/mwa-wallet-standard-mobile-0.5.3.js"`.
  Still a lazy `import()`, Android only; failures are still logged with
  `[mobile_wallet] Failed to register MWA:`.
- `_headers`: `/mwa-*.js` gets the same immutable cache rule as `/jsqr-*.js`.
  Not precompressed through the Worker: Cloudflare's on-the-fly brotli gives
  ~41.9 KB against 35.2 KB at q11, a 6.7 KB saving on a lazy, Android-only
  load, which does not justify another `run_worker_first` glob. The service
  worker handles it like jsQR (stale-while-revalidate; the name is versioned).

## Guards

- `frontend-leptos/tests/js_remote_script_origins.rs`: every origin a
  `js/*.js` file loads a script from (`import(…)`, `.src = …`, `…Script(…)`,
  directly or through a `var NAME =`) must be in `script-src` parsed from
  `headers.rs`. Run against the pre-fix `mobile_wallet.js` it fails with
  `[("mobile_wallet.js", "https://esm.sh")]`. It also pins `MWA_LIB_URL`, the
  `copy-file` lines, and checks the bundle has no eval, no imports, and keeps
  the exports and the LNA check.
- `worker/tests/vendored_mwa_integrity.rs`: the bundle's sha256 matches the
  one in the LICENSE build note, and `_headers` caches it immutably.

## Local verification

`frontend-leptos/build.sh`, then the worker with `wrangler dev --local`
(port 8797, fresh `--persist-to`, `--env-file` of an empty file, so no real
credentials). `GET /mwa-wallet-standard-mobile-0.5.3.js` answered 200,
`Content-Type: text/javascript; charset=utf-8`, `Cache-Control: public,
max-age=31536000, immutable`, with the same CSP as prod. Headless Chrome
(`--headless=new`, fresh profile, Android 14 Pixel 8 UA) on `/` logged:

```
"[mobile_wallet] Loading MWA library from /mwa-wallet-standard-mobile-0.5.3.js"
"[solana_wallet] Wallet Standard wallet registered: Mobile Wallet Adapter"
"[mobile_wallet] MWA registered — Phantom/Solflare/Seed Vault will appear in wallet picker on Android"
```

No CSP violation and no `Failed to register MWA` in the log. The
`Wallet Standard wallet registered` line comes from `solana_wallet.js`
listening for the registry event, so the wallet really reached the registry
(the `MWA registered` line alone is printed whatever `registerMwa` decided).
Not covered: a real wallet app connect (headless Chrome has no MWA wallet to
open), and Chrome on a phone (the UA was spoofed on desktop).

## Verify on a real Android phone after deploy (owner-gated)

1. After the deploy, `curl -sI https://bethere.solana-thailand.workers.dev/mwa-wallet-standard-mobile-0.5.3.js`
   must answer 200 with `content-type: application/javascript` (or
   `text/javascript`) and `cache-control: public, max-age=31536000, immutable`.
   An `application/octet-stream` here is the asset poisoning seen in July; a
   dynamic import refuses it.
2. On an Android phone with Phantom or Solflare installed, open the site in
   Chrome, connect it to a desktop Chrome via `chrome://inspect`, reload, and
   check the console: `[mobile_wallet] MWA registered …`, and no CSP error.
3. Open the wallet sign-in: the installed wallet appears through MWA. Tap it,
   approve in the wallet app, and confirm the app gets the address back
   (sign-in completes).
4. Then set this issue to `deployed` with the version id from
   `npx wrangler deployments list`.
