# 158: The /privacy page promises things the code does not do

**Status:** open (2026-09-28). Found while writing `docs/pdpa_ropa.md` (§16); each point re-checked in source by session `event-checkin-af`. Fixing the page text is a public legal statement, so the wording is the owner's call.

## What the page says vs what the code does

| `/privacy` says (`frontend-leptos/src/pages/privacy.rs`) | Code |
|---|---|
| "Personal data in Google Sheets is retained until event conclusion plus 90 days … After this period, PII fields … are cleared." | Nothing clears Sheet PII on a schedule. `sheets::write::clear_sheet_cells_batch` has one caller, the deletion-request handler (`worker/src/handlers/privacy.rs:466`). The nightly cron (`worker/src/cleanup.rs`) never touches Sheets. |
| "You may decline photo consent without affecting your registration." | When an event sets `require_photo_consent`, registration is refused without it (`worker/src/handlers/register/signup.rs:174`). |
| Recipients: organizers, Google, Helius. | Also Cloudflare (hosting, D1/KV/R2, Web Analytics), Crossmint (wallet address for badge mint), GitHub and Telegram (when linked), Slack (ops alerts). Full list: `docs/pdpa_ropa.md` §1. |
| "Cloudflare logs auto-expire after 72 hours." | Unverified: `wrangler.toml` has no `[observability]` block. |

## Fix options

- Make the page match the code now: state the retention rules the code actually has (RoPA "retention"), say photo consent can be required by an event, and list every processor.
- Or implement the promised 90-day Sheet clearing (ties into plan 029 §3 retention, an owner decision).
