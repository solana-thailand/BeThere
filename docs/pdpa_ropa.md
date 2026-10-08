# PDPA Record of Processing Activities (RoPA)

> **Record kept under Thailand PDPA section 39.** It lists what personal data
> BeThere processes, why, where it lives, who receives it and how long it is kept.
>
> Written 2026-09-28 against `develop` at `02dd7466`. Every system fact was read
> from code or from an existing doc, and the file is cited. "Unverified" means it
> could not be checked from the repo. "Owner to confirm" marks a legal judgment
> that the owner has to make. Plan: `.plans/029_iso27001_remediation.md` §4 (owner
> decision 2026-09-28: no certification; write the PDPA minimum).
> Breach procedure: [pdpa_breach_procedure.md](pdpa_breach_procedure.md).

---

## 0. Controller and scope

| Item | Value |
|---|---|
| Data controller | **[Owner: legal name and contact]** |
| DPO | None appointed. Owner to confirm whether s.41 requires one. |
| System | Cloudflare Worker `bethere` (`worker/`), Leptos SPA (`frontend-leptos/`), Solana escrow (`bethere-escrow/`) |
| Prod stores | D1 `bethere-db`, KV `EVENTS`, R2 `bethere-assets` (`worker/wrangler.toml`) |
| Staging | Separate D1/KV/R2 (`[env.staging]`, `worker/wrangler.toml`). Test data only. |

**Controller or processor? Owner to confirm.** The `/privacy` page says "BeThere is
operated by Solana Thailand" (`frontend-leptos/src/pages/privacy.rs:24`).
`.issues/043` records a different design choice: the organizer is the controller
and BeThere is a processor. The owner has to decide which is true for each
organization. If organizers are controllers, a s.40 processing agreement is needed.

**Lawful bases used below** (PDPA s.24 / s.19): *Contract* = needed to deliver
the event service the person signed up for. *Consent* = an optional choice.
*Legitimate interest* = security or operations, balanced against the person.
*Legal obligation* = a Thai law requires it.

---

## 1. Processors and recipients

"On" means the Worker calls it in prod today. Prod secret **names** were read
with `npx wrangler secret list` on 2026-09-28 (values are never shown).

| Recipient | What it gets | Called from | Prod state |
|---|---|---|---|
| Cloudflare (Workers, D1, KV, R2, rate limits) | Everything below; hosting | `worker/wrangler.toml` | On |
| Cloudflare Web Analytics | Page views from the browser beacon | `frontend-leptos/index.html:137` | On |
| Google OAuth | Sign-in: `openid email profile` scope | `worker/src/auth.rs:130`, `worker/src/http.rs:167,173` | On |
| Google Sheets (service account) | Attendee rows, bank info, consent, contacts, sign-in log, waitlist | `worker/src/sheets/bg_sync.rs`, `worker/src/handlers/user_log.rs`, `worker/src/handlers/waitlist.rs` | On (`GOOGLE_SERVICE_ACCOUNT_*` set) |
| Google Fonts | Visitor IP (font CSS/files loaded by the browser) | `frontend-leptos/index.html:141-144` | On |
| Helius (RPC, DAS, webhooks) | Wallet addresses, tx signatures | `worker/src/solana.rs:405`, `worker/src/escrow_indexer/poller.rs`, `worker/src/solana_escrow/wire.rs` | On (`HELIUS_API_KEY` set) |
| Crossmint | Recipient wallet + event-level NFT metadata (no name/email) | `worker/src/solana.rs:94-160` | On (`CROSSMINT_API_KEY` set; `CROSSMINT_HOST = www.crossmint.com`, `worker/wrangler.toml:161`) |
| GitHub OAuth | OAuth code exchange; reads `/user` | `worker/src/handlers/social_link.rs:171,368` | On (`GITHUB_CLIENT_*` set) |
| Telegram | Login widget runs in the browser; Worker only checks the HMAC, no outbound call | `worker/src/handlers/social_link.rs:721` | On (`TELEGRAM_BOT_TOKEN` set) |
| Slack (incoming webhook) | Ops alerts; see §13 for what they contain | `worker/src/middleware/alert.rs:108`, `worker/src/lib.rs:227,260,290` | On (`SLACK_WEBHOOK_URL` set) |
| unpkg / jsDelivr CDNs | Visitor IP (web3.js fallback; SRI-pinned) | `.plans/029` §2 | On |
| Solana network (public) | Wallets, signatures, check-in markers, on-chain forever | escrow + cNFT | On (escrow is **devnet**, `worker/wrangler.toml:169`) |
| Anthropic (slip vision) | Would receive slip images | `worker/src/slip_vision.rs:27` | **Off.** Needs `ANTHROPIC_API_KEY` **and** `SLIP_AGENT_VISION=on` (`worker/src/state.rs:443`). Neither is in prod. |
| Cloudflare Email Sending | Would receive recipient email | `worker/src/notifications/transport.rs:36` | **Off.** `NOTIFICATIONS_ENABLED = "0"` (`worker/wrangler.toml:81`), `[[send_email]]` commented out (`:449`), and `notifications::dispatch` has no caller in `worker/src`. |
| Organizers and staff | Attendee lists, exports, Sheets | §12 | On |

**Cross-border transfer (s.28/29).** All the processors above are foreign
companies. Prod D1 runs in **APAC** with no jurisdiction set (`npx wrangler d1 info
bethere-db`, 2026-09-28); where the other processors store data is unverified.
The `/privacy` page said transfers are disclosed (`.issues/043` checklist) but named only
Google and Helius (`privacy.rs:86-88`). Basis for transfer: owner to confirm.

---

## 2. Sign-in and sessions

| | |
|---|---|
| Purpose | Identify the user; run role checks (attendee, staff, organizer, super-admin) |
| Lawful basis | Contract |
| Subjects | Anyone who signs in |
| Data | Google `id`, `email`, `name` (`domain/src/models/auth.rs:51`); or a Solana wallet address (SIWS, `/api/auth/wallet/*`). JWT holds `email`, `sub`, `email_verified`. |
| Storage | JWT in cookie `event_checkin_token`, `HttpOnly; Secure; SameSite=Lax`, `Max-Age=86400` (`worker/src/handlers/auth.rs:196`). D1 `jwt_blacklist.token_hash` on logout. No Google refresh token is stored (no `refresh_token` in `worker/src`). |
| Also | **Sign-in log:** every Google sign-in upserts email, Google id, role, first/last seen into the `users` tab of the platform Sheet (`worker/src/handlers/user_log.rs:1-16`, called from `handlers/auth.rs:135`). |
| Recipients | Google (OAuth, Sheets) |
| Retention | Cookie 24 h. `jwt_blacklist` rows deleted after expiry (`worker/src/db/jwt_blacklist.rs:50`, cron `cleanup.rs:293`). **Sign-in log Sheet: no retention limit in code today. Owner decision pending (plan 029 §3).** |
| Lawful basis of the sign-in log | Legitimate interest (analytics). Owner to confirm. |
| Security | HS256 JWT, constant-time verify, blacklist (`docs/iso27001_gap_assessment.md` 8.5). Login CSRF still open (`.issues/143` Part B). |

## 3. Event registration and check-in

| | |
|---|---|
| Purpose | Register, manage capacity, issue a ticket/QR, check in at the door, walk-ins, post-event registration |
| Lawful basis | Contract. The form also requires a PDPA consent tick (`worker/src/handlers/register/signup.rs:166`). A tick that is a condition of service is weak consent; owner to confirm whether contract is the real basis. |
| Subjects | Attendees, walk-ins |
| Data (D1 `attendees`) | `email`, `name`, `ticket_name`, `participation_type`, `approval_status`, `registration_phase`, `contact_channel`, `contact_handle`, `checked_in_at`, `checked_in_by`, `claim_token`, `qr_url`, `consent_marketing`, `consent_marketing_at` (migrations 0002, 0005, 0020, 0050). Walk-in phone goes in `contact_channel` (`worker/src/db/attendees/walkin.rs:57`). |
| Data (other) | `registration_responses` (`developer_email`, `field_key`, `field_value`), including `consent_given` and `photo_consent_given` (`worker/src/handlers/register/contact.rs:164-170`). `attendance_answers` (`answer`, `updated_by`) for the RTM#6 re-ask (0052). KV `qr:{attendee_id}`. |
| Photo and marketing consent | Consent, each in its own unticked checkbox on the registration form (`.issues/161`, `registration_form.rs`). Photo is optional unless the event sets `require_photo_consent`; then registration is refused without it (`signup.rs:174`) and the box says so. Marketing is always optional. Before `.issues/161` one required box set both, so `consent_marketing` recorded before that fix is not valid consent. |
| Storage | D1; the event's Google Sheet (row incl. consent columns, `worker/src/sheets/bg_sync.rs:268-304`) |
| Recipients | Google Sheets; organizers/staff of that event |
| Retention | **No retention limit in code today. Owner decision pending (plan 029 §3).** The cron does not touch `attendees`, `registration_responses` or any Sheet. The `/privacy` page promises "event conclusion plus 90 days ... PII fields are cleared" (`privacy.rs:97`); no code does this except a data-subject request (§15). |
| Security | Attendee id is global, so every by-id lookup filters `event_id` (`.issues/153`, guard `attendee_event_scope_guard`). Log fingerprints (§13). |

## 4. Developer profile and organizer audience

| | |
|---|---|
| Purpose | A profile built across events; organizer audience views (`/api/contacts*`, `/api/community/*`); marketing outreach |
| Lawful basis | Profile: legitimate interest or consent, owner to confirm. Outreach: consent (unticked by default, `.issues/043` Phase E, `.issues/115`). |
| Subjects | Registrants |
| Data | `developer_profiles`: `email`, `display_name`, `wallet_address`, `github_handle`, `discord_handle`, `twitter_handle`, `telegram_handle`, `telegram_id`, `experience_level`, `primary_role`, `tech_stack`, `interests`, `learning_goals`, `expectations`, `company_org`, `location_city`, `consent_outreach`, `badges_earned`, `total_events` (0002, 0025, 0031). `contacts`: `email`, `name`, `events_joined`, `event_count`, `contact_channel`, `contact_handle` (0002). |
| Storage | D1; the org's Contacts Sheet (`organizations.contacts_sheet_id`, or the global `CONTACTS_SHEET_ID` secret) |
| Recipients | Google Sheets; org owners and organizers |
| Retention | **No retention limit in code today. Owner decision pending (plan 029 §3).** |
| Withdrawal | `POST /api/privacy/unsubscribe-marketing` clears `attendees.consent_marketing` and `developer_profiles.consent_outreach` (`worker/src/handlers/privacy.rs:318`, `worker/src/db/developers.rs`). |

## 5. THB deposit slips and refunds

| | |
|---|---|
| Purpose | Take a PromptPay deposit, verify the slip, refund it or hold it as credit (`docs/deposit-commitment-model.md` §3.1) |
| Lawful basis | Contract |
| Subjects | In-person attendees who pay |
| Data (D1 `thb_deposits`) | `attendee_id`, `attendee_name`, `amount_thb`, `slip_url`, `slip_blake3`, `bank_account`, `bank_name`, `account_name`, `refund_proof_url`, `verified_by`, `deposit_source`, timestamps (0013, 0022, 0046, 0047). Legacy `attendees.bank_name`, `bank_account_number`, `bank_account_name`, `deposit_slip_r2_key` (0002; written by the Sheet→D1 sync, `worker/src/db/attendees/management.rs:203`). |
| Slip checker | `slip_proposals`: `bank_ref`, `claimed_ref`, `amount_satang`, `transferred_at`, `receiver_account`, `verdict` (0054). The mini-QR is decoded in the browser and re-parsed on the server (`handlers/deposit/thb/handlers/slip_agent.rs:21`). Vision is off (§1). |
| Storage | Slip and refund images in R2 `slips/{event_id}/{attendee_id}.{ext}` and `refunds/...` (`worker/src/storage.rs:9-10`). If the R2 put fails, the data URL is kept in `slip_url` in D1 (`handlers/deposit/thb/handlers/mod.rs:246`). A `slip_url` can also be an external link (one RTM#3 row is a Google Drive link, `.issues/126`). **Bank account and name are also written to the event Google Sheet** (`slip_upload.rs:378`, `bg_sync::write_bank_info`). Legacy KV `event:{id}:deposit:thb:*`. |
| Recipients | Google Sheets; organizers (refund queue, slip view) |
| Retention | Nightly cron, 03:00 UTC (`worker/wrangler.toml:218`). After `event_end + refund_deadline_hours + 90 days` (`worker/src/cleanup.rs:44,107`) it: copies amounts into the PII-free `thb_deposit_archive` (`cleanup.rs:148`); deletes `thb_deposits` **only if every live row has an archive row** (`cleanup.rs:162`, `.issues/127`); deletes `slip_proposals` with them (`worker/src/db/thb_deposits.rs:549-558`); deletes `deposit_statuses` and the KV deposit keys (`cleanup.rs:118`). `refund_deadline_hours` falls back to 7 days if the config is unreadable (`cleanup.rs:92`); the column default is 168 h (0003). |
| Not purged | **R2 slip and refund images** (`cleanup.rs:27`, `.issues/126` §3, R2 orphans open). Legacy `attendees.bank_*` columns. Bank data in the Sheet. `thb_deposit_archive` keeps `attendee_id`, which still links to the `attendees` row, so it is pseudonymous, not anonymous. **No retention limit in code today for these. Owner decision pending (plan 029 §3).** Whether accounting law requires keeping the amounts: owner to confirm. |
| Security | Magic-byte check on uploads (`domain::image_kind`, `.plans/029` §2). Slips served only through authed `/api/storage/slips/...`. Bank fields are plaintext; field-level encryption is blocked on an owner key decision (plan 029 §3). Refund-proof URL has no scheme check (`.issues/145`). |

## 6. Rolling credit ledger

| | |
|---|---|
| Purpose | Keep a THB deposit as credit for the next event and apply it automatically |
| Lawful basis | Contract (the attendee chooses credit) |
| Subjects | Attendees who hold credit |
| Data | `credit_ledger`: `email`, `organization_id`, `currency`, `delta`, `reason`, `event_id`, `deposit_id`, `note` (0028). `contacts.deposit_credit_thb`, `deposit_credit_usdc`, `deposit_credit_since`, `credit_refund_requested(_at)` (0002, 0023). Contacts Sheet credit columns (`worker/src/sheets/contacts.rs`). **Payout account** `credit_refund_accounts` (0059, `.issues/190`): `email`, `method` (`promptpay`/`bank`), `promptpay_id` or `bank_name` + `bank_account` + `account_name`, `source` (`deposit`/`attendee`), `source_deposit_ref` (`{event_id}:{attendee_id}`), `captured_at`, `replaced_deposit_account`. A `deposit` row is a copy of the refund account the attendee gave with their THB deposit (`thb_deposits.bank_*`, §5), taken when the deposit is held as credit (`db::thb_deposits::try_settle_hold_credit`) and backfilled by 0059 for people holding credit then; an `attendee` row is one they typed on the credit refund card. |
| Storage | D1; Contacts Sheet |
| Recipients | Google Sheets; organizers (credit liability views). The payout account: organizers whose payout scope covers every organization of the person's credit (`credit_payout::payout_scope`), in full; the attendee, masked (bank, last four digits, first name and initial). |
| Retention | Ledger: **no retention limit in code today. Owner decision pending (plan 029 §3).** The ledger deliberately survives the deposit purge (`cleanup.rs:140`). Credit is never forfeited (owner decision, `docs/deposit-commitment-model.md`). **Payout account:** kept while the person (all linked emails) has an open credit refund request or holds credit — a positive ledger bucket, or credit applied to an event and not yet returned. Deleted when the payout is recorded (`contacts::clear_credit_refund_requested`), by the nightly cron once there is no open request and that balance is 0 (`credit_refund_accounts::PURGE_SQL`, `cleanup.rs` phase 0), and on erasure (§15). It outlives the `thb_deposits` row it was copied from (deleted 90 days after the event) on purpose: the attendee is not asked again for an account they already gave. This is within the `/privacy` notice, which keeps refund bank details until the person asks for deletion (`locales/*/privacy.json` `retention_body`). |
| Security | Nightly ledger reconcile with a Slack alert (`worker/src/lib.rs:243-275`); counts only, no identities. Payout account fields are never logged (`worker/tests/credit_payout_guards.rs::account_details_never_reach_the_log`); the attendee API returns the masked preview only (`the_attendee_gets_a_masked_preview_only`). Plaintext, like `thb_deposits.bank_*`. |

## 7. Email linking

| | |
|---|---|
| Purpose | Let one person join several emails so credit follows them (plan 025, `.issues/122`) |
| Lawful basis | Contract (user-initiated). Admin links: legitimate interest, owner to confirm. |
| Subjects | Attendees with more than one email |
| Data | `person_emails`: `email`, `person_id`, `is_primary`, `proof` (`google` or `admin`), `linked_at` (0043) |
| Storage | D1 |
| Retention | **No retention limit in code today. Owner decision pending (plan 029 §3).** Unlink deletes the row (`worker/src/db/person.rs:195-204`). |
| Security | Both emails proved by Google in one browser; HMAC state plus the session cookie stops link-CSRF (`worker/src/handlers/email_link.rs:1-19`). |

## 8. USDC escrow deposits

| | |
|---|---|
| Purpose | Take a USDC deposit into the on-chain escrow and refund it |
| Lawful basis | Contract |
| Subjects | Attendees paying in USDC; organizers (escrow owners) |
| Data | `deposit_statuses`: `attendee_id`, `wallet_address`, `tx_signature`, `amount`, `method` (0014). `attendees.deposit_tx_hash`, `refund_tx_hash`. `onchain_events`: `attendee`, `organizer`, `signature`, `amount` (0011). On-chain `AttendeeDeposit` PDA derived from the wallet. |
| Storage | D1; KV `event:{id}:deposit:status:*`; Solana (public, permanent) |
| Recipients | Helius (RPC, webhooks `/api/deposit/usdc/webhook`, `/api/escrow/onchain-webhook`); the public Solana network |
| Retention | `deposit_statuses`, KV deposit keys and `onchain_events` deleted at `event_end + refund_deadline + 90 days` (`cleanup.rs:118,257`). `onchain_dedup` after 90 days (`cleanup.rs:286`). **On-chain data cannot be deleted** (disclosed, `privacy.rs:63-71`). |
| Note | The escrow runs on **devnet** today (`worker/wrangler.toml:169`), so no real USDC moves, but wallet addresses are still personal data. |
| Security | Webhook bearer compared in constant time (`docs/iso27001_gap_assessment.md`). Wallets, signatures and the PDA are fingerprinted in logs (`.issues/070` slice 2). |

## 9. NFT badge claim, quiz, adventure, campaigns

| | |
|---|---|
| Purpose | Mint an attendance badge to the attendee's wallet; quiz/adventure gates; multi-event campaign rewards |
| Lawful basis | Contract (the attendee asks for the badge) |
| Subjects | Checked-in attendees |
| Data | `claim_locks`: `token`, `wallet`, `asset_id`, `signature` (0001, 0055). `nft_mint_jobs`: `claim_token`, `wallet`, `provider_mint_id`, `asset_id`, `signature` (0033). `attendees.claim_token`, `claimed_at`, `claim_asset_id`, `claim_signature`. `quiz_progress` / `adventure_progress` keyed by `claim_token` (0004, 0010). `developer_campaign_progress.developer_email` (0007). KV `crossmint:pending:{token}`. |
| Storage | D1; KV; the minted cNFT on Solana (public, permanent) |
| Recipients | Crossmint (wallet + event metadata only, `solana.rs:141-155`); Helius DAS for `/api/wallet/{address}/nfts` (`solana.rs:405`) |
| Retention | KV quiz/adventure progress: `event_end + 30 days` (`cleanup.rs:40,97`). KV claim locks: financial cutoff (`cleanup.rs:44,107`). **D1 `claim_locks`, `nft_mint_jobs`, `quiz_progress`, `adventure_progress`, `developer_campaign_progress`: no retention limit in code today. Owner decision pending (plan 029 §3).** Claim tokens stop working 180 days after check-in in prod (`CLAIM_TOKEN_TTL_SECS`, `worker/wrangler.toml:146`); that limits use, not storage. |
| Security | Claim token in URL path reaches Cloudflare request logs (`.issues/071`, bounded by the TTL above). Fingerprinted in Worker logs. |

## 10. Social account linking (GitHub, Telegram)

| | |
|---|---|
| Purpose | Show verified GitHub / Telegram handles on the developer profile |
| Lawful basis | Consent (optional, user-initiated; can unlink via `/api/auth/social/unlink`) |
| Subjects | Users who link |
| Data | `developer_profiles.github_handle`, `github_verified(_at)`, `telegram_handle`, `telegram_id`, `telegram_verified(_at)` (0025) |
| Storage | D1 |
| Recipients | GitHub (OAuth); Telegram (browser widget) |
| Retention | Until unlink or erasure (`worker/src/db/developers.rs:573`). **No time limit in code today. Owner decision pending (plan 029 §3).** |
| Security | Signed OAuth state `{email}|{expires}|{hmac}` (`handlers/social_link.rs:91`); Telegram hash verified with the bot token (`:721`). Handles and ids fingerprinted in logs. |

## 11. Notifications, surveys and feedback

| | |
|---|---|
| Purpose | In-app inbox (registration, reminder, deposit result, post-event survey); `/feedback` session ratings |
| Lawful basis | Inbox: contract. Survey/feedback: legitimate interest, owner to confirm. |
| Subjects | Registrants; checked-in attendees for surveys |
| Data | `notification_enrollments` (`attendee_id`, `event_id`), `notification_outbox` (`attendee_id`, `kind`, `status`, `read_at`, `message_id`) (0030, 0035). The queue stores no recipient or body copies (`worker/src/notifications/mod.rs:80`). Feedback answers are `registration_responses` rows with `field_key` `post.*` (0040). |
| Storage | D1 |
| Recipients | None today: email sending is off (§1) |
| Retention | Outbox rows go when the attendee row is deleted or its email changes (0035 triggers). **Otherwise no retention limit in code today. Owner decision pending (plan 029 §3).** |

## 12. Staff, organizer and admin access

| | |
|---|---|
| Purpose | Run events; control who may see attendee data; keep an audit trail |
| Lawful basis | Legitimate interest (and contract with organizers). Owner to confirm. |
| Subjects | Staff, organizers, org owners, super-admins; attendees named in audit entries |
| Data | `staff` (`email`, `role`, `name`); `events.organizer_emails`, `staff_emails`, `updated_by`; `organizations.owner_emails`; `STAFF_EMAILS` secret; `SUPER_ADMIN_EMAILS` var (a real personal email sits in `worker/wrangler.toml`, gap 8.9). Actor columns: `attendees.checked_in_by`, `refund_marked_by`, `deposit_verified_by`, `thb_deposits.verified_by`, `event_summaries.frozen_by`, `attendance_answers.updated_by`. Audit: D1 `audit_log` (`actor`, `action`, `target`, `description`, `metadata`) (0001), KV fallback `event:{id}:audit`. Staff Sheet tab `staff`. |
| Storage | D1, KV, Google Sheets, Worker config |
| Recipients | Google Sheets; staff exports (`/api/walkin/export`, CSV) |
| Retention | KV audit entries pruned at `event_end + 90 days` (`cleanup.rs:52,232`), orphans removed (`cleanup.rs:279`). **D1 `audit_log` has no DELETE anywhere in `worker/src`: no retention limit in code today. Owner decision pending (plan 029 §3).** |
| Security | Role checks `require_super_admin`, `require_org_access`, `check_event_access` (`worker/src/auth.rs`). No MFA yet; Google 2SV decided first (plan 029 §4). Exports not audit-logged (gap 8.12). A 24 h JWT outlives removal from staff unless blacklisted (gap 6.1). |

## 13. Logs, alerts and observability

| | |
|---|---|
| Purpose | Debug, detect attacks, alert on failures |
| Lawful basis | Legitimate interest (security and reliability) |
| Subjects | Everyone who sends a request |
| Data | Worker `tracing` lines with a correlation id, a **redacted** path (`worker/src/middleware/correlation.rs:44,104`) and keyed fingerprints instead of email, wallet, signature, name, GitHub/Telegram id and claim token (`.issues/070`; `AppState::log_fingerprint`, `worker/src/state.rs:470`). Cloudflare's own request log keeps the raw URL (`.issues/071`). |
| Slack | 5xx and security-spike alerts carry method + path + correlation id (`worker/src/middleware/alert.rs:63-89`). **That path is the raw `req.uri().path()` (`alert.rs:44`), not the redacted one**, so a claim token or wallet in the path reaches Slack. `/api/admin/test-alert` sends the requesting super-admin's email (`worker/src/handlers/attendee/admin.rs:102`). Cron alerts carry counts and event ids only (`worker/src/cleanup.rs` `CleanupFailure`). |
| Storage | Cloudflare Workers Logs / `wrangler tail`; Slack |
| Retention | If Workers Logs is on, the free plan keeps logs **3 days** (Cloudflare Workers Logs docs, read 2026-09-28). Whether it is on for `bethere` is **unverified**: `wrangler.toml` has no `[observability]` block, new Workers default to on, and no read-only wrangler command shows the setting (check the dashboard). The `/privacy` page no longer states a log retention (`.issues/158`). D1 Time Travel keeps **7 days** of history on the free plan (D1 docs), which is also how long a deleted row stays restorable. Slack retention: unverified (Slack workspace setting). |
| Security | Guards: `worker/tests/log_pii_guard.rs` (8 tests), `error_body_guard.rs`; runtime probe `scripts/verify/pii_log_probe.sh`. Fingerprint key is `JWT_SECRET` until `LOG_FINGERPRINT_KEY` is provisioned (branch `feature/029-log-fingerprint-key`, plan 029 §3). |

## 14. Waitlist and backups

| | Waitlist | Backups |
|---|---|---|
| Purpose | Tell people about launches | Restore after a bad migration or deploy |
| Lawful basis | Consent | Legitimate interest |
| Data | Email + timestamp (`worker/src/handlers/waitlist.rs:1-5`) | Full D1 exports (`npx wrangler d1 export ... --remote`) with all tables above |
| Storage | `waitlist` tab of the platform Sheet | Operator workstation (`backups/`, `worker/backups/`, `exports/`); FileVault on (`docs/iso27001_gap_assessment.md` §7) |
| Recipients | Google Sheets | None |
| Retention | **No retention limit in code today. Owner decision pending (plan 029 §3).** | 7 days, mode 600, older ones moved to Trash (owner decision 2026-09-28, plan 029 §4). R2 and KV are not backed up. D1 Time Travel exists; its window is unverified. Offsite encrypted backup blocked on owner (plan 029 §3). |

---

## 15. Data-subject rights

| Right | How today | Gap |
|---|---|---|
| Erasure | `POST /api/privacy/delete-request`, only after the event ends (`worker/src/handlers/privacy.rs:56`; `.issues/043` Phase D). Clears `attendees` name/email/contact/bank/claim token (`worker/src/db/attendees/management.rs:110`), `contacts` name/handle and the `credit_refund_accounts` row of that email (`contacts::clear_contact_pii`), `developer_profiles` (`worker/src/db/developers.rs:573`), deletes `registration_responses`, KV keys, R2 slips/refunds, and the Sheet row. | Not touched: `thb_deposits` (bank fields until the purge), `credit_ledger`, `person_emails`, `deposit_statuses`, `claim_locks`, `nft_mint_jobs`, `audit_log`, `contacts.email` (the key), sign-in log and waitlist Sheets. R2 delete errors are discarded (`privacy.rs:182,191`). |
| Withdraw marketing consent | `POST /api/privacy/unsubscribe-marketing` | — |
| Access / portability | None | No DSAR export (gap 5.31). Manual today. |
| Correction | Organizer edits the Sheet (`.issues/043`) | — |

## 16. Known gaps (found while writing this record; not fixed here)

1. `/privacy` says Sheet PII is cleared at event end + 90 days; no code does it (`privacy.rs:97`, §3).
2. Fixed on `feature/161-separate-consent`: photo and marketing consent have their own boxes. Marketing consent recorded by the old single box still needs clearing (`.issues/161`).
3. `/privacy` lists only Google and Helius as recipients; Crossmint, GitHub, Telegram, Slack and Cloudflare are missing (`privacy.rs:86-88`).
4. Slack 5xx and spike alerts send the raw request path (`middleware/alert.rs:44`).
5. No retention limit for most D1 tables, all Sheets, and R2 slips (plan 029 §3, `.issues/126`).
6. Bank account data sits in four places: D1 `thb_deposits`, legacy `attendees.bank_*`, the event Google Sheet, and — for people holding credit — the payout account copy in `credit_refund_accounts` (§6, which has its own retention rule).
