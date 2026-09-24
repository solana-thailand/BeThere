# migrations-pending

Migrations that are **written and verified but must not run yet**.

`wrangler d1 migrations apply` applies every `.sql` in `migrations_dir`
(`worker/migrations`, per `wrangler.toml`). A `-- DO NOT APPLY` comment at the
top of a file there does nothing at all — the runner never reads it. That was
the state of `0048` for about twenty minutes on 2026-09-22, which is a small
re-enactment of the very bug `.issues/127` is about: a rule written where
nothing enforces it.

So a migration that is deliberately deferred lives **here**, outside
`migrations_dir`, where the runner cannot see it. The directory is the
enforcement; the header comment is only the explanation.

## To apply one

Move it back and run the migration normally:

```
git mv worker/migrations-pending/00NN_name.sql worker/migrations/
cd worker
npx wrangler d1 migrations list DB --remote            # listing first is what
CI=true npx wrangler d1 migrations apply DB --remote   # stops it being "blind"
```

Then read the schema back — `SELECT sql FROM sqlite_master WHERE name='<table>'`
— rather than trusting the ✅.

## Currently deferred

| file | blocked until | why |
|---|---|---|
| `0048_thb_deposits_unique.sql` | **2026-09-28** | Rebuilds the live `thb_deposits` table to add `UNIQUE (event_id, attendee_id)` (`.issues/127`). RTM #6 is 2026-09-27; rebuilding the table the door runs on, days before the door opens, is the wrong trade. Production had 0 duplicates on 2026-09-22, so it applies cleanly — but re-run `scripts/verify/thb_duplicate_report.sh` first, because RTM #6 adds ~24 rows and that count is a fact about a Tuesday, not a property of the table. |
