# Plan 029: ISO 27001 remediation and dependency hygiene

**Created:** 2026-09-23 · **Source:** `docs/iso27001_gap_assessment.md`
**Rule:** nothing here changes auth, deploy or data flows before RTM#6
(2026-09-27) unless it is a fix for a verified defect.

## 1. Done 2026-09-23 (`develop`, not deployed)

- [x] Open redirects on the OAuth `state` and `/login?next=` (`.issues/143` A)
- [x] Random SIWS nonce; constant-time webhook bearer compares
- [x] Subresource Integrity on the CDN scripts (`@solana/web3.js` 1.95.3,
      jsQR 1.4.0 `dist/jsQR.js`). Hashes were checked against the npm tarballs;
      headless Chrome loads them and blocks a mismatch.
- [x] OSV scan of all three lockfiles: 0 exploitable advisories. Patch bumps
      for the two notices that have a fixed release: `anyhow` 1.0.102 → 1.0.104
      (RUSTSEC-2026-0190) and `event-listener` 5.4.1 → 5.4.2
      (RUSTSEC-2026-0221). Re-scan after the bumps: worker lockfile 0 hits;
      frontend 2 "unmaintained" notices on build-time proc-macros only
      (`paste`, `proc-macro-error2`, both pulled in by leptos), nothing
      that ships in the wasm as runtime code.
- [x] Public ticket page no longer bounces signed-out attendees to /login (`.issues/142`)

## 2. Ungated, small (S)

- [ ] CI: add a dependency advisory gate. `cargo-deny` (advisories + licenses),
      or the OSV querybatch script used for the 2026-09-23 scan, run on
      `Cargo.lock`, `frontend-leptos/Cargo.lock` and
      `bethere-escrow/Cargo.lock`. Ignore list: the escrow's
      Solana-toolchain transitive notices (`bincode`, `derivative`,
      `libsecp256k1`, `rand 0.7`, `paste`), which move only with the SBPF
      toolchain decision (`[[sbpf-v3-gate-still-closed]]`).
- [ ] CI: gitleaks secret scan. Pin third-party actions to commit SHAs.
- [ ] `.github/CODEOWNERS`; `SECURITY.md` names a real contact.
- [ ] Update the secrets list in the `worker/wrangler.toml` comments. It
      misses `WEBHOOK_SECRET`, `CROSSMINT_API_KEY`, `GITHUB_CLIENT_*`,
      `TELEGRAM_BOT_TOKEN` and `SLACK_WEBHOOK_URL`.
- [ ] Magic-byte validation on slip uploads (8.7).
- [ ] Alert on 401 and 429 spikes through the existing Slack alert (8.16).
- [ ] Swap the real email in `worker/scripts/seed_dev.sh` for `example.test` (8.33).
- [ ] Self-host jsQR: same-origin, precompressed, and no CDN on venue Wi-Fi.
      This would let the CSP drop `cdn.jsdelivr.net`.

## 3. After RTM#6

- [ ] `.issues/143` Part B: browser-bound OAuth state (login CSRF).
- [ ] Dependency bumps that need a coordinated toolchain move:
  - `wasm-bindgen` 0.2.118 → 0.2.128 (with `js-sys`/`web-sys` 0.3.105) needs
    the CI `wasm-bindgen-cli@0.2.118` pins (`ci.yml` lines 120 and 229) and
    the local CLI moved together, because `deploy.sh` refuses a mismatch;
  - `worker` 0.8.1 → 0.8.6;
  - `leptos` 0.8.19 → 0.8.20 and `leptos_router` 0.8.13 → 0.8.15;
  - `uuid` 1.23 → 1.26, plus the `serde`/`serde_json`/`chrono` patches.

  Run `cargo update` per lockfile, then the full gates, then a staging soak.
  Majors to evaluate: `base64` 0.23 and `gloo-timers` 0.4.
- [ ] `wrangler` 4.99.0 → 4.137.0 (`worker/package.json`). This is the deploy
      tool, so bump it on a quiet day and re-check the 10013/PUT fallback
      path first.
- [ ] Retention: a purge or anonymise policy for D1 `attendees`/`contacts`,
      and an R2 lifecycle rule for slips (8.10; `.issues/126`).
- [ ] Field-level encryption for bank account/name. Include a classification
      scheme (5.12, 8.24).
- [ ] Scheduled, encrypted D1 export with an offsite copy and a restore drill.
      Covers R2 and KV too (5.30, 8.13).
- [ ] CSP: remove `script-src 'unsafe-inline'` (8.23).
- [ ] A separate log-fingerprint key, instead of reusing `JWT_SECRET` (8.11).
      Move capability tokens out of URL paths (`.issues/071`).

## 4. Owner decisions (gated)

- [ ] Storage and disposal rule for the plaintext prod dumps on the
      workstation: 11 files in `backups/` and `worker/backups/`, plus
      `exports/`, all mode 644. Suggest `chmod 600` now and deleting anything
      older than the rollback window. **Nothing has been deleted.**
- [ ] Second approver for prod deploys, or deploys run from CI off `main` (5.3, 8.32).
- [ ] MFA for super-admin (Cloudflare Access on `/admin`, or Google Workspace
      2SV enforcement) (8.2).
- [ ] ISMS documents (policy, risk register, asset/supplier registers, PDPA
      incident and breach procedure, RoPA, DSAR export), and whether to pursue
      certification at all. An external audit is L effort and cost.
