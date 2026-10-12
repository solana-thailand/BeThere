# 046 — Attendee path onto the site design (UX)

Status: Phase 0 deployed (pull 247, prod `6cda00ee`, 2026-10-13: /events and the static home quote Road to Mainnet); Phases 1–2 not started.

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
- [ ] Logo mismatch: moved to Phase 1. It is the shell swap, not a one-liner.

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

## Phase 2

Fonts, the stamp animation, loading skeletons. These were deferred since
the Night Edition redesign.

## Out of scope

Staff pages (scanner, admin) stay as they are.
