# 158: The /privacy page promises things the code does not do

**Status:** deployed 2026-09-28 — `/privacy` rewritten to match the code in `34a430d7`, on prod as version `3ce6d82c` (`main` `dbdf03b3`) and read back from the live page (session `event-checkin-af`). The owner approved "match the code now". The promised 90-day Sheet clearing is still not built; it waits on the retention decision (plan 029 §3). The consent bundling found while fixing this is `.issues/161`.

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
