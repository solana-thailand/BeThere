# Web Verification Runbook

Use this checklist to verify BeThere through the deployed web application. Run
write tests on staging only. Production checks in this document are read-only.

## Environments

| Environment | Base URL | Allowed verification |
|---|---|---|
| Staging | `https://bethere-staging.solana-thailand.workers.dev` | Read-only checks and disposable test data |
| Production | `https://bethere.solana-thailand.workers.dev` | Read-only smoke checks unless a release plan explicitly authorizes writes |

Risk labels used below:

- **R0** — read-only, no external side effect, no expected cost.
- **R1** — writes disposable staging data; no blockchain or provider action.
- **R2** — wallet signature, payment, refund, message delivery, or NFT mint. Run
  only with dedicated staging fixtures and capped devnet wallets.

Record the environment, deployed version ID, tester, timestamp, event ID/slug,
and result for every R1/R2 session. Never paste claim tokens, attendee email,
wallet signatures, API keys, or OAuth tokens into an issue or chat.

## 1. Deployment smoke checks (R0)

Open the base URL in a private browser window. Confirm the landing page renders,
navigation works, and the browser downloads no HTML/JS file.

```sh
BASE_URL=https://bethere-staging.solana-thailand.workers.dev

curl -fsS "$BASE_URL/api/health" | python3 -m json.tool
curl -fsSI "$BASE_URL/"
curl -fsS "$BASE_URL/api/public/events" | python3 -m json.tool
```

Expected health properties:

- `status` is `ok` and `d1.connected` is `true`.
- Legacy `cluster` equals `solana.escrow_cluster`.
- Staging uses `devnet` for RPC, NFT, and escrow.
- `solana.warnings` explains incomplete or mismatched configuration. Staging
  currently reports `nft_not_configured` until a free staging Crossmint
  collection and key are installed.
- The root response is `text/html`. Hashed JavaScript is `text/javascript` and
  WASM is `application/wasm` in the browser Network panel.

## 2. Public pages (R0)

Verify at desktop width and at 390 x 844 mobile width:

| Page | URL | Expected result |
|---|---|---|
| Landing | `/` | Upcoming events render; primary navigation and footer links work |
| Event detail | `/e/<slug>` | Name, dates, venue/online mode, capacity and payment wording match the selected event |
| Past events | `/past-events` | Only completed events with published recaps appear |
| Recap | `/events/<slug>/recap` | Published content renders; unknown/private recap fails safely |
| Privacy | `/privacy` and `/data-privacy` | Policies render without authentication |
| Unknown route | `/route-that-does-not-exist` | Branded Page Not Found view appears |

In DevTools, confirm there are no failed JS/WASM requests, uncaught exceptions,
mixed-content errors, or API responses cached for a different user.

## 3. Authentication and admin readiness (R0/R1)

Use the staging admin identity. Opening `/admin` while signed out must redirect to
`/login`. After login, confirm the selected event stays stable while navigating
between Events, Attendees, Deposits, Quiz, and Audit views.

On the Events view:

1. Confirm quiz readiness warnings appear for active/quiz-enabled events with no
   valid enabled question.
2. Open the browser Network panel and inspect `GET /api/events/readiness`; it
   must return only events visible to the logged-in organizer.
3. Create or duplicate a **draft** test event (R1). Attempt to activate it with
   quiz enabled and no valid quiz. Activation must fail with a useful message.
4. Add one enabled quiz question with a valid answer, save, and retry activation.
5. Archive the disposable event when finished. Do not force-delete an event
   with deposits, attendees, or an active escrow.

## 4. Registration and attendee UX (R1)

Use a disposable staging event and test account:

1. Open `/e/<staging-slug>` and submit a new registration.
2. Confirm success links to `/ticket/<attendee-id>` and persists after refresh.
3. Submit the same registration again. It must resume or report the existing
   registration rather than create another attendee.
4. Sign in on a fresh private window and verify My Registration recovery.
5. Check both online and in-person participation choices where the event allows
   them.
6. In admin, verify exactly one attendee record and an audit entry exist.

Do not test capacity races, credit spending, slip verification, or wallet
transactions on production.

## 5. Quiz, adventure, ticket and claim pages (R0/R1)

With a staging claim token kept private:

| Check | Expected result |
|---|---|
| Open `/claim/<token>` | Attendee/event state loads without minting |
| Quiz enabled but not passed | Claim action is blocked and links to the required lesson/quiz |
| Submit a wrong answer | Attempt/retry UX is clear and no NFT request occurs |
| Submit a passing answer | Status survives refresh and claim eligibility updates |
| Already passed | Reopening the quiz does not erase progress |
| Open `/ticket/<attendee-id>` | Deposit, check-in, quiz and NFT states agree with server data |

Viewing a claim page is safe. Pressing its mint/claim button is **R2**.

## 6. Deposits and refunds (R1/R2)

For R1, verify only UI and server state using a disposable staging attendee:

- Currency labels say THB or USDC accurately and show the Solana network before
  wallet signing.
- Pending, rejected, expired, verified, refunded, and held-as-credit states have
  distinct actions and recovery instructions.
- Admin deposit filters agree with the attendee ticket.

USDC transfer, escrow initialization, refund signing, PromptPay slip processing,
credit spending, and message delivery are R2. Run them only through the staging
flow harness with dedicated fixtures. Confirm transaction signatures on devnet
and return test rows to a documented terminal state.

## 7. NFT verification (R0/R2)

R0 checks never call Crossmint:

```sh
curl -fsS "$BASE_URL/api/health" | python3 -m json.tool

cd worker
npx wrangler d1 execute bethere-db-staging --remote --env staging \
  --command "SELECT status, COUNT(*) AS jobs FROM nft_mint_jobs GROUP BY status ORDER BY status;"
```

Expected invariants after an authorized R2 staging mint:

- One journal row exists for the event and claim token.
- A retry uses the same opaque provider mint ID and does not create a second NFT.
- A successful row advances `pending -> confirmed -> persisted`.
- The attendee asset/signature exactly match the persisted journal row.
- A confirmed projection failure returns a retry message rather than an
  untracked success. Daily reconciliation repairs a safe missing projection and
  alerts when confirmed data conflicts or pending remains older than one hour.

Staging Crossmint is not configured at the time this runbook was added. Do not
run an R2 mint until health reports `nft_configured: true`. Mainnet minting may
cost money and is outside this checklist.

## 8. Production read-only verification (R0)

```sh
PROD_URL=https://bethere.solana-thailand.workers.dev

curl -fsS "$PROD_URL/api/health" | python3 -m json.tool
curl -fsSI "$PROD_URL/"
curl -fsS "$PROD_URL/api/public/events" | python3 -m json.tool
```

Also open `/`, one public `/e/<slug>`, `/past-events`, `/login`, and one published
recap. Stop and roll back the release if health loses D1, static assets have the
wrong content type, public APIs return 5xx, or the browser cannot initialize.

Do not register, check in, alter events, verify deposits, sign wallet messages,
send notifications, refund, or mint during a production smoke test.

## 9. Evidence and rollback

Attach only non-sensitive evidence: deployed version ID, HTTP status/content
type, health warning codes, route checked, timestamp, and redacted screenshots.
For rollback commands and traffic rollout, follow
[`gradual_deploy_runbook.md`](gradual_deploy_runbook.md). For staging setup and
isolation, follow [`staging_deploy_runbook.md`](staging_deploy_runbook.md).
Crossmint configuration and cost boundaries are documented in
[`crossmint-minting.md`](crossmint-minting.md).

