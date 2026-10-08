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

- [x] **0.1** `8f67b707`. Verified on staging + devnet 2026-10-06: deposit
  `4FypX24c…`, `mark_checked_in` `3zmwz7ki…`, refund `2xdP8JSu…`, all
  finalized, agent USDC back at 38; a second claim says "already claimed"
  (`bethere-mcp/README.md`).
- [x] **0.2** `8773eaaa`. The phrase was rendered only by the landing footer.
  Open for 0.5: the landing FAQ `deposit_a` still carries
  `{{ never_forfeited }}` and the credit exit, against build-plan rule 3.
- [x] **0.3** `85296121`. Prod, read-only, 2026-10-06: 14 held, 141 on site,
  111 door scans, 385 online, 68 deposits / ฿34,000, 50 of 54 on-site payers
  came. Differences from the handoff literals, all on purpose:
  - 14, not 13: RTM #6 ran on 4 Oct and is still `active`.
  - 141, not 142: one in-person row belongs to an event that no longer exists.
  - "73 of 76 (RTM #1–#5)" and the RTM #1 ladder (25/45, 25/30, 16/16) are
    not in D1: deposits before RTM #4 were taken by hand. D1 holds RTM #4–#6
    only, and RTM #6 was postponed by the flood (several payers moved
    online). Owner call: import that history, or show only what the system
    holds.
- [ ] 0.4 · [x] 0.5 landing release 1 on prod 2026-10-06 07:27 UTC (`6eb39e9e`, version `a518d80a`, tree = `0010f115`); L8–L10 follow in release 3
  **0.4 blocked (2026-10-06, `event-checkin-42`):** needs the owner's sandbox organizer + faucet keys and a Turnstile site key, and ships only via a staging push.
  **0.5 blocked (2026-10-06, `event-checkin-42`):** built on `feature/042-landing` (peer `event-checkin-0d`, `.plans/043`); L8–L10 wait for owner content, L12 for the CI baselines, prod for an owner go after the take.
  **Still waiting (2026-10-08, `event-checkin-19`):** 0.4 on the owner's sandbox organizer + faucet keys and a Turnstile site key.
  **Checked (2026-10-08, `event-checkin-fe`):** no sandbox work on any branch. The Turnstile key is optional (the plan allows rate limits only). **Owner must:** decide who holds the sandbox organizer and faucet keys and how much devnet USDC to fund them with. Also blocked by overlap: a `/sandbox` route edits `frontend-leptos/src/lib.rs` and `pages/mod.rs`, which `event-checkin-f0` has open for R4.0 in `/tmp/ec-f0-r4`, so it starts after R4.0 lands.
- [~] **0.6** built on `develop`: `og:image` / `twitter:image` were the 400×400
  `/api/badge.svg`, which X, Facebook and LINE do not render; now
  `/og-image.png`, 1200×630, one bilingual card (crawlers do not run the SPA,
  so the head cannot pick a language). Favicon and apple-touch-icon are the
  brand PNGs (64, 180). Source `frontend-leptos/share/og-card.html`, re-render
  with `share/render_og.mjs`. The tags are absolute prod URLs, so the
  "Done when" (X, Facebook, LINE previews) can only be checked after a prod
  deploy, with each platform's debugger. Not in scope: per-event previews
  (needs the Worker to rewrite the head of `/e/*`), PWA manifest icons
  (still the SVG).
  **0.6 on prod (2026-10-06, `event-checkin-42`):** the prod head carries `og:image`/`twitter:image` = `/og-image.png` (200 `image/png`) and the PNG favicons. Left: paste a link into X, Facebook's Sharing Debugger and LINE; each needs a signed-in account, so a person does it.
  **Crawler probe (2026-10-08, `event-checkin-8a`):** prod `/` fetched with the facebookexternalhit, Twitterbot and LINE (`line-poker`) user agents returns the same head: absolute `og:image` = `twitter:image` = `/og-image.png`, `og:image:type` png, 1200×630, `og:description`, `og:url`, `twitter:card` `summary_large_image`. The PNG answers 200 `image/png`, 218,761 bytes, really 1200×630 (`file`). Every documented requirement is met; only the in-app render remains, and it needs a signed-in person.
- [x] **Phase 1.0 (Thai-safe slugs), early,** on `develop`: the four builders
  are one `event_checkin_domain::slug::Slug`. ASCII names keep the slug they
  had; a name that keeps under 3 ASCII characters after dropping non-ASCII
  letters (all Thai, emoji, "ครั้งที่ 1") gets a deterministic FNV-1a hash:
  `event-{6}` / `org-{6}` / `series-{6}`. Deterministic, not random, because the
  feedback series key is recomputed per request. Tests
  `domain/tests/thai_safe_slug.rs` (hash pinned). Not touched:
  `campaigns_page::slugify` (campaign slugs, own length cap).
- **Staging, 2026-10-06:** `a43fba5c` (0.1–0.3, 0.6, Thai-safe slugs) is
  version `52d1c9ec`; CI green on that SHA (after three GitHub runner-outage
  re-runs); smoke reads + writes pass; the page was opened in EN and TH.
- **Prod, 2026-10-06 (owner go, `event-checkin-42`):** release merge
  `b8bd6010` (tree = `develop` `7dd27d40`), version `e0f11214` at 100 %; the
  parity gate passed with staging version `b2f8e113` (same tree, write smoke
  passed), no `--force`. No migrations; D1 backup taken first. Checked: stats
  match the 0.3 numbers (14 / 141 / 385 / 111 / 68 / ฿34,000 / 50 of 54),
  the footer shows the 0.2 status line in EN and TH with no "Non-custodial",
  and the share tags are as above. Prod write smoke not run (no `SMOKE_TOKEN`).
  `thb_deposits` had no rows on 5–6 Oct (1–4 a day on 1–4 Oct, before the
  deploy, after RTM #6); re-check later in the day.
  0.5 is on
  `feature/042-landing` (`.plans/043`), not on develop.
