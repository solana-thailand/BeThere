# 043 · The new landing (build plan 0.5)

Status: planned, 2026-10-06. Branch `feature/042-landing` (not `develop`), so
no prod deploy before the 8 Oct take can carry it (`.plans/039` freeze).
Staging shows it; prod only on an owner go after the take.

Sources: `solana-thailand-devrel-helper/reports/phase-2/BETHERE-BUILD-PLAN.md`
(0.5 and the rules for the whole build), `BETHERE-ASKS-4.md` §6–19,
`LANDING-PREFLIGHT.md` §2–3, prototype `bethere-ux/landing.html` +
`LANDING-COPY.md` (every string, TH/EN).

## Section order (build plan 0.5)

hero → upcoming events (posters) → commitment ladder (chairs) → how it works
(swimlane) + why USDC + sandbox → so far · goal (reels, live strip, globe,
communities) → sponsors (placements, tiers without prices, contact card) →
join.

## Slices, each its own commit on the branch

| # | Slice | Needs from the owner | Done when |
|---|---|---|---|
| L1 | Paper/navy tokens for the landing only (`styles/style-23-landing.css`), light/dark switch that follows the system until picked | none | both themes pass WCAG AA on every text pair (checked, not assumed) |
| L2 | Hero: promise, sub-line with keyword links (`{word\|#id}` markup), the 6 s CSS loop, reduced motion = end state | none | matches the prototype at 390×844 and 1440×900, TH and EN |
| L3 | Upcoming: nearest 3 (existing `UpcomingEvents` data), poster, one deposit line "฿500, all back when you show up", empty state → sandbox or Join | none | no "credit" on any card (rule 3, ASKS-4 §19) |
| L4 | How it works swimlane: 3 routes (baht LIVE, USDC DEVNET, agent DEVNET), tabs on mobile, `?track=` | none | labels match what ships; step 4 neutral ("the organizer sets the rule") |
| L5 | So far: numbers from `GET /api/public/stats` with `measured_at` shown; count-up once; static under reduced motion | none | nothing hard-coded; a stats failure hides the strip, not the page |
| L6 | Sponsors: placement wireframe, LIVE/PROPOSED tags, no prices, no other organisation's logos | contact card fields (name, email/LINE, .vcf) | tags match ASKS-4 §10 |
| L7 | Join: organiser waitlist (existing `WaitlistForm`), share button | none | the existing waitlist still posts |
| L8 | Ladder (chairs) | **which numbers**: D1 holds RTM #4–#6 only; "25/45, 25/30, 16/16" and "73 of 76" are hand records (`.plans/042` 0.3 note) | built from an API, or left out |
| L9 | Reels + photos | **consent covers publishing** (PREFLIGHT §2) and the files | until then: no photos of people |
| L10 | Globe + communities | data refresh source (`globe_data.py` on a schedule, or the stats API) | data is not a hand-pasted literal |
| L11 | FAQ: drop `{{ never_forfeited }}` and the credit exit from `deposit_a` (rule 3) | none | rg finds neither in `locales/*/landing.json` |
| L12 | Visual baselines: `/` at 390×844 and 1440×900, TH and EN, both themes (ASKS-4 §4) | none | CI e2e compares them |

Signed-in view (ASKS-4 §7) keeps today's `MyRegistrations`; reordering it is a
follow-up, not part of 0.5.

## Constraints

- One stylesheet for the landing, `style-` prefixed (feeds `_headers` caching,
  `frontend-stylesheet-split`); no new fonts in the first cut (Anuphan via
  Google Fonts as today; woff2 subsets are Phase 1.7).
- Strings through `locale::tr` (not `t!` per site: wasm bloat).
- Measure with `frontend_size_budget.sh` after each slice.
- Verify by opening the page (headless Chrome), both languages, both themes.

## Status

- [x] L1 tokens + light/dark (`style-23-landing.css`, `landing/theme.rs`,
  `bethere.theme`, system until picked). Tokens sit on `.lp`; only ported
  sections paint paper/navy, so unported ones keep the app palette until
  their slice lands. Contrast computed for every text pair, both themes:
  lowest 5.18 (small text), heading 3.95 (large), loop chip label bold.
  First load +7.7 KB (develop +61,459 → branch +69,135 vs the 29 Sep baseline).
- [x] L2 hero (`landing/hero.rs`, `utils/copy_markup.rs` + tests): checked at
  390×844 and 1440×900, TH/EN, light/dark, reduced motion: no overflow, six
  keyword links. The "why" film button waits for the film to be hosted
  (3.8 MB, click-to-load); the anchors `#story #usdc #goal #sponsors` land
  when those slices do.
- [x] L3 upcoming (`landing/upcoming.rs`, `landing/event_card.rs` + tests):
  nearest 3 by start time (TBA last), poster cover, the event's own THB
  amount (`deposit_amount_thb`, added to the public events listing on the D1
  path) in "฿N, all back when you show up", online chip, a test that the
  `upcoming` copy never says credit or forfeit. Empty state: the sample
  event if one is set, else Discord. No seat chips: the listing has no
  counts. 21 dead `landing-*` rules removed. Checked 390/1440, TH/EN, both
  themes (found and fixed the app's global `h3` colour winning in light).
- [x] L4 how it works (`landing/how.rs`, `landing/stats.rs` + tests): the
  swimlane, three routes (baht LIVE, USDC DEVNET, agent DEVNET), the last
  route on cannot be turned off, tabs on phones, `?track=sol|ai`. The baht
  timings are medians from `GET /api/public/stats`, now with `slip_check` and
  `refund_after_end` (second D1 statement: D1 rejects a 6-term compound
  SELECT). Prod, read-only, 2026-10-06: slips 68, median 5.7 min; refunds
  with a time only 8, median 67 h, not the handoff's "31 h over 22". A
  timing under 5 cases is not published, and the footnote names only the
  published ones with their sample size. Agent claim back is no longer
  "proposed" (0.1 shipped on devnet); the cap is "the cap you set", not a
  literal 10 USDC; "฿500 by PromptPay" became "the deposit by PromptPay".
  The "why USDC" panel and the sandbox block wait for 0.4. Checked at
  390/1440, TH/EN, both themes, with a mocked stats response and with stats
  failing; every colour pair ≥ 4.76:1.
- [x] L5 so far (`landing/sofar.rs`): on the night field, "from the system
  · measured {time}", the totals line and "50 of 54 who paid came" with rule
  4's caveat (pinned by a test, both languages), all from the shared stats;
  a failed stats call draws none of it. Counts up once in view (one-shot
  IntersectionObserver, 1.2 s ease-out, cleared on unmount); reduced motion
  shows the final figures; screen readers get the final figures from a
  visually hidden copy, never the zeros. Community row: our own name as a
  text wordmark plus "+ your community" (rule 5). The tape label repeats 32
  times so the loop never shows a gap at 1440 px.
- [x] L6 sponsors (`landing/sponsors.rs` + tests): six placements as
  wireframes with empty dashed slots (no other organisation's logo), LIVE
  for the three that ship (event-page row, poster line, group photo) and
  PROPOSED for recording card, ticket line and badge (pinned by a test); a
  category filter; tiers with no price (a test bans ฿ $ THB USD บาท in the
  sponsor copy); the contact card with the name, role, Facebook, Discord and
  a .vcf the owner gave the prototype. **Owner:** the prototype's avatar is
  a photo of a person, kept out of the public repo until you say so; email,
  phone and LINE were never given.
- [x] L7 join (`landing/join.rs`): "Build it with us", a share button
  (native sheet, else copy the link), and the organizer card with the same
  role logic `page.rs` had (admin/organizer → dashboard, signed in → DM,
  else the waitlist form, restyled in place). `#waitlist` anchors now point
  at `#join`. Checked signed out and signed in, both themes.
- [x] "Why USDC on Solana" (`#usdc`, under the swimlane, DEVNET tag): no
  gated content in it; the prototype's sandbox block stays out until 0.4.
  The hero's "show up" keyword points at `#goal` until the ladder (`#story`,
  L8) exists; a browser check finds no dead in-page anchor.
- [x] Footer: one row of links (how, FAQ, staff portal, Discord, X,
  GitHub), one of fine print with the 0.2 status line and the build
  version (the visual spec's mask moved to `.lp-version`). Under reduced
  motion the so-far figures show at once, not only after the strip is seen
  (a full-page capture showed zeros).
- [x] L12, PR #154 CI:
  - Size gate: was +110,135 attendee (limit 102,400). Fixed on develop
    `5a663964` (favicons quantized, 47.7 KB → 22.5 KB) plus
    event-checkin-ca's staff hand-off of `/` (`a024282b`, staff −99 KB).
    Now attendee +87,312 and staff +8,643; no baseline bump.
  - axe (run 37381847919): the so-far tape label, ink on `#6b63f0` = 4.16:1.
    Fixed in `58e43a02` (paper on `#5a54cb`, 5.18:1).
  - Visual: the live stats strip (`.lp-livebar`, `.lp-done`) is masked: the
    measured-at time changes each run and the figures count up only once
    seen. New Linux baselines, each checked by eye: signed-in ticket and
    privacy notice (identical in runs 37381847919 and 37383427171), landing
    desktop, landing mobile and the expanded ticket (run 37384976523).
- [ ] L8 ladder: waits on the owner's choice for RTM #2/#3 (system 14/14 and
  14/13 vs the hand 12/12: import to reach 73/76, or print 77/74 as the
  sources say). Not built.
- [ ] L9 photos: waits on consent.
- [x] L10 on `feature/046-landing-globe` (PR #157, stacked on #156 → #155;
  `event-checkin-d6`): goal bar (live `events_held` / measured 692+) and the
  globe, both loaded on first sight from `globe/` (`scripts/globe_data_import.py`
  imports the owner's `globe-data.js`); sponsor avatar 56/112 px lazy. First
  load +10,001 B br4 over the reset baseline (`1d12cb03`, 1,311,318 B).
  Browser-checked at 1366/390, EN and TH; new landing Linux baselines from CI.
  Stage after Release 2 is on prod.
- [x] L11 on the branch: `/faq` `deposit_a` no longer renders "never
  forfeited" or the credit exit; neutral "tell the organizer before the
  cut-off; the organizer sets the rule". Checked in the browser, EN and TH.
  `org_settle_a` (organizer tab) still names the credit: it tells organizers
  how settlement works, it does not sell the exit to attendees.
- **Open, after the take (frozen page):** the event page deposit section
  (`public_event/deposit_section.rs`) still renders `never_forfeited` (D1 in
  `utils/deposit_copy.rs`, from the 2026-09-28 policy round). Build-plan rule
  3 (6 Oct) says never write it; the newer rule wins, but the page is
  demo-facing until 8 Oct. When it goes, delete `NEVER_FORFEITED*` too.
- **Staging, 2026-10-06 (owner OK'd staging, and prod if staging looks right; `event-checkin-42`):**
  `develop` fast-forwarded to `d6367104` (CI all green), staging version
  `fe615b53`; smoke reads + writes pass. `/api/public/stats` carries
  `slip_check` / `refund_after_end` (null on staging, fewer than 5 timed
  deposits) and `/api/public/events` carries `deposit_amount_thb`. Opened `/`
  with the service worker bypassed at 390 and 1280, EN light, TH dark, EN
  dark, TH light: the sections render, there is no horizontal scroll, there is no
  "never forfeited" / "non-custodial", and the only error is the signed-out
  `/api/auth/me` 401. First load +88,069 B br4 vs the 29 Sep baseline (warn,
  under the 102,400 fail). Prod waits for the owner's "deploy prod" after
  looking at staging. L8–L10 are not built.
- **Releases, 2026-10-06 (`event-checkin-d6`):** Release 1 = PR #155
  (`a92c17f1`, on staging), waits for the owner's "deploy prod". Release 2 =
  PR #156 (`6580f921`, CI green incl. e2e; the 404 e2e now asserts the
  worker's real 404). Release 3 = PR #157 (draft; L10 + avatar).
- **Release 1 re-staged, 2026-10-06 (`event-checkin-d6`):** PR #155 head
  `0010f115`, staging version `162a45cf`. Films with "RTM #1–#5: 70 of 73"
  (sha256 en `f8ec15a8…`, th `c86b0d9b…`, served byte-identical from staging,
  206 on both), film URLs `?v=4` + edge key `v=4`. Live strip payers now
  exclude the event's staff/organizers (prod read-only: 68/64 → 65/61). Smoke
  reads + writes pass. Phone screenshots at 50 s, EN/TH:
  `~/Downloads/bethere-staging-film-70of73/`. Prod waits for the owner's
  "deploy prod".
- **L8 table (owner, 6 Oct):** one definition — in-person, cash or credit;
  comp, staff and online payers out; came = checked in. #1 16/16 hand record;
  #2 15/15 (14 archive + 1 owner record, plan 010 left it out of D1); #3 12/12
  (statement paired 1:1, confirmed from the 17 Sep dump: the non-checked-in
  payer was online); #4 16/14; #5 14/13. Total 70 of 73.
- **Release 1 on prod, 2026-10-06 07:27 UTC (owner go, `event-checkin-d6`):**
  `main` `6eb39e9e` (tree = `0010f115`, staging parity passed), version
  `a518d80a`. Backup `bethere-db-20261006-1422-pre-release-0010f115.sql`
  (2,497,242 B, 600). Smoke reads + writes pass with the token; films 206 with
  the owner's sha256. Pull 155 merged into develop (`2dc39d84`). Smoke leaves
  an orphan fixture deposit: `.issues/187`.
