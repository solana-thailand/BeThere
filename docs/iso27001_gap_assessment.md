# ISO/IEC 27001:2022 Annex A: gap assessment

**Date:** 2026-09-23 · **Scope:** the BeThere check-in system:
- the Cloudflare Worker (`worker/`) and its D1, KV and R2 data;
- the Leptos wasm frontend (`frontend-leptos/`);
- the Solana escrow (`bethere-escrow/`);
- the operator workstation that holds deploy credentials and D1 backups.

**Status:** this is a self-assessment, not a certification. The system is not
ISO 27001 certified, and nothing here claims it is. Certification needs a
documented ISMS (clauses 4–10: scope, risk assessment, Statement of
Applicability, internal audit, management review) and an accredited external
auditor. None of that exists yet. This document maps the technical and
operational controls that already exist to Annex A and ranks the gaps.

**Method:**
1. A read-only sweep of the repo (code, docs, `.issues/`, CI).
2. Every security-relevant finding below was re-checked against the source
   before being written down. "Verified" in the tables means that re-check.
3. Remediation items are tracked in `.plans/029_iso27001_remediation.md`.

## Summary

| Area | Posture |
|---|---|
| Secure development (8.25–8.29) | **Strong.** CI runs fmt, clippy `-D warnings`, the worker/domain/frontend test suites, Playwright e2e, escrow SVM tests, ShellCheck and Python security tests, plus about 40 source-grep guard tests (`worker/tests/*_guard.rs`) that pin past defects. |
| Logging and masking (8.11, 8.15) | **Good.** The D1 `audit_log` (`worker/src/audit_store.rs`) has a KV fallback. Keyed log fingerprints stand in for raw identities, and `log_pii_guard.rs`/`error_body_guard.rs` guard them. |
| Environment separation (8.31) | **Good.** `[env.staging]` in `worker/wrangler.toml` has its own D1, KV, R2 and rate limits. |
| Network and headers (8.20–8.23) | **Good.** HSTS, CSP, XFO DENY, COOP and CORP are set on every response (`worker/src/middleware/headers.rs`). Cloudflare rate limits plus an in-isolate fallback cover the auth routes. |
| Vulnerability management (8.8) | **Gap.** CI has no dependency advisory scan and no secret scanning. |
| ISMS documentation (5.1–5.10) | **Gap.** No policy set, risk register, asset register or supplier register. |
| Continuity and backup (5.30, 8.13) | **Gap.** Backups are manual, with no restore drill. R2 and KV are not backed up. |
| Retention (8.10) | **Partial.** A KV/audit/deposit purge cron exists. D1 attendee/contact rows and R2 slip images have no retention limit. |
| Segregation of duties (5.3) | **Gap.** One operator develops, approves and deploys from a laptop. |

## Fixed in this pass (2026-09-23, `develop`, not deployed)

| Control | Finding | Fix |
|---|---|---|
| 8.5, 8.26 | **Open redirect after Google sign-in.** `GET /api/auth/url?redirect=` put any string into the OAuth `state`, and `auth_callback` redirected to it verbatim. Anyone could craft a real Google sign-in link that landed the victim on an arbitrary site. Verified. | `event_checkin_domain::validation::safe_redirect_path` accepts only a same-origin path, and the callback applies it at the redirect sink. |
| 8.5, 8.26 | **Open redirect and script URL on `/login?next=`.** After wallet sign-in the page ran `location.href = next` unvalidated, so `next=javascript:…` would run in this origin. The already-signed-in redirect and the Google `state` came from the same parameter. Verified. | `pages/login.rs` filters `next` once through the same helper, which covers all three sinks. |
| 8.5 | **Predictable SIWS nonce.** The nonce was `now ^ constant`, so the whole challenge message was predictable. A victim phished into signing a future challenge could be impersonated at that time. Verified. | Random nonce: 128 bits from `crypto.getRandomValues` (`crypto::random_hex`), failing closed. |
| 8.24 | **Webhook bearer compared with `==`.** Two handlers did this; not constant-time. Verified. | One `crypto::constant_time_eq`, shared with JWT verification. |
| 8.28, 8.30 | **CDN scripts loaded without Subresource Integrity.** These are `@solana/web3.js`, which builds the transactions the wallet signs, and jsQR. Verified. | Pinned versions with `integrity` + `crossorigin`. Hashes were checked against the npm tarballs, and a headless Chrome run showed that a mismatched hash is blocked. |
| 5.34 | The public ticket page redirected signed-out attendees to /login (`.issues/142`). A reliability bug with the same root cause as `.issues/099`. | Non-redirecting read helper plus a guard test. |

## Annex A mapping

### 5 Organizational

| Control | Evidence | Gap | Effort |
|---|---|---|---|
| 5.1 Policies | `SECURITY.md` (disclosure process only) | No information-security, acceptable-use or access-control policy. `SECURITY.md` names no contact. | S |
| 5.2/5.3 Roles, segregation | `docs/operator-handover.md` | One person develops, approves and deploys. No second approver for prod. | S (doc) / M (process) |
| 5.9 Asset inventory | Bindings in `worker/wrangler.toml` | No register. The secrets list in the `wrangler.toml` comments is stale: it misses `WEBHOOK_SECRET`, `CROSSMINT_API_KEY`, `GITHUB_CLIENT_*`, `TELEGRAM_BOT_TOKEN` and `SLACK_WEBHOOK_URL` (all read in `worker/src/state.rs`). | S |
| 5.12/5.13 Classification | Informal. Migration 0044 separates PII from deposit amounts. | No scheme. Bank account/name and slip images are stored in plaintext columns and objects. | S |
| 5.19–5.22 Suppliers | Integrations: Cloudflare, Google OAuth/Sheets, Crossmint, Helius, Telegram, GitHub OAuth, Slack webhook, jsdelivr/unpkg CDNs | No supplier register and no DPA review. | S |
| 5.24–5.28 Incidents | `.issues/` works as postmortems; `docs/mainnet_canary_mitigation_runbook.md` | No severity scheme, incident log, or PDPA breach-notification procedure (72 h to the PDPC). | M |
| 5.29/5.30 Continuity | `docs/gradual_deploy_runbook.md`; D1 Time Travel | No RTO/RPO. Backups are manual; R2 and KV have none. No restore drill. | M |
| 5.31/5.34 Legal, PDPA | `worker/src/handlers/privacy.rs` (delete request, unsubscribe); privacy pages | No data-subject **access/export** endpoint and no Record of Processing. R2 deletes in `privacy.rs` discard their errors. | M |
| 5.35 Independent review | `docs/security_audit.md`, `docs/escrow-audit-2026-08-13.md`, `docs/SECURITY-FINDINGS-2026-08-13.md` | All reviews are internal or AI-assisted. No third-party audit. | L |

### 6 People

| Control | Evidence | Gap | Effort |
|---|---|---|---|
| 6.1–6.5 Screening, onboarding, offboarding | Staff roles come from the Sheet/event config (`auth.rs` `get_staff_role`) | No offboarding or access-review procedure. A 24 h JWT outlives removal from staff unless it is blacklisted. | S |

### 7 Physical

Mostly not applicable: everything is hosted on Cloudflare, so we rely on the
provider's attestations. The exception is **7.9/7.10**. The operator workstation
holds **11 plaintext prod D1 dumps with attendee PII** in `backups/` and
`worker/backups/`, plus `exports/`. All are mode `644`. They are git-ignored and
were never committed. FileVault is on, which covers loss of the device but not
other local users or processes. Gap: no rule on where dumps live, how long
they are kept, or how they are disposed of.

### 8 Technological

| Control | Evidence | Gap | Effort |
|---|---|---|---|
| 8.2/8.3 Privileged access | `require_super_admin`, `require_org_access`, `check_event_access` in `worker/src/auth.rs` | Super-admin comes from an env var, with no MFA or step-up. | M |
| 8.5 Authentication | HS256 JWT with constant-time verify; 24 h expiry; blacklist (migration 0016); `HttpOnly; Secure; SameSite=Lax` cookie; SIWS challenge held server-side and single-use | **Open:** the OAuth `state` is not bound to the browser, which allows login CSRF (`.issues/143`). | S–M |
| 8.7 Malware | Slip bytes are hashed to catch duplicates | No magic-byte or content-type validation of uploaded images. | S |
| 8.8 Vulnerability management | Lockfiles are committed and CI builds with them | **No advisory scan in CI.** A manual OSV scan on 2026-09-23 found 0 exploitable advisories. It did find 4 "unsound"/"unmaintained" notices, 2 of them fixed by patch bumps (see `.plans/029`). CI actions are pinned to tags, not SHAs. No Dependabot. | S |
| 8.9 Configuration | Staging and prod env blocks; `worker/tests/security/test_wrangler_environments.py` | The operator's personal email is in `wrangler.toml` vars. No `[observability]` retention setting. | S |
| 8.10 Deletion | `worker/src/cleanup.rs` daily cron (KV TTLs; deposit purge archives the amounts first) | **No retention limit for D1 attendees/contacts or R2 slip images** (`.issues/126`). | M |
| 8.11 Masking | Keyed fingerprints; PII log guards | Capability tokens in URL paths reach Cloudflare request logs (`.issues/071`). The fingerprint key reuses `JWT_SECRET`. | M |
| 8.12 DLP | Exports are role-gated | Exports are not audit-logged. | S |
| 8.13 Backup | Manual `wrangler d1 export` before migrations | Not scheduled, not encrypted, no offsite copy, no restore test. | M |
| 8.15 Logging | D1 `audit_log`, correlation IDs | Audit rows are pruned 90 days after event end. The log is not append-only. | M |
| 8.16 Monitoring | Slack alert on 5xx (`middleware/alert.rs`); `/api/health` | No alerting on security events (401 or 429 spikes). | S |
| 8.20–8.23 Network | Security headers, rate limits, no permissive CORS | CSP allows `script-src 'unsafe-inline'`. It also allows the two CDN hosts, which are now SRI-pinned. | M |
| 8.24 Cryptography | WebCrypto HMAC/RSA; secrets via `wrangler secret` | No key-rotation runbook. Bank fields are not encrypted at field level. | M |
| 8.25–8.29 Secure SDLC | See the summary | No SAST or secret scanning. No CODEOWNERS or required review. Kani proofs are not in CI. | S |
| 8.31 Environments | Separate staging resources | Staging and prod share `PLATFORM_SHEET_ID` and the super-admin identities. | S |
| 8.32 Change management | Gitflow; `deploy.sh` gates (size, content type, preflight with audited bypass, deploy tags) | Deploys run from a laptop, not CI. The preflight gate has not been satisfiable (`.issues/084`, `141`). | M |
| 8.33 Test data | Synthetic fixtures | `worker/scripts/seed_dev.sh` uses a real operator email. | S |

## Top gaps, ranked

1. **Login CSRF via unbound OAuth `state`** (`.issues/143`). S–M.
2. **Plaintext prod PII dumps on the workstation.** Needs a storage/retention/disposal rule. Immediate step for the owner: `chmod 600`, then delete dumps older than the rollback window. S. *Owner's call; nothing was deleted.*
3. **No dependency/secret scanning in CI.** Add cargo-deny (or an OSV scan) and gitleaks. S.
4. **No retention limit for D1 attendee/contact PII or R2 slips.** M.
5. **Bank data and slip images stored unencrypted, with no classification scheme.** M.
6. **Backups are manual and never restore-tested; R2 and KV are not covered.** M.
7. **Single-operator control over deploys.** M.
8. **No incident/breach procedure that meets PDPA notification duties, and no DSAR export.** M.
9. **Capability tokens in URL paths are logged** (`.issues/071`). M.
10. **No ISMS documents: policy, risk, asset and supplier registers.** S each; an external audit is L.
