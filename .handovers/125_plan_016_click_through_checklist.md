# Click-Through Checklist — Plan 016 (Campaign Create UX)

> Companion to `.plans/016_campaign_create_ux.md` and `.handovers/124_campaigns_ux_completion.md`.
> Verifies every P0 / P1 change shipped on `feature/campaign_create_ux` (commits `4c99532` P0.1/P0.2, `0cd752c` P0.3/P1.1, `895df80` P1.2/P1.3).
> Production-grade: every step exercises the real worker + D1 + deployed frontend on production. No mocks, no stubs, no placeholders.

---

## 0. Scope

| Tier | Change | Test Cases |
|------|--------|-----------|
| 🚀 deploy | Worker + frontend redeploy (P0.3 access widening needs worker) | TC-01 |
| 🔴 P0.1 | Auto-slug from Title (with manual-edit guard) | TC-02, TC-03 |
| 🔴 P0.2 | Optional NFT reward hints showing literal default templates | TC-04 |
| 🔴 P0.3 | Organization **dropdown** from `GET /orgs` (access widened) | TC-05 → TC-08 |
| 🟡 P1.1 | **Advanced (optional)** disclosure + honest Collection Mint hint | TC-09, TC-10 |
| 🟡 P1.2 | Required-field red asterisks + org validation toast | TC-11, TC-12 |
| 🟡 P1.3 | Post-create **nudge** → Detail → Events tab | TC-13, TC-14 |

Pass criteria: **every** TC marked Pass. Any Fail blocks `develop` → `main` merge and a production redeploy of Plan 016.

> **Honesty notes baked into this checklist (verify, don't assume):**
> - **P1.1 Collection Mint:** the original plan claimed "3× leaderboard scoring". That claim was **verified false** — no scoring logic reads `collection_mint` anywhere in `worker/src`; its only real use is classifying campaign vs event NFTs in `dev_dashboard.rs`. The deployed hint must therefore **NOT** mention a multiplier (see TC-10).
> - **P0.3 edit-view pre-selection:** "n/a" — org is immutable post-create (`UpdateCampaignRequest` has no `organization_id`), and the field is wrapped in `<Show when=editing_id.is_none()>`, so it only renders on **create** (see TC-08).
> - **P1.3 "no new state" deviation:** one local `Signal<bool>` (`draft_nudge`) was added for a dismissible one-shot banner. No global/persistent state (see TC-13/TC-14).

---

## 1. Prerequisites

### 1.1 Environment
- [ ] Local working tree on branch `feature/campaign_create_ux` (HEAD should be `766f340` or later).
- [ ] `frontend-leptos/dist/` will be rebuilt by §2 TC-01 (no pre-existing stale bundle required, but note current bundle hash for comparison): `_________________________________`.
- [ ] Production worker reachable at `https://bethere.solana-thailand.workers.dev` (note any proxy/VPN if applicable).
- [ ] Production D1 id (for verification queries): `98d09542-e7d8-4413-ac34-4276a50d126c`.
- [ ] `wrangler` auth valid (`npx wrangler whoami` returns the Cloudflare account that owns `bethere`).

### 1.2 Accounts / sessions
- [ ] **SuperAdmin OAuth session** (Google login with super-admin role) — needed to (a) confirm the dropdown shows all orgs, (b) seed a non-super-admin organizer.
- [ ] **Plain organizer OAuth session** (Google login with a non-super-admin admin/organizer role) — needed to verify the access widening in TC-06. Record organizer email: `_________________________________`.
  - If no plain organizer exists, create one in D1 (or via whatever admin-roles table your deployment uses) before starting; this account must NOT have super-admin.
- [ ] **Two browser profiles** (or one normal + one incognito) so the two sessions don't collide.

### 1.3 Test data
- [ ] At least **2 organizations** exist in D1 (so the dropdown is non-trivial). Record their ids/names:
  - org A id: `_________________________________` name: `_________________________________`
  - org B id: `_________________________________` name: `_________________________________`
- [ ] At least **1 event** exists and is available to link (only needed for the nudge / Events-tab TCs; not strictly required for create-form TCs).
- [ ] Confirm `list_orgs` currently returns data via the API (as SuperAdmin) before deploying:
  ```bash
  # Replace <TOKEN> with a valid super-admin session bearer token from DevTools.
  curl -s "https://bethere.solana-thailand.workers.dev/api/orgs" \
    -H "Authorization: Bearer <TOKEN>" | jq '.data.orgs | length'
  ```
  - [ ] Returns `>= 2`. If `0` or `null`, create orgs in D1 before proceeding.

> **Before-deploy baseline (record for TC-01 regression check):**
> - As the **plain organizer**, hit `GET /api/orgs` with their token: expected **HTTP 403** today (this is the pre-deploy baseline that TC-06 will flip to 200 after the worker redeploy).
>   - Pre-deploy status code: ☐ 403 ☐ other: _________

---

## 2. Deploy (gate for P0.3)

### TC-01 — Deploy worker + frontend (P0.3 access widening goes live)
**Preconditions:** §1.1 / §1.2 satisfied; working tree clean on `feature/campaign_create_ux`.

**Steps**
1. Build the frontend bundle:
   ```bash
   cd frontend-leptos && bash build.sh
   ```
   - Confirm it finishes with `trunk build --release` success and that `frontend-leptos/dist/index.html` + `frontend-leptos/dist/lazy_assets.js` exist.
2. Record the new bundle hash from `dist/index.html` (the `event-checkin-frontend-<hash>.js` filename): `_________________________________`.
3. Deploy worker + assets together (production):
   ```bash
   cd ../worker && bash deploy.sh
   ```
   - ⚠️ `deploy.sh` with **no args** deploys to **production** worker `bethere` against the **production** D1/KV/R2.
   - ⚠️ Do **NOT** use `deploy.sh dev` for verification — it runs a local worker but reads/writes **production** D1/KV (per `deploy.sh` header). Use the deployed production URL instead.
4. Smoke the deployed worker:
   ```bash
   curl -s -o /dev/null -w "%{http_code} %{size_download}b\n" https://bethere.solana-thailand.workers.dev/
   curl -s -o /dev/null -w "%{http_code}\n" https://bethere.solana-thailand.workers.dev/api/health
   ```
5. Confirm the new frontend bundle is the one being served:
   ```bash
   curl -s https://bethere.solana-thailand.workers.dev/ | grep -o 'event-checkin-frontend-[a-z0-9]*\.js'
   ```

**Expected**
- [ ] `build.sh` exits 0; `dist/` contains a fresh bundle.
- [ ] `deploy.sh` exits 0; its log shows `wrangler deploy` (or the PUT-API fallback) succeeding for worker `bethere`.
- [ ] `/` returns HTTP 200; `/api/health` returns HTTP 200.
- [ ] The bundle filename served on production **matches** the one recorded in step 2 (no stale CDN cache).
- [ ] **Access-widening now live:** as the **plain organizer** (§1.2), `GET /api/orgs` returns **HTTP 200** with the org list (this was 403 before deploy per §1.3 baseline).

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 3. P0.1 — Auto-Slug from Title

### TC-02 — Typing Title auto-fills the slug
**Preconditions:** TC-01 Pass; logged in as SuperAdmin (or organizer post-deploy).

**Steps**
1. Admin → Campaigns → **+ Create Campaign**.
2. Leave the slug field untouched. In **Title**, type: `Solana Hacker Series 2025`.
3. Observe the **Campaign ID (slug)** field.

**Expected**
- [ ] Slug auto-fills to exactly `solana-hacker-series-2025` (lowercase, hyphen-separated, no leading/trailing hyphen, no double hyphen).
- [ ] Slug field remains editable (not disabled).

**Result:** ☐ Pass ☐ Fail — Notes:

### TC-03 — Manually editing the slug stops auto-fill for the session
**Steps**
1. From TC-02's state, click into the slug field and change it to `my-custom-slug`.
2. Go back to **Title** and append more text (e.g. change it to `Solana Hacker Series 2025 Bangkok`).

**Expected**
- [ ] The slug **does not** change — it stays `my-custom-slug` (the `slug_manually_edited` guard suppressed the auto-fill).
- [ ] No console errors.

**Edge check**
- [ ] Clear Title entirely → slug stays as-is (does not get wiped to `---` or empty).
- [ ] Reload the page and open Create again → auto-fill works again on a fresh form (the guard is per-session/per-mount, not global).

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 4. P0.2 — Optional NFT Reward Hints

### TC-04 — NFT section shows "all optional" note + literal default-template hints
**Steps**
1. Create Campaign → set **Reward Type** = `NFT Certificate`.
2. Inspect the **NFT Reward Configuration** section.

**Expected**
- [ ] Section header note reads: *"All fields below are optional — sensible defaults are applied on mint."*
- [ ] Under **NFT Name**: hint reads `Leave blank to use '{Title} - Campaign Complete' on mint.` (`{Title}` shown literally as a placeholder, **not** substituted live).
- [ ] Under **Symbol**: hint reads `Leave blank to use 'CAMPAIGN' on mint.`
- [ ] Under **Description**: hint reads `Leave blank to use 'Completed the {Title} campaign' on mint.`
- [ ] Saving with all three blank still succeeds (defaults apply on claim mint — no regression; the claim path already handles defaults).

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 5. P0.3 — Organization Dropdown

### TC-05 — Org field is a `<select>` populated with org names (SuperAdmin)
**Steps**
1. Logged in as **SuperAdmin**, open Create Campaign.
2. Inspect the **Organization** field.

**Expected**
- [ ] It is a `<select>` dropdown (not a text input).
- [ ] First option is a disabled-looking placeholder `— Select organization —` with empty value.
- [ ] Options are the orgs from §1.3, **sorted by name**, showing the **name** as the label (not the raw id).
- [ ] Selecting an org and saving persists it; verify in D1:
  ```sql
  -- Via wrangler or the Cloudflare D1 console. Replace <created-id>.
  SELECT id, organization_id FROM campaigns WHERE id='<created-id>';
  ```
  - [ ] `organization_id` equals the selected org's id.

**Result:** ☐ Pass ☐ Fail — Notes:

### TC-06 — Plain organizer can load the dropdown (access widening, post-redeploy)
**Preconditions:** TC-01 deployed (worker access widening live); §1.2 plain organizer session ready.

**Steps**
1. Switch to the **plain organizer** browser profile, log in, open Create Campaign.
2. Inspect the **Organization** field.
3. Open DevTools → Network. Confirm the `GET /api/orgs` request.

**Expected**
- [ ] The dropdown is populated (not empty) — no `403` returned by `/api/orgs`.
- [ ] Network shows `GET /api/orgs` → **HTTP 200** with the org list in the response body.
- [ ] No error toast on the page; console has no `[campaigns-page] failed to load orgs list` warning.

> If this fails with 403, the worker redeploy in TC-01 did not take effect — re-run `deploy.sh`. Do **not** proceed to merge.

**Result:** ☐ Pass ☐ Fail — Notes:

### TC-07 — Selecting an org sets `organization_id` on save (regression)
**Steps**
1. As organizer, fill Title (slug auto-fills per TC-02), pick an org from the dropdown, set Reward Type = None.
2. Click **Save Campaign**.

**Expected**
- [ ] Success toast `Campaign created`.
- [ ] In D1, the new row has the correct `organization_id` (verify with the SQL from TC-05).

**Result:** ☐ Pass ☐ Fail — Notes:

### TC-08 — Org field is create-only (immutable on edit)
**Preconditions:** A campaign created via TC-05 or TC-07.

**Steps**
1. From the Campaigns list, click into an existing campaign, then **Edit**.
2. Inspect the form for an Organization field.

**Expected**
- [ ] The **Organization** field does **not** appear on the edit form (only on create).
- [ ] Saving the edit does **not** change `organization_id` (regression — backend `UpdateCampaignRequest` has no `organization_id` field; verify in D1 the value is unchanged).

> This is the documented "n/a" for edit-view pre-selection: org is immutable post-create, so no pre-selection is needed or possible.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 6. P1.1 — Advanced (Optional) Disclosure

### TC-09 — Advanced disclosure is collapsed by default
**Steps**
1. Create Campaign → Reward Type = NFT Certificate.
2. Inspect the **NFT Reward Configuration** section.

**Expected**
- [ ] **Image URL**, **Metadata URI**, **Collection Mint** are **not** visible until the disclosure is expanded.
- [ ] A `▶ Advanced (optional)` summary row is present and the `<details>` is **collapsed by default** (no `open` attribute in the DOM).
- [ ] Clicking the summary expands and reveals the three fields; clicking again collapses them.

**Result:** ☐ Pass ☐ Fail — Notes:

### TC-10 — Collection Mint hint is honest (no "3× leaderboard" claim)
**Steps**
1. Expand the Advanced disclosure (TC-09).
2. Read the hint under **Collection Mint**.

**Expected**
- [ ] The hint reads: `Optional. Groups minted NFTs into an on-chain Solana collection and is used to tell campaign rewards apart from event NFTs. Leave blank if unsure.`
- [ ] The hint **does NOT** contain the words `3×`, `multiplier`, `score`, or `leaderboard` — those were a false claim removed during implementation (verify by reading the literal text; grep the rendered DOM in DevTools if unsure).

> Honesty gate: any mention of a scoring multiplier here is an automatic Fail — the multiplier does not exist in the backend.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 7. P1.2 — Required-Field Markers + Org Validation

### TC-11 — Red asterisks on slug / Title / Organization labels
**Steps**
1. Open Create Campaign.

**Expected**
- [ ] `Campaign ID (slug)` label shows a **red** `*` (rendered via `.required-marker`, `color: var(--danger)`).
- [ ] `Title` label shows a red `*`.
- [ ] `Organization` label shows a red `*`.
- [ ] Red color matches the design-system `--danger` (`#ef4444`) — confirm in DevTools computed styles.

**Result:** ☐ Pass ☐ Fail — Notes:

### TC-12 — Org validation toast on blank save
**Steps**
1. Open Create Campaign.
2. Fill **Title** (so title/slug checks pass) but **leave Organization as `— Select organization —`** (empty).
3. Click **Save Campaign**.

**Expected**
- [ ] A **Warning** toast appears: `Organization is required`.
- [ ] **No** network request to `POST /api/campaigns` is sent (open DevTools → Network to confirm — the guard returns before the request).
- [ ] The form stays on the Create view; no campaign row is created.

**Edge checks**
- [ ] Leaving Title blank → still shows `Title is required` (existing behavior, regression).
- [ ] Leaving slug blank on create → still shows `Campaign ID (slug) is required` (existing behavior, regression).
- [ ] With Title + slug + org all filled → save proceeds normally.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 8. P1.3 — Post-Create Nudge

### TC-13 — Pure create navigates to Detail → Events tab with a nudge banner
**Preconditions:** A "pure" create (NOT promoted from an event) — i.e. reached via **+ Create Campaign**, not via an event's "promote to campaign" action.

**Steps**
1. Admin → Campaigns → **+ Create Campaign**.
2. Fill Title (slug auto-fills), select an org, Reward Type = None, click **Save Campaign**.
3. Observe the resulting view.

**Expected**
- [ ] Success toast `Campaign created` appears.
- [ ] The view navigates to the newly created campaign's **Detail** view (not the List view).
- [ ] The **Events** tab is active (not Progress / Stats).
- [ ] A `.campaign-nudge` banner renders at the top of the Events tab reading: **"Campaign created as draft. Add events to activate it."** with a **Dismiss** button.
- [ ] The banner uses the warning palette (`--warning-bg` / `--warning-border` / `--warning-bright`).

> Regression check: the **promote-from-event** flow (event → "promote to campaign") is intentionally **unchanged** — it still navigates to Detail with the source event auto-linked and **does not** show the nudge (the campaign already has an event). Verify one promote flow if convenient.

**Result:** ☐ Pass ☐ Fail — Notes:

### TC-14 — Nudge is dismissible and auto-clears on navigation
**Steps**
1. From TC-13's state (nudge visible on Events tab).
2. Click **Dismiss** on the banner.
3. Open Create Campaign again (or click another campaign in the list, then come back).

**Expected**
- [ ] Clicking **Dismiss** hides the banner immediately (the local `draft_nudge` signal flips to false).
- [ ] Navigating away from the just-created detail (via Back, selecting another campaign, or starting a new Create) clears the nudge — when you return to that campaign's Events tab, the banner **does not** reappear.
- [ ] Switching to the Progress or Stats tab hides the banner (it lives inside the Events-tab `<Show>`); switching back to Events shows it again **only** if you haven't navigated/dismissed yet — and it never survives a full navigation away.

> Honesty note: the banner is backed by a single local `Signal<bool>` (`draft_nudge`) — there is no persistent/global state, so the banner is inherently session-local and one-shot per create. This matches the documented P1.3 deviation.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 9. Test Data Setup Runbook (only if §1.3 data is missing)

Run on production D1 (`98d09542-e7d8-4413-ac34-4276a50d126c`) via `wrangler d1 execute bethere --remote --command "..."` or the Cloudflare dashboard:

1. **Create 2 organizations** (id = kebab-case, name = display):
   ```sql
   -- Inspect the orgs schema first; adapt column names to your deployment.
   -- The two rows below assume the canonical (id, name) shape used by
   -- OrganizationConfig.
   INSERT INTO organizations (id, name) VALUES
     ('plan016-org-a', 'Plan 016 Org A'),
     ('plan016-org-b', 'Plan 016 Org B');
   ```
2. **Create a plain organizer** (NOT super-admin) in whatever roles/admin table your deployment uses. Record the email in §1.2.
3. **(Optional) Create one event** to link in the Events-tab TCs — only needed if you also want to exercise the promote-from-event regression in TC-13.
4. **Pre-deploy baseline** (§1.3): as the plain organizer, hit `GET /api/orgs` and record the `403` — TC-01/TC-06 will flip this to `200`.

> Do not create test `badge` campaigns or invalid `collection_mint` rows — Plan 016 does not touch the badge path or mint validation; those belong to Plan 015's checklist.

---

## 10. Sign-Off

| Field | Value |
|-------|-------|
| Tester | _________________________________ |
| Date (UTC) | _________________________________ |
| Worker URL | `https://bethere.solana-thailand.workers.dev` |
| Worker bundle / deploy hash | _________________________________ |
| Frontend bundle hash (post-build) | _________________________________ |
| Pre-deploy plain-organizer `/orgs` status | ☐ 403 ☐ other: _________ |
| Post-deploy plain-organizer `/orgs` status | ☐ 200 ☐ other: _________ |
| All 14 TCs Pass | ☐ Yes ☐ No (list failures below) |
| Failures / follow-ups | _________________________________ |

**Gate:** all 14 TCs (TC-01 → TC-14) must be Pass before:
1. Merging `develop` → `main` for the Plan 016 batch, **and**
2. Any further production redeploy that bundles Plan 016.

Any failure → file an issue referencing this checklist and the relevant TC id (e.g. "Plan 016 TC-06 regression: organizer 403 after redeploy") before re-attempting.

> **Cross-plan reminder:** Plan 015's 19 TCs (`.handovers/124_browser_test_checklist.md`) remain a separate, still-open gate for the prior claim-button batch. Passing Plan 016 does **not** satisfy Plan 015 — both must pass before the next `develop` → `main` merge.