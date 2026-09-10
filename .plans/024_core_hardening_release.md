# 024 — Core hardening release

Status: implementation complete locally; push and deployment pending.

This release contains the stacked CSS cleanup, notification/core hardening,
wallet-email provenance, and event-list pagination branches. It uses existing
Cloudflare resources only and does not authorize a paid-plan upgrade.

## Branch order

1. `feature/css-prune`
2. `feature/core-hardening-notifications`
3. `feature/event-pagination`

Push all three branch refs, then review them as stacked diffs in that order.
Do not open `feature/event-pagination` directly against `develop` as a small PR:
its ancestry intentionally contains the earlier stack. After each parent lands,
retarget the next PR to `develop`.

## Completed local gates

- Worker full tests and strict Clippy pass.
- Frontend host tests, wasm32 strict Clippy, and CSS contract tests pass.
- Production migrations execute together under SQLite behavior tests.
- Wallet provenance tests cover legacy, recipient, verified, and conflicting links.
- Event pagination tests cover authorization-before-limit, stable keysets, and index use.

## Remote gates before production

1. Push the three branches and require green remote CI.
2. Deploy the current head to staging and run the flow preflight/smoke tests.
3. Confirm the staging and production D1 IDs differ and list pending migrations.
4. Export production D1 to the existing private backup directory outside this repo.
5. Record a D1 Time Travel bookmark. The free plan retains Time Travel for seven days.
6. Apply migrations `0030_notifications.sql`, `0031_wallet_email_provenance.sql`,
   and `0032_events_pagination.sql` to production. Wrangler applies each migration
   transactionally and captures its own backup, but the explicit export remains the
   project rollback artifact.
7. Deploy Worker and frontend from the reviewed commit. Migration must precede code
   because the new wallet lookup requires the provenance columns.
8. Verify `/api/health`, Google login, wallet login, the first and second event pages,
   attendee registration, notification inbox, dashboard, and scanner selection.
9. Watch 4xx/5xx and D1 error logs. Roll the Worker back if application checks fail;
   use D1 Time Travel only if schema/data rollback is actually required.

## Go/no-go

Push is ready when the local pagination commit is recorded. Production deploy is
ready only after remote CI and staging are green and the private D1 export exists.
