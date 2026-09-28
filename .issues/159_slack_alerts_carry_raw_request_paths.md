# 159: Slack alerts carry the raw request path

**Status:** fixed on develop 2026-09-28 (session `event-checkin-af`); not deployed to prod. Found while writing `docs/pdpa_ropa.md` (§16).

## What happens

`middleware::alert::slack_alert_layer` builds the 5xx and spike alert text
from `req.uri().path()` unchanged (`worker/src/middleware/alert.rs:44`, used
in the `format!` that posts to Slack). Several paths carry durable
identifiers or capability tokens: `/api/claim/{token}`, `/api/public/ticket/{id}`,
wallet addresses in escrow paths. Any of those that 500s puts the token or
address into Slack, outside the log redaction Issue 070 applies to the log
stream. `/api/admin/test-alert` also includes the super-admin's email in the
message (`worker/src/handlers/attendee/admin.rs:102`).

## Fix

Send the matched route template (e.g. `/api/claim/{token}`) or the redacted
path, not the raw one, and fingerprint the email in the test alert
(`state.log_fingerprint`). Add a guard test in `worker/tests/` that the alert
text is built from the template.

## Fix (on develop)

`worker/src/alert_path.rs::alert_path` keeps route words (short, lowercase,
`-`/`_`) and turns every other segment into `{id}`. Both the 5xx and the spike
alert format it; `classify` still sees the real path. The test alert names the
requester by `state.log_fingerprint`. `worker/tests/alert_path.rs` pins both
(formatting the raw `{path}` again fails it).
