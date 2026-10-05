# Plan 042: Phase 0 of the BeThere build plan (hackathon, 12 Oct)

**Created:** 2026-10-06, session `event-checkin-87`.
**Trigger:** the owner's handoff
`/Users/ozone/solana-thailand-devrel-helper/reports/phase-2/BETHERE-BUILD-PLAN.md`
(with ASKS-4, ASKS-5-SANDBOX, LANDING-PREFLIGHT, FEEDBACK-RTM6-TALK and the
`bethere-ux/` prototypes). Its "Rules for the whole build" apply to every item
here; the numbering 0.1–0.6 is the handoff's.
**Order:** the handoff's cut line: 0.1, 0.2, 0.3, then the sandbox must-cut,
then the landing, then the share tags. Each item ships on its own.

## Constraints

- **The 8 Oct take.** `.plans/039`: from 6 Oct, do not destabilize the
  demo-facing flows (event, deposit, ticket, claim, scanner). 0.1–0.3 do not
  touch them. The landing (0.5) replaces a demo-facing page, so it is built
  and shown on staging, and goes to prod only on an owner go after the take.
- **Prod deploys stay owner-gated, one go per deploy.** Staging is not.
- **Honest labels and numbers:** every number on the landing comes from 0.3,
  with a measured-at time; LIVE / DEVNET / PROPOSED / sample only as shipped.

## 0.1 `claim_refund` tool in `bethere-mcp`

- Same shape as `pay_deposit`: read `/api/deposit/status/{attendee}` (it
  carries `event_end_ms`, `refund_deadline_ms`, `checked_in`), refuse outside
  the program's refund window (after the end; a no-show only until the
  deadline), `POST /api/escrow/refund` with the agent's wallet, sign locally,
  send to devnet, return the signature and an Explorer link.
- The Worker already refuses a wallet that is not the deposit of record, so
  the agent can only claim what it paid.
- The window rule lives in the frontend today (`refund_window_open_at`); the
  tool gets its own copy with the same tests. Moving both onto one domain
  function waits for after the take (it touches the deposit page).
- **Done when:** an agent books, the person is scanned, the agent claims back
  on devnet, and Explorer shows all three transactions.

## 0.2 Remove the "Non-custodial" footer claim

- `footer.non_custodial` in both locales and wherever it renders. Replace with
  the status line the handoff gives ("Deposits: THB, organizer-held · escrow:
  devnet"), and add "(on devnet for now)" to `faq.deposit_a` and
  `faq.org_settle_a`.
- **Done when:** the phrase is gone from every page.

## 0.3 `GET /api/public/stats`

- Aggregates only, no PII, cached a few minutes, with `measured_at`.
- Fields: events held (completed and not cancelled, not `COUNT(*)`), on-site
  and online registrations (invite-only rows excluded), door scans, deposits
  handled (verified THB cash or credit; amount and count), payers who came.
- **Done when:** the definitions in ASKS-4 §5 hold, checked against prod D1
  read-only, and the landing reads only this.

## 0.4 Sandbox on staging `/sandbox` (must, then should)

- **Must:** a stranger with no login creates a test event, joins with a
  wallet, is checked in, and claims back on devnet.
- **Should:** a browser burner wallet with a faucet; the light agent path.
- **Guards:** Turnstile, rate limits, 24 h expiry, devnet only, names as plain
  text, no email. No hosted agent.
- **Needs the owner:** the sandbox organizer and faucet keys (who holds them,
  how much devnet USDC), and a Turnstile site key. Without a Turnstile key the
  build uses rate limits only and says so.

## 0.5 The new landing

- Section order from the handoff; numbers from 0.3; both languages and themes.
- Built behind the staging deploy; prod after the take, on an owner go.

## 0.6 Head tags

- A 1200×630 share image (TH and EN) and the logo favicon.
- **Done when:** a shared link previews on X, Facebook and LINE.

## Status

- [ ] 0.1 · [ ] 0.2 · [ ] 0.3 · [ ] 0.4 · [ ] 0.5 · [ ] 0.6
