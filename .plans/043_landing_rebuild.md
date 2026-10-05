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
- [ ] L4–L10, L12
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
