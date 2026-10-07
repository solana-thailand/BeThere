# 044 · Agent message of 7 Oct 2026: plan

Status: plan, waiting for the owner's "go". Written 2026-10-08 by session
`event-checkin-90`. Source: the owner's message "Message to the event-checkin
agent, 7 Oct 2026 (final)". Items in the owner's order; item 1 is the top
priority. Each step gets ticked here with its proof (commit, command output,
link) as it lands. If reality differs, this file changes first.

Standing rules for every item:
- Prod backup before any prod deploy or prod write, into `~/bethere-backups/`
  (600), reported by name, size and sha256. Never moved to the Trash.
- No merge to `main` without the owner. Pull requests are written "pull N" in
  commits.
- Facts checked while writing this plan (8 Oct, read-only): the 10 new
  card/film files all match the listed sha256; RTM #6 is `status = active`, no
  escrow; prod has 1 campaign with 1 linked event (the message says none); 13
  events have a recording and 2 have a summary row; the `send_email` binding
  is still commented out (no sender domain onboarded).

**Needed from the owner before some steps** (asked once, here):
- A fresh `SMOKE_TOKEN`: the 6 Oct token was valid ~24 h, so it has expired.
  Items 2 and 5 need it (prod smoke with writes; marking RTM #6 completed).
- Item 3: the two smoke events also have 2 `deposit_statuses` rows (counted 6
  Oct). The message says "nothing else". Delete those 2 as well, or leave them?
- Item 4: the card loops (`bethere-card-og-*.mp4`): ship as `og:video`, or keep
  the PNG as the only share image? Default: PNG only.

---

## 1. One fresh devnet deposit, end to end (by 10 Oct 18:00 Bangkok)

Steps:
- [ ] 1.1 Fresh attendee keypair (devnet). Fund it: devnet SOL from the
  organizer wallet; 1 devnet USDC sent from the harness wallet (the fresh
  wallet itself has never been used). Record balances with the commands.
- [ ] 1.2 Run `scripts/e2e/test_escrow_devnet.sh` steps 1–10 against staging
  (DEV_MODE, the path that ran green on 23 and 28 Sep), fresh `EVENT_ID`
  (`devnet-run-20261008-…`), `event_end` = creation + 10 min,
  `refund_deadline` > `event_end`: `create_event` → `deposit` →
  `mark_checked_in` (before `event_end`) → wait past `event_end` → `refund`
  (it closes the deposit PDA in the same tx).
- [ ] 1.3 For each step, `solana confirm -v <sig> --url devnet` and the
  Explorer link (`?cluster=devnet`) opened in a browser: status Success, slot,
  block time. Attendee and vault USDC balances before deposit, after deposit,
  after refund (`spl-token balance` / `solana account`), command beside each.
- [ ] 1.4 `docs/devnet-run-2026-10-<dd>.md` on branch `feature/devnet-run-<dd>`
  (no merge to main): step, signer, signature, link, slot, time; `event_end`
  and `refund_deadline` in unix and Bangkok time.
- [ ] 1.5 Screen recording of the live run (terminal + Explorer) with
  `screencapture -v`, scaled to 1080p with ffmpeg →
  `~/Movies/bethere-devnet-e2e-<date>.mov`. The script's output goes through a
  filter that removes home-directory and keypair paths; nothing else on screen.

Proof: every signature `Success` in `solana confirm` and in Explorer; balances
return to the start (minus fees). Owner's go: none (devnet only).
Risks: devnet RPC or faucet outage; Explorer slowness (the 27 Sep problem:
wait and reload, never replay); macOS may deny screen recording to the
terminal (then I say so at once and give a one-command runner the owner can
record). Estimate: half a day. If a step fails I report the program error and
the step, and by 10 Oct 18:00 at the latest whether it landed.

## 2. Pull 158 to prod (`.issues/187`)

Steps:
- [ ] 2.1 Merge pull 158 into `develop` (merge commit, "pull 158").
- [ ] 2.2 Stage `develop`'s head (staging parity compares trees: the branch has
  a docs commit after the staged `346686f7`). Staging smoke reads + writes.
- [ ] 2.3 Prod backup; release merge `develop` → `main`, tree = staged tree;
  `worker/deploy.sh`; smoke with the new token; `wrangler deployments list`.
- [ ] 2.4 Write volume on `thb_deposits`; issue 187 → "deployed".

Proof: parity line "HEAD … has the same tree as staging's …"; smoke "Writes
work"; a prod orphan count unchanged by the smoke (the fix's own check).
Owner's go: given by the message. Risk: low (staged and proven on 6 Oct).
Estimate: 2 h.

## 3. Delete the leftover smoke-test rows in prod

Steps:
- [ ] 3.1 Read-only count `WHERE event_id IN ('smoke-1791271668',
  'smoke-1791274365')` in `attendees`, `thb_deposits`, `thb_deposit_archive`
  (and `deposit_statuses`, to report). Expected 2 / 2 (฿1,000) / 0. If any
  differs: stop and report.
- [ ] 3.2 Prod backup (name, size, sha256).
- [ ] 3.3 One D1 batch: `DELETE … WHERE event_id IN (those two)` on
  `thb_deposits` and `attendees` (plus `deposit_statuses` only if the owner
  says so). Count again: 0 / 0 / 0. Post both counts.
- [ ] 3.4 Note in `.issues/187`: the 4 Jun in-person attendee of the deleted
  `solana-x-ai-builders-2` stays (real registration; why a hand query reads
  142 on-site and `/api/public/stats` 141).

Proof: before/after counts. Owner's go: given (exact counts). Risk: none
beyond the listed rows (the WHERE is the two ids only). Estimate: 30 min.
Runs after item 2, so the deployed fix is what later smoke runs use.

## 4. Release 3 with the new card and films

Steps:
- [ ] 4.1 Rebase pull 157 on `develop` (after item 2).
- [ ] 4.2 Ladder: add the RTM #6 row (21 paid, 20 came, source "BeThere
  system"), re-derived read-only with the ladder's definition before it goes
  in; headline "RTM #1–#6: 90 of 94"; tests pin 94/90 and came ≤ paid.
- [ ] 4.3 Share card: `share/og-image.png` ← `bethere-card-og-en.png`
  (sha256 checked); `og:title` "BeThere — Show up. Get it all back.";
  `og:description` "90 of 94 who paid a deposit came (RTM #1–#6, staff not
  counted). Free events, a refundable deposit, all back after the event." in
  `index.html` meta, OG, Twitter and JSON-LD; the share-copy test updated.
- [ ] 4.4 Films: `why-{en,th}.mp4` and `.vtt` (sha256 checked), posters re-cut
  at 1.5 s, `FILM_REV` and the `/media` cache key → `v=5` (pinned together by a
  test). `.srt` files are not served by the site (captions are WebVTT).
- [ ] 4.5 Live counter fine print, EN: "90 of 94 including RTM #1 (paid by
  hand) and RTM #3 (rows recovered from a backup)". TH draft for the owner:
  "รวม RTM #1 (จ่ายด้วยมือ) และ RTM #3 (กู้แถวจากไฟล์สำรอง) เป็น 90 จาก 94".
- [ ] 4.6 CI green incl. e2e (landing baselines regenerated from CI, checked by
  eye); size budget reported.
- [ ] 4.7 Stage; phone + desktop screenshots EN/TH (ladder, film at ~50 s,
  reel, globe, share card via the OG tags); wait for the owner's look.
- [ ] 4.8 On the owner's go: prod backup, release merge, deploy, smoke; upload
  the 8 photos to the prod bucket with `scripts/landing_photos_upload.py`
  (read back, 16/16 hashes).

Proof: hashes, tests, screenshots, smoke. Owner's go: prod deploy (4.8).
Risk: first load is +35.6 KB over the R2 baseline (past the warn line, under
fail); the new assets are not first load. Estimate: 1 day to staging.

## 5. Past events: two fixes

Steps:
- [ ] 5.1 Find the app's path that sets `status = completed` (the admin event
  update) and prove it on staging with a fixture event: KV and D1 both say
  completed, `/e/<slug>` shows the ended view.
- [ ] 5.2 Prod: same call for RTM #6 with the new token (no D1 write); check
  `/e/solana-x-ai-builders-the-road-to-mainnet-6-bangkok` shows the ended view
  and `/api/public/events/past` behaviour.
- [ ] 5.3 Report recaps ready to publish (recording present AND summary row
  present; read-only today: 13 have a recording, 2 have a summary). No flags
  flipped.

Proof: before/after `GET /api/events/{id}` status and the page. Owner's go:
given for 5.2 (normal app path). Risk: completing may trigger hooks (summary
freeze, post-event registration rules): read them in 5.1 first. Estimate: 2 h.

## 6. Release 4 and Super GOAT: plan only, nothing built before 13 Oct

Basis: `bethere-ux/site/` (index, events, course, organizers, sponsors, try,
record; shared `site.js`/`site.css`, ~2,000 lines) and
`reports/phase-2/SUPER-GOAT-SPEC.md`. Estimates are working days for one agent,
including tests, staging and the owner's look.

**Cross-cutting first (R4.0, 1.5 d).** Route map: `/` story, `/events` (today
`/discover` becomes a redirect), `/events/<series>`, `/organizers`,
`/sponsors`, `/try` (hidden in prod until SG3). One `doors` component and one
`try` band component. Every page sits in the attendee shell; the staff shell
is at 95% of its ceiling, so nothing new goes there.

**Pages**
| line | what | est. |
|---|---|---|
| R4.1 | `/` lit room: 320×180 grey image dithered on canvas, pointer light avoiding text, a kept light per watched episode (localStorage), lazy JS like the globe | 1.5 d |
| R4.2 | `/` payers' hall by event (from R4.7), so far, came back, globe, three doors | 1 d |
| R4.3 | `/events`: open events and courses; empty state computed from the catalogue (times run, median gap, last date), subscribe as the main action, last episode, devnet / Discord / host links | 1.5 d |
| R4.4 | `/events/<series>` course page on `campaigns` / `campaign_events.sequence_order` (`.issues/068`): register once, episode list, progress in worker storage (new table, migration) | 3 d |
| R4.5 | `/organizers`: the existing swimlane (`how.rs`) reused as is + try line | 0.5 d |
| R4.6 | `/sponsors`: the existing sponsor section as a page | 0.5 d |

**Worker**
| line | what | est. |
|---|---|---|
| R4.7 | Facts: `GET /api/public/stats` + the hand-recorded facts as one typed constant set in `domain` (the ladder table moves there); per-event payers for the hall | 1 d |
| R4.8 | Per-event OG image: the poster resized at upload (CPU cap: resize in the browser before upload, not in the worker) and `og:*` per `/e/` page | 1 d |
| R4.9 | Static no-WASM home: worker-rendered HTML for `/` with the same numbers (first paint, crawlers), the SPA hydrates over it | 2 d |
| R4.10 | Cookie-free aggregate click counters (per page × door, daily rows in D1, no ids) | 0.5 d |
| R4.11 | Meals not ordered per event: registered − came against the organizer's food count (new event field), aggregate only | 1 d |
| R4.12 | Subscribe sender: waitlist moves from the Sheet tab to D1 (dedupe, consent wording, one-click unsubscribe token); one email per newly opened public event via Cloudflare Email Sending. **Owner dependency:** a sender domain on Cloudflare (`send_email` is commented out). Until it ships, prod does not show subscribe as a promise | 2.5 d |
| R4.13 | Signed-in empty state with credit: `/api/deposit/credit-balance` shown with "covers your next deposit" | 0.5 d |

**Super GOAT** (after the above, in the spec's order)
| line | what | est. |
|---|---|---|
| SG1 | `domain::record::door_history` (one definition, the 20-regulars rule), `GET /api/me/record`, `/me/record`, `events.regular_rule` + register form + ticket + admin editor and count | 4 d |
| SG4 | "I showed up" card: `/api/me/showed/:slug.png` (pre-rendered per event, owner's call on cost), share button, revocable `/showed/:token` | 2 d |
| SG3 | `/try` devnet sandbox: daily sandbox event by cron, rate-limited devnet USDC faucet with Turnstile, worker-signed door for sandbox only, Explorer links, live tally; separate keypair, program-level event check | 4–5 d (after item 1) |
| SG2 | x402 design doc only: the `payTo` custody catch first, options (a) deposit-tx payload / (b) vault PDA / (c) non-refundable parts, duplicate-settlement cache, 10 USDC cap; recommendation; no build | 1 d |

Total R4 ≈ 17.5 d; Super GOAT ≈ 11–12 d. Order after 13 Oct: R4.0 → R4.7 →
R4.2/R4.1 → R4.3 (+R4.12 if the sender domain exists) → R4.5/R4.6 → R4.8 →
R4.9 → R4.4 → R4.10/R4.11/R4.13 → SG1 → SG4 → SG3 → SG2.
Risks: the sender domain (R4.12); the free-plan 10 ms CPU cap (R4.8 resize
stays in the browser; R4.9 HTML must stay small); the course page needs a
migration and the `.issues/068` rule that retrospective registration is never
attendance.
