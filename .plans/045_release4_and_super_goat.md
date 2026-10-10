# 045 · Release 4 and Super GOAT

Status: in progress. Owner approved the plan and the start on 2026-10-08
(session `event-checkin-f0`), ahead of the 13 Oct date first written here;
confirmed again 2026-10-10 (session `event-checkin-d3`). **Deployed to prod
`05db3a46` 2026-10-10** (owner lifted the freeze; release pull 191, staging `406921ba` first, 20/20 page × theme × viewport checks on both):
R4.0 + R4.5 + R4.6 (pull 184, review fixes, phone nav pull 187), R4.7
(pull 185), SG3 own-wallet (pull 188) and the sandbox RPC 429 retry (pull 190;
sandbox stays off on prod). **R4.2 deployed to prod `ff8a1a24` 2026-10-10** (main `ddcced44`, release pull 194; staging `04a2a117` first; 20/20 page checks and the hall probe on both). **R4.1 deployed to prod `4a74bbc2` 2026-10-10** (main `bb986e18`, release pull 197, with the deploy gate pull 196; staging `30df4071` first). Baselines reset to that build (pull 199). **Site parity pass deployed to prod `755a3750` 2026-10-10 (pull 200, release pull 201):** a page-by-page comparison with the prototype found the pages only half ported (the home still the old long landing, `/organizers` only the swimlane, `/events` a bare list, no page heads); pull 200 adds the page heads, the full `/organizers`, the home trim and R4.3. **R4.12 deployed to prod 2026-10-10** (pull 202, release 203; then the one-cron fix pull 204, release 205, prod `4f43669a`): migration 0060 on staging and prod; Gmail secrets on staging and prod; a real test mail went from staging at 12:17 UTC. Next: R4.4 (course pages), R4.8, R4.9. Originally: plan, for the owner's review. Written 2026-10-08 by session
`event-checkin-90` (`.plans/044` item 6). Basis: the prototype
`bethere-ux/site/` in the devrel-helper repo (index, events, course,
organizers, sponsors, try, record; shared `site.js`/`site.css`/`room.js`/
`returns.js`/`stats.js`, ~2,000 lines; each page's `.proto` line says what is
prototype) and `reports/phase-2/SUPER-GOAT-SPEC.md`.

Estimates are working days for one agent, including tests, a staging pass
and the owner's look; prod deploys are separate, owner-gated steps.

## Constraints every line respects

- **Two shells.** Every new page lives in the attendee shell. The staff shell
  was at 95.3 % of the 2 MiB ceiling; since pull 181 (prod 2026-10-10) it hands
  every attendee page to the attendee shell and sits at 63.5 %, but new pages
  still go in the attendee shell only.
- **First load.** The attendee first load is +35.9 KB over the 6 Oct baseline
  after Release 3 (warn, under fail). New pages are routes in the same wasm, so
  each line states its expected growth; anything heavy (canvas art, data) is
  lazy JS or JSON, like the globe.
- **Numbers.** Every number comes from `GET /api/public/stats` or the typed
  hand-record table (`story.rs` today, moving to `domain` in R4.7), each with
  its source and measured date. No other literal.
- **Free plan.** 10 ms CPU per request: no image resizing or HTML rendering of
  size in the worker (R4.8, R4.9 are shaped around this).
- **Routes already taken.** `/events/:slug/recap`,
  `/events/:slug/post-event-register` (attendee) and `/events/:id/summary`,
  `/events/:id/pr-pack` (staff). `/events/<series>` is one segment, so it does
  not collide, but series and event slugs share one namespace: R4.0 makes
  series slugs carry a reserved prefix or checks both tables on create.
- **Prod gates.** Subscribe is not shown as a promise until R4.12 sends mail;
  the `/try` band does not appear in prod until SG3 ships.

## R4.0 Routes, doors and the try band (1.5 d)

- What: route map `/` (story), `/events` (new; `/discover` becomes a client
  redirect, the crawler `APP_ROUTES` pin and sitemap updated),
  `/events/<series>`, `/organizers`, `/sponsors`, `/try` (feature-flagged off
  in prod). One `Doors` component (three doors) and one `TryBand` component
  used by every page; the organizers swimlane gets the try line.
- Files: `frontend-leptos/src/lib.rs` routes, `pages/site/doors.rs`,
  `pages/site/try_band.rs`, `worker/src/crawl.rs` (`APP_ROUTES`, sitemap),
  `worker/tests/crawl_routes.rs`.
- Proof: the crawl-routes test pins both directions; e2e visual for each page;
  a test that `TryBand` renders nothing when the flag is off.
- Go: none until the prod deploy.

- **Built (2026-10-08, `feature/r4-0-site-routes`):**
  - `pages/landing/frame.rs` `SiteFrame` (header, doors, footer; the auth
    check and stats fetch moved out of `page.rs`), `pages/site/doors.rs`
    (`SitePage`, `Doors`, the try band and try line), `/organizers` (the
    swimlane as is, R4.5) and `/sponsors` (the sponsor section with `#contact`,
    R4.6). The header now links pages (Events & courses · For organizers ·
    Sponsors) as the prototype does; the landing's section dots stay.
  - `/events` shows the existing Discover list until R4.3; `/discover` is a
    client redirect to it (query and hash kept, history replaced). In-app
    links and the boot summary point at `/events`; the sitemap lists
    `/events`, `/organizers`, `/sponsors` and no longer `/discover`.
- **Reviewed and merged (2026-10-10, `event-checkin-d3`, pull 184 → develop
  `c8a05bf5`; phone nav pull 187):** replayed on develop with the staff
  hand-off and `/sandbox`; `/events` is now `site::EventsPage` (the Discover
  list inside `SiteFrame`, title "Events & courses"); one `h1` per page
  (`page_title` prop on `HowItWorks` / `Sponsors`); the door section and the
  `/events` list follow the site theme (`.lp-events-sec` maps the app colour
  variables; date chips 4.69–6.12:1, axe green); Linux baselines re-taken on
  CI and checked by eye. Phones get the page links as a second header row,
  and the current page is an underline (the app's orange `nav a[aria-current]`
  block is cancelled inside `.lp`). Size vs develop: attendee +1.8 KB,
  staff +0.3 KB. Check every site page in light **and** dark (CI baselines
  are light).
  - The staff shell hands the three pages to the attendee shell, as it does
    `/`, so none of them grows the staff wasm.
  - **Deviation:** no `/try` route yet. Its content is SG3; a flagged-off
    route would be an empty page. `TRY_LIVE` (false) gates the band and the
    line, and `tests/site_pages.rs` fails if it and a `/try` route disagree.

## Pages

| line | what | files / data | proof | est. |
|---|---|---|---|---|
| R4.1 | **Deployed 2026-10-10 (pull 195, prod `4a74bbc2`):** room.js + rtm6-room.webp off the first load (+1,797 B), night hero in both themes, quiet boxes, reduced motion = drawn once dim; lamps wait for R4.4. Original: `/` lit room: a 320×180 grey image dithered on a canvas; the pointer lights it and keeps off the words; each episode watched keeps one light on (localStorage) | `frontend-leptos/room/room.js` (lazy, like `globe/`), image in R2 or `media/`; no wasm growth beyond a mount bridge | reduced motion = still image; e2e masks the canvas; first-load delta < 2 KB | 1.5 d |
| R4.2 | **Hall deployed 2026-10-10 (pull 193, prod `ff8a1a24`)**: the hall replaced the hero ticket loop (owner's pick); `landing/hall.rs`, hall total = ladder total (`tests/landing_hall.rs`). The home itself was still the old landing; trimmed to hero → your registrations → doors → so far + goal in pull 200. Original: `/` story and evidence: the payers' hall by event (R4.7), so far, came back, globe, three doors | reuse `story.rs`, `sofar.rs`, `goal.rs`; hall = one chair per payer per event | hall total = ladder total (test) | 1 d |
| R4.3 | **Deployed 2026-10-10 (pull 200, prod `755a3750`):** open events in the head, else the empty state from `domain::models::catalogue::cadence` (a public-only snapshot of the 14 past events: the API lists past events only once a recap is published, and events have no series or recording field until R4.4); learn from past events (episodes open their recordings); the deposit strip; 90/94. Subscribe arrived with R4.12 (pull 202). Original: `/events`: open events and courses; empty state computed from the catalogue (times run, median gap, last date), subscribe as the main action, last episode, then devnet · Discord · host your own | `pages/site/events.rs`; catalogue from `/api/public/events` + past events | a pure `empty_state(catalogue)` with tests (median of gaps, no open events) | 1.5 d |
| R4.4 | **Deployed 2026-10-11 (pull 216, release 217, prod `61c3c6c6`):** source is `domain::models::catalogue` (prod has no usable campaigns and events expose no recording; `.issues/068` keeps campaigns out of the MVP); migration 0061 `course_enrolments` + `course_progress` keyed by email (not `person_id`); attendee-authed `/api/courses/{course}/progress|enrol|watched`; `.issues/068` rule in `worker/tests/courses_guard.rs`; `/events/x` stays a 404. Original: `/events/<series>` course page: register once, watch episode by episode, progress | `campaigns` / `campaign_events.sequence_order` (prod has 1 campaign, 1 link today); new migration `course_progress(person_id, campaign_id, event_id, watched_at)`; worker `GET/POST /api/courses/<slug>/progress` (authed) | `.issues/068` rule as a test: retrospective registration is never check-in, attendance, deposit or badge; migration rehearsed + write-volume check after deploy | 3 d |
| R4.5 | **Rebuilt in pull 200, deployed `755a3750`:** the prototype's page, not only the swimlane: head → the room narrowed in three steps → how it works → "Try it on your event" (`facts::expected_came`) beside the organizer card. Original: `/organizers`: "How it works" is the existing swimlane (`how.rs`, `landing.json` "how") as is; the try line under it | reuse | swimlane test unchanged | 0.5 d |
| R4.6 | `/sponsors`: the existing sponsor section as its own page, the contact card anchor kept | reuse `sponsors.rs` | link test for `#contact` | 0.5 d |

## Worker

| line | what | files / data | proof | est. |
|---|---|---|---|---|
| R4.7 | **Done on develop 2026-10-10 (pull 185, `3fd1d524`):** `domain::models::facts` holds the ladder and room; `PublicStats.payers_by_event` (public events only, `public_stats_by_event.sql`). Checked on prod D1 read-only: RTM #4 16/14, #5 14/13, #6 21/20 = the System rows; RTM #2 14/14 vs the table's 15/15 is the owner's hand-recorded payer. Original: Facts: `/api/public/stats` plus the hand-recorded facts as one typed constant set in `domain` (the ladder table moves there; the frontend and any worker page read the same set); per-event payers for the hall | `domain/src/models/facts.rs`; `public_stats.sql` gains a per-event `paid` grouping (staff/comp/online out, same rule) | the hall's per-event rows equal the ladder rows for #4–#6 (test against fixtures) | 1 d |
| R4.8 | **Done by `.issues/183` (pull 167, deployed 2026-10-09); checked on prod 2026-10-10:** a crawler fetch of `/e/<slug>` gets the event's own `og:title`, `og:description`, `og:image` (its poster) and `twitter:card summary_large_image`. Original: Per-event OG image: the event poster resized for 1200×630 in the organizer's browser at upload (not in the worker: CPU cap), stored beside the poster; `og:*` per `/e/<slug>` via the worker's HTML response | `poster.rs` upload path, `frontend-leptos` uploader, worker HTML meta rewrite for `/e/*` | crawler fetch of `/e/<slug>` shows the event's own `og:image` and title | 1 d |
| R4.9 | **Deployed 2026-10-10 (pull 211, release 213, prod `b6c59edc`):** `/` joins `run_worker_first`; `worker/src/home.rs` splices the opening into `#boot-summary`; 17.3 KB; staging cpuTime median 1 ms, max 6 ms (`.benchmarks/008`). Original: Static no-WASM home: worker-rendered HTML for `/` with the same numbers (first paint, crawlers, no-JS), the SPA hydrates over it | `worker/src/home.rs` (small template, numbers from R4.7), served by `crawl::route_kind` for `/` only | HTML < 20 KB; CPU < 5 ms (bench note in `.benchmarks/`); same numbers as the SPA (test) | 2 d |
| R4.10 | Aggregate click counters without cookies: page × door × day, no ids, no IP | migration `door_clicks(day, page, door, n)`; `POST /api/public/click` (rate-limited, sendBeacon) | no personal data in the table (schema test); counts only | 0.5 d |
| R4.11 | Meals not ordered per event: registered − came, against what the organizer ordered | new event field `food_ordered` (admin form), aggregate `meals_not_ordered` per event in stats | aggregate only; shown only when the organizer entered a count | 1 d |
| R4.12 | **Built 2026-10-10 (pull 202):** Gmail API sender (owner's free path), not `[[send_email]]`: migration 0060 (`subscribers`, `announced_events`, `event_announcements`; open events seeded as seen), `POST /api/subscribe` + `POST /api/unsubscribe/{token}` + `/unsubscribe/{token}`, hourly cron `17 * * * *` at 16 mails a run, claim before send, PDPA erasure and event purge cover the tables (`docs/gmail_sender_setup.md`). Original: Subscribe that sends: the waitlist moves from the Sheet tab to D1 (dedupe, the register form's consent wording, a one-click unsubscribe token); one email per newly opened public event to everyone on it, via Cloudflare Email Sending | migration `subscribers(email, consent_at, unsub_token, created_at)`; `[[send_email]]` binding; the notifications outbox reused for retries | unsubscribe works without sign-in (test); one email per event per subscriber (idempotency key); Turnstile on subscribe | 2.5 d |
| R4.13 | **Deployed 2026-10-10 (pull 207, release 208, prod `4baf1431`):** THB only (USDC is the devnet rail). Original: Signed in with credit held: the empty state shows the balance (`/api/deposit/credit-balance`) and that it covers the next deposit | `events.rs` empty state | shown only when signed in and balance > 0 | 0.5 d |

**R4.12 is blocked on the owner:** Cloudflare Email Sending needs a sender
domain onboarded to Cloudflare (`worker/wrangler.toml` has `[[send_email]]`
commented out for that reason; the site runs on `workers.dev`). Until a domain
is set up, R4.3 ships the subscribe as "we'll tell you" only after R4.12, or
without the subscribe at all.

**Owner, 2026-10-08: try the free path first.** Cloudflare Email Sending to
arbitrary recipients needs Workers Paid ($5/month) and a sender domain on
Cloudflare DNS, and there is no custom domain today. Free candidates, in
order:
1. Gmail API from a dedicated project Gmail account (one-time OAuth consent,
   refresh token as a worker secret, HTTPS from the worker; consumer Gmail
   allows about 500 recipients a day). Sent by Gmail itself, so it passes
   SPF/DKIM/DMARC. Needs the OAuth app published, because refresh tokens issued
   while it is in "Testing" expire after 7 days.
2. Apps Script `MailApp` bound to the waitlist Sheet, called by the worker
   (about 100 recipients a day on a consumer account). Least code, lowest cap.
3. Brevo or Resend: free tiers need a domain we control. Revisit when there
   is one.
Keep the one-click unsubscribe and the `List-Unsubscribe` header on every
path. Sender (owner, 2026-10-08): **bethere.sol@gmail.com** via option 1.
Setup: `docs/gmail_sender_setup.md` (owner steps 1–6, then the consent
helper `scripts/gmail_refresh_token.py`). Waiting on: the owner running
those steps; the build itself starts with Release 4 (not before 13 Oct).
Branding check (2026-10-08): the consent screen requires a home page and
privacy link, and Google rejected the prod origin as not registered to the
account. Search Console ownership tag shipped to prod (pull 163, `main`
`a911a575`, version `29e787a2`); owner: Verify in Search Console, wait 24 h,
then "I have fixed the issues". Until then the app may stay in Testing
(test user bethere.sol, 7-day refresh tokens): fine for staging, not for
real sends.
Setup done 2026-10-08: branding verified and published, app In production
(data-access verification for `gmail.send` deliberately skipped: one
consenting account, under the 100-user cap). Desktop client in Cloud project
`serene-anagram-511006-j9` (`bethere-mail`); consent as bethere.sol; the
refresh token exchanged for a 1 h access token with scope `gmail.send` only
(no mail sent). Files `~/.bethere-gmail-client.json` and
`~/.bethere-gmail-refresh-token` (600) wait for R4.12 to load them as
staging secrets.

## Super GOAT (after Release 4, in the spec's order)

| line | what | est. |
|---|---|---|
| SG1 | `domain::record::door_history(person_id)` (the single definition: RTM meetups, door check-ins, staff/organizers and online excluded, merged emails once, smoke events out; 20 regulars today), `GET /api/me/record` (own record only), `/me/record`, per-event `events.regular_rule` JSON evaluated at registration (register form + ticket say why the seat costs ฿0), admin rule editor with the count of qualifying registrants | 4 d |
| SG4 | "I showed up" card: `GET /api/me/showed/:slug.png` only if `door_history` says came (pre-rendered per event by ffilms, owner's call on cost), share button on the ticket and `/me/record`, public `/showed/:token` (random, revocable) with OG tags | 2 d |
| SG3 | **Own wallet added 2026-10-10 (pull 188):** "Which wallet?" before step 1 (test wallet or a detected wallet, checked to be on devnet), signing through `pages/sandbox/signer.rs` with a confirmation wait, faucet.solana.com / faucet.circle.com links for the visitor's own wallet. Real-wallet signing still to be tried on staging. **Partly built 2026-10-09 as `/sandbox` (plan 042 0.4, pull 173; staging only):** the Worker-held devnet organizer key signs `create_event` and `mark_checked_in` (sandbox events only: the escrow is derived from that key), a Circle-USDC faucet the Worker signs for (one grant per wallet per day, 50 grants / 200 events per day, deposit rate limiter), browser burner wallet, Explorer links per step, return-to-faucet, 120 s events. **Left for SG3:** the standing daily event by cron, Turnstile, a per-IP faucet cap, the live tally, the `/try` route + `TryBand`, organizer-rent reclaim (close_event). Revised estimate ≈ 2 d. Original line: `/try` devnet sandbox: a standing daily sandbox event by cron, rolling `event_end` (60 s), a devnet USDC faucet the worker controls (Turnstile, 3 per hour per IP and per wallet), the worker signs `mark_checked_in` for sandbox events only (separate keypair, program-level event check), Explorer links per step, live tally; reuses `.plans/044` item 1's script and `docs/devnet-run-2026-10-08.md` | 4–5 d |
| SG2 | x402 design doc only. First the custody catch: x402 `exact` pays a `payTo` address, which would make BeThere custodial. Options: (a) a BeThere scheme whose payload is the signed escrow `deposit` transaction, verified (right event, right vault) then submitted; (b) `exact` to a vault PDA plus a server-side deposit record (the program cannot accept this today); (c) x402 only for non-refundable parts (none today). Plus the duplicate-settlement cache and the agent's 10 USDC cap. Recommendation, no build until the owner picks | 1 d |

## Totals and order

- Release 4: R4.0–R4.13 ≈ 17.5 d (R4.12 waits on the sender domain).
- Super GOAT: SG1 4 d, SG4 2 d, SG3 ≈ 2 d (was 4–5 d; see SG3), SG2 1 d ≈ 9 d.
- Order after 13 Oct: R4.0 → R4.7 → R4.2 / R4.1 → R4.3 (+ R4.12 when the
  domain exists) → R4.5 / R4.6 → R4.8 → R4.9 → R4.4 → R4.10 / R4.11 / R4.13
  → SG1 → SG4 → SG3 → SG2.

## Risks

- Sender domain (R4.12): no subscribe email without it.
- CPU cap: R4.8 and R4.9 are shaped to stay small; R4.9 gets a benchmark.
- First load: each page adds wasm; the budget's warn line is already crossed.
  Pages over ~5 KB br4 get their heavy parts as lazy JS.
- Migrations (R4.4, R4.10, R4.11, R4.12): apply before code, read the schema
  back, check write volume after each deploy (`.issues/138`).
- SG3 touches signing keys on the worker: a separate devnet-only keypair, never
  the organizer's.
