# 046 — Attendee path onto the site design (UX)

Status: in progress. Phase 0 deployed (pull 247, prod `6cda00ee`). Logo A2 + Phase 1 (all six pages) on develop and staging `9d1665ae` (2026-10-13; pulls 251–258); prod waits on the owner's go. Phase 2 not started.

Owner, 2026-10-12, after the hackathon submission: "better UI/UX", after
the critical fixes (191 → 160 → 165). Audit screenshots (390 px, dark):
`/tmp/ec-ac-scratch/ux/01-home … 09-record.png` (scratch, not in git).

## Finding

The site pages (`/`, `/events`, `/events/{course}`, `/organizers`,
`/me/record`) use the plan 043 landing design (`lp-*`, style-23-landing.css,
the white logo with the purple dot). The attendee path still uses the old
dark-blue and orange "Proof of Attendance" shell (`brand-logo` +
`brand-logo-sub`):

| Page | File | Old shell |
|---|---|---|
| `/e/{slug}` + register form | `pages/public_event/` | style-13/14-public-event.css |
| `/deposit` | `pages/deposit/mod.rs:166` | style-16/19-deposit.css |
| ticket | `pages/ticket/` | style-11-ticket.css |
| claim | `pages/claim/page.rs:308` | style-02-claim.css |
| login | `pages/login.rs:153` | style-01-core.css |
| `/sandbox` | `pages/sandbox/` | mixed |

## Phase 0 — quick fixes (pull 247, deployed)

- [x] `/events` "about every 1 weeks" → "about every week" (`Cadence::weekly`, en/th, static home).
- [x] Cadence quoted the online Latent Space series: a 6–6 tie with RTM, and `max_by_key` kept the last. A tie now keeps the first listed.
- [x] `/deposit` for an ended event with nothing on file: the title was "Something went wrong"; it now says "This event has ended" and links to `/events`.
- [x] Logo mismatch: the final A2 mark (chair + person, `bethere-ux/brand/logo`) in the site header, footer, favicon and apple-touch icon (pull 251); the old orange logo left with Phase 1.

## Phase 1 — one page per PR

Order: deposit → event page → ticket → claim → login → sandbox. These have
the most attendee traffic, and the deposit page is the one in the audit.

Each PR:
- swaps `brand-logo` for the site header (`pages/landing/header.rs`, `lp-logo`). Ask the plan-043 session before changing `header.rs` itself;
- maps the old tokens onto the landing palette; no new colours;
- keeps the attendee wasm first load within the size gate (`frontend_size_budget.sh`);
- keeps `.staff.css` selectors out of attendee markup (`staff_css_fence.py`);
- attaches before/after screenshots: 390 px and desktop, light and dark, en and th;
- is verified by opening the page on staging (`docs/web-verification-runbook.md`), not by clippy + curl.

### Done (2026-10-13, on develop + staging)

| Page | Pull | Notes |
|---|---|---|
| `/deposit/{attendee}` | 252 | first page; Linux baselines added |
| `/e/{slug}` + register form | 253 | ~300 lines of animated background CSS removed; `public_event` reuses the landing `AuthState` |
| `/ticket/{attendee}` | 254 | state banner = ink box with a state-coloured top edge; action cards flat |
| `/claim/{token}` | 255 | gradient glass panels flattened; `tests/css_braces.rs` added after a conflict dropped a `}` |
| `/login` | 256 | `.brand-logo` rules removed (nothing renders them now) |
| `/sandbox` | 257, 258 | no doors (one is this page); inline `code` AA |

How it works: every framed attendee page wraps its content in
`SiteFrame` + `<main class="lp-att">`. `style-24-attendee-site.css` maps the
old tokens (`--bg-*`, `--text-*`, `--accent`, radii) onto the landing ones
inside `.lp-att`, then restyles only hard-coded values. `FRAMED_PREFIXES`
(`locale.rs`) turns the old language bar off on these routes. Fills that
carry paper text use `#5a54cb` (AA), not `--lp-field`.

How it was checked: the local fixture worker (e2e seed) plus a contrast
probe over every text node, which also flags text on a gradient. It was
proven to fail on the unfixed CSS; it is in
`~/worktrees/event-checkin/scratch/probes/ev_colors.mjs` (scratch). Also all
14 a11y specs, the CI visual baselines, and staging after the deploy (which
found the sandbox `code` colour).

Not seen yet: the claim page's checked-in, quiz and success states (the
fixture has no check-in); the live sandbox flow past its first screen; Thai
on every page except `/deposit`.

## Phase 2

Fonts, the stamp animation, loading skeletons. These were deferred since
the Night Edition redesign.

## Out of scope

Staff pages (scanner, admin) stay as they are.
