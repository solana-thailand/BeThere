# Personal-data breach procedure (PDPA)

> **What to do when personal data leaks, is lost, or is changed without
> permission.** Written for a one-person operator running the `bethere` Worker.
>
> Written 2026-09-28. Owner decision: no ISO 27001 certification; keep the PDPA
> minimum (`.plans/029_iso27001_remediation.md` §4). What data exists and where:
> [pdpa_ropa.md](pdpa_ropa.md). Control gaps: `docs/iso27001_gap_assessment.md`
> (5.24–5.28 has no incident process; this file fills that gap).

---

## 0. The short version

| Step | Deadline | Output |
|---|---|---|
| 1 Detect | — | Time of awareness written down. **The 72-hour clock starts here.** |
| 2 Contain | Same day | Leak stopped, secrets rotated, evidence kept |
| 3 Assess | Within 24 h | Risk level, people affected, data categories |
| 4 Notify | PDPC **within 72 h** of awareness unless the breach is unlikely to risk anyone's rights; people **without delay** if the risk is high (PDPA s.37(4)) | Notices sent, or a written reason for not sending |
| 5 Record | Every breach, even a small one | A log entry (§5) |
| 6 Review | Within 2 weeks | Root cause, guard test, RoPA update |

| Role | Who |
|---|---|
| Incident lead, decision maker | **[Owner: legal name and contact]** |
| Backup contact if the owner is unreachable | **[Owner: name and contact]** |
| PDPC reporting channel | **[Owner: fill in the PDPC's current notification channel]** (unverified here) |

`SECURITY.md` is the public reporting route, but it names no contact yet
(`docs/iso27001_gap_assessment.md` 5.1). Fix that so reports can reach you.

---

## 1. Detect

A breach is any loss, access, disclosure or change of personal data that was not
allowed, whether by an attacker, a bug, or a mistake. Signals in this stack:

| Signal | Where |
|---|---|
| 5xx and 401/429 spike alerts | Slack (`worker/src/middleware/alert.rs`, `worker/src/spike.rs`) |
| Nightly cleanup, credit-ledger or NFT-journal reconcile failures | Slack (`worker/src/lib.rs` scheduled handler) |
| Secret found in git, vulnerable dependency | CI `security.yml` (gitleaks, cargo-deny/audit; plan 029 §2) |
| PII in Worker logs | `scripts/verify/pii_log_probe.sh` (leak → exit 1); `worker/tests/log_pii_guard.rs` |
| Outside report | `SECURITY.md` route, attendee message, organizer |
| Provider notice | Cloudflare, Google, GitHub, Crossmint, Helius, Slack, Telegram |
| Lost or stolen laptop | It holds prod D1 dumps (plan 029 §4: 7 days, mode 600) and deploy credentials |

**First action, always:** open a log entry (§5) and write the date and time you
became aware. Do not wait until you are sure it is a breach.

---

## 2. Contain

Keep evidence **before** you change things, then stop the leak.

### 2.1 Keep evidence

```sh
cd worker
npx wrangler deployments list            # which code is live
npx wrangler d1 export bethere-db --remote --output backup-$(date +%Y%m%d).sql
chmod 600 backup-*.sql                   # never commit it; the repo is public
```

- Screenshot or save the alert, report or log lines. Save them outside the repo.
- Read-only D1 queries (`SELECT`) are fine for scoping. Do not `d1 execute`
  writes on `events` rows; the read path is KV-first (`CLAUDE.md`, Data rules).
- Cloudflare Workers Logs keep 3 days on the free plan, if they are on for
  `bethere` (RoPA §13). Save what you need now; it expires.

### 2.2 Stop the leak: by scenario

Secrets are set with `npx wrangler secret put <NAME>` from `worker/`. Pipe the
value so the command never waits on a prompt. Add `--env staging` for staging.

```sh
openssl rand -hex 32 | npx wrangler secret put JWT_SECRET
```

| Scenario | Do this | Side effects |
|---|---|---|
| `JWT_SECRET` leaked, forged sessions, or a staff account compromised | Rotate `JWT_SECRET` (above). Remove the person from `STAFF_EMAILS`, the event's staff list and `organizations.owner_emails` as needed. | Every session ends; everyone signs in again. Every log fingerprint changes (`.issues/070`). Pending GitHub-link and email-link states become invalid. The JWT lasts 24 h, so rotation is the only way to end all of them at once. |
| Google service-account key leaked, or the Sheets hold unexpected data | Google Cloud console → IAM → Service accounts → Keys: disable the old key, create a new one. `npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_PRIVATE_KEY` (and `_EMAIL` if the account changed). Delete the cached token: `npx wrangler kv key delete google_access_token --binding EVENTS --remote` (it otherwise lives up to 3500 s, `worker/src/sheets/mod.rs:31-34`). Check each Sheet's sharing list and remove unknown people. | Sheets mirror writes fail until the new key is in. D1 stays the source of truth. |
| Google OAuth client secret leaked | Reset it in Google Cloud console, then `secret put GOOGLE_CLIENT_SECRET`. | Sign-in fails until the new secret is in. |
| Webhook secret leaked | New `WEBHOOK_SECRET`, and the same value in the Helius webhook auth header. | Deposit/escrow webhooks fail until both match. |
| Other provider keys | `HELIUS_API_KEY`, `CROSSMINT_API_KEY` (provider dashboards); `GITHUB_CLIENT_SECRET` (GitHub OAuth app); `TELEGRAM_BOT_TOKEN` (BotFather `/revoke`); `SLACK_WEBHOOK_URL` (regenerate the Slack webhook). Then `secret put` each. | That feature stops until the new key is in. |
| A deploy leaks data or breaks access control | `npx wrangler deployments list`, then `npx wrangler rollback <version-id>` (example: `docs/deploy_20260923_runbook.md`). | Rollback changes code only. D1 migrations and data are **not** rolled back. PUT-fallback deploys have caveats (`docs/mainnet_canary_mitigation_runbook.md`). |
| One route leaks data | There is **no per-route kill switch** in code. Ship a patch that returns 503 on that route, or roll back. Last resort: turn off the Worker's route in the Cloudflare dashboard (takes the whole site down). | — |
| PII in logs or Slack | Fix the code path, then run `scripts/verify/pii_log_probe.sh`. Delete the Slack messages. Worker logs cannot be deleted by us; they expire after 3 days on the free plan, if on. Known gap: Slack 5xx/spike alerts carry the raw path (RoPA §13). | — |
| Laptop lost, or a D1 dump exposed | Treat every dump on it as leaked. Rotate every secret in the table above (the deploy credentials were on it). Revoke the Cloudflare API token / `wrangler logout` sessions in the Cloudflare dashboard. Find out which dumps existed (plan 029 §4 keeps 7 days). | — |
| R2 slip or refund images exposed | Slips are served only through authed `/api/storage/slips/...`. Both buckets had public access off on 2026-09-28 (r2.dev URL disabled, no custom domain: `npx wrangler r2 bucket dev-url get` / `domain list`); re-check that first. | Slips contain bank names, account numbers and amounts: **high risk**. |

---

## 3. Assess the risk

Use [pdpa_ropa.md](pdpa_ropa.md) to list what was exposed. Answer in the log:

1. Which data categories? (RoPA sections and table/column names)
2. How many people? Use `SELECT COUNT(*)` queries; write the number, never the rows.
3. Was the data readable? (plaintext, fingerprint, encrypted)
4. Who got it? (public internet, one known person, a processor)
5. Is it still happening?

| Risk | Typical data here | Notify PDPC? | Notify people? |
|---|---|---|---|
| **High** | Bank account/name, slip images (`thb_deposits`, R2 `slips/`, event Sheet); session or claim tokens that allow takeover; email + name + contact handle at scale; photo consent with photos | Yes, within 72 h | Yes, without delay |
| **Medium** | Emails and names of a few people; developer profile fields | Yes, unless you can show it is unlikely to cause risk | Case by case; write why |
| **Low** | Log fingerprints, event ids, counts; wallet addresses and signatures that are already public on-chain | Usually not; **write down why** | No |

When in doubt, notify. A late or missing notice needs a written reason.

If an organizer is the controller for that event's data (RoPA §0, owner to
confirm), tell the organizer without delay so they can notify. If the breach is
at a processor (Cloudflare, Google, etc.), their notice to you starts your clock.

---

## 4. Notify

### 4.1 PDPC: within 72 hours of awareness

Send through the PDPC's official channel. Include at least:

- what happened and when, and when you found out;
- the kinds of personal data and the rough number of people;
- the likely consequences;
- what you did and will do to contain it and reduce harm;
- a contact person: **[Owner: legal name and contact]**.

If some facts are not known yet, send what you have within 72 h and follow up.
If you send it late, say why.

### 4.2 Affected people: without delay when the risk is high

The app cannot send email today (notifications are off, RoPA §1), and the in-app
inbox only carries fixed message kinds. Send from the owner's mailbox, one message
per person (never a visible recipient list). Use Thai and English.

Template:

> **Subject:** BeThere: an incident involving your personal data
>
> On [date] we found that [what happened]. It involved your [data categories].
> We have [containment steps]. We suggest you [concrete steps, for example: watch
> your bank account for unexpected transfers; sign in again].
> Contact: [Owner: contact]. We have [notified / will notify] the Personal Data
> Protection Committee.

---

## 5. Record every breach

Keep the full record **outside this repo**; the repo is public
(**[Owner: private location]**). A PII-free summary may go in `.issues/` using the
Status vocabulary in `CLAUDE.md`.

```markdown
## Breach YYYY-MM-DD-NN

- Aware at (date, time, TZ):
- Detected by:
- What happened:
- Data categories (RoPA section, tables/columns):
- People affected (count only):
- Risk level and reasoning:
- Containment steps (time, action):
- Secrets rotated:
- PDPC notified? (yes: time, channel, reference / no: reason)
- People notified? (yes: time, how many, channel / no: reason)
- Organizers or processors told:
- Root cause:
- Fix (commit, deploy tag):
- Guard added (test or script):
- Closed on:
```

---

## 6. Review after the incident

Within two weeks:

1. Find the root cause. Re-run the repro against the fixed code.
2. Add a guard that fails if it comes back (a `worker/tests/*_guard.rs` test, or
   a `scripts/verify/` gate with a `--self-test`). A gate that cannot fail is not
   a gate.
3. If the fix guards one path, find every other writer of the same data and
   guard it too (`CLAUDE.md`, Data rules).
4. Update [pdpa_ropa.md](pdpa_ropa.md) if a data flow, processor or retention
   rule changed, and the `/privacy` page (`frontend-leptos/src/pages/privacy.rs`)
   if what it tells users is no longer true.
5. Check the rotated secrets are in both prod and staging, and that
   `LOG_FINGERPRINT_KEY` is still on the plan (plan 029 §3).
6. Close the log entry.

Practise once a year: walk through §2 on staging (`--env staging`) with a fake
scenario, and time how long the secret rotation takes.
