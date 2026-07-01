# Plan 016 — Campaign Create UX

> Branch: `feature/campaign_create_ux` (off `develop` @ `e5cc18d`)
> Predecessor: `.plans/015_campaigns_ux_completion.md` (campaigns claim button)
> Motivation: organizer feedback — "I feel it hard to create campaign."
> Goal: make campaign creation usable by a non-technical organizer in under a minute, with zero knowledge of slugs, org IDs, Arweave/IPFS, or on-chain collection mints.

---

## 0. Problem Analysis (grounded in code)

Source: `frontend-leptos/src/pages/campaigns_page.rs` — `Create Campaign` view (L741–907).

The `reward_config` is **not** a raw JSON textarea (good) — it's already structured, and the NFT fields are correctly gated behind `reward_type == "nft_certificate"`. The real friction is:

| # | Friction | Evidence | Impact |
|---|----------|----------|--------|
| 1 | **`Organization ID` is a raw text input** — organizer must know/paste the org id | `campaigns_page.rs:795-805` placeholder `"Organization ID"` | 🔴 Blocker for non-engineers |
| 2 | **`Campaign ID (slug)` is manual** — must invent a unique kebab-case slug | `campaigns_page.rs:763-778` | 🔴 Friction + collision risk |
| 3 | **NFT fields look mandatory but have silent backend defaults** | `claim_campaign_reward` (`worker/src/handlers/campaigns.rs:626-672`) defaults `name` → `"{title} - Campaign Complete"`, `symbol` → `"CAMPAIGN"`, `description` → `"Completed the {title} campaign"` | 🟠 Intimidating (6 mandatory-looking fields) |
| 4 | **Image URL / Metadata URI / Collection Mint are advanced** (Arweave/IPFS hosting + on-chain collection) yet shown at top level | `campaigns_page.rs:863-885` | 🟠 scares non-Solana organizers |
| 5 | **Events are a separate post-create step** — "create" yields an empty draft; must then find the Events tab | flow gap | 🟡 confusing |

### Backend facts (already verified)
- `GET /orgs` **exists** (`worker/src/handlers/mod.rs:377`) → `OrgListResponse { orgs: Vec<OrganizationConfig> }`.
- ⚠️ `list_orgs` is **SuperAdmin-only** (`worker/src/handlers/orgs.rs:35-57`: non-super-admin → `403 Forbidden`). This blocks the dropdown for plain organizers — see P0.3 open decision.
- `create_campaign` hardcodes `status='draft'` (`worker/src/db/campaigns.rs:100-105`) — new campaigns always start as draft (no status field on create is correct).
- Slug format expected: kebab-case (e.g. `solana-hacker-series-2025`). No existing slugify helper on the frontend.

---

## 1. Scope

| Tier | Item | Surface | Risk |
|------|------|---------|------|
| 🔴 P0.1 | Auto-generate slug from Title (editable) | frontend only | Low |
| 🔴 P0.2 | Mark reward fields optional + show default hints | frontend only | Low |
| 🔴 P0.3 | `Organization ID` → dropdown (reuse event-picker pattern) | frontend + **backend access decision** | Medium |
| 🟡 P1.1 | Collapse Image URL / Metadata URI / Collection Mint into "Advanced (optional)" disclosure + explain Collection Mint | frontend only | Low |
| 🟡 P1.2 | Required-field markers (`*`) + inline validation (title, org) | frontend only | Low |
| 🟡 P1.3 | Post-create nudge → "Created as draft — add events" button to Events tab | frontend only | Low |
| 🟢 P2.1 | Inline slug uniqueness check | frontend + new endpoint | Medium |
| 🟢 P2.2 | NFT/campaign preview card | frontend only | Medium |
| 🟢 P2.3 | Status choice on create (active vs draft) | frontend + backend | Low (defer; draft-default likely intentional) |

---

## 2. P0 — Detail & Acceptance

### P0.1 — Auto-generate slug from Title
**Behavior**
- While editing Title in the **create** view (not edit view), if the slug has not been manually touched, auto-fill `form_id` from a slugify(Title).
- Slugify rules: lowercase; replace any run of non-`[a-z0-9]` with `-`; collapse repeated `-`; trim leading/trailing `-`; cap length 60; if empty result, leave blank.
- A `slug_manually_edited: Signal<bool>` guards against clobbering a user-typed slug.
- Slug field remains editable (user can override); the auto-fill is a convenience.
- Disabled on edit view (slug is immutable post-create — already enforced by `disabled=...editing_id...`).

**Acceptance**
- [ ] Typing `"Solana Hacker Series 2025"` in Title fills slug with `solana-hacker-series-2025`.
- [ ] Manually editing the slug stops auto-fill for the rest of the session.
- [ ] Empty/whitespace title leaves slug empty (no `---` garbage).
- [ ] Edit view never overwrites the existing slug.
- [ ] `cargo check --target wasm32-unknown-unknown` clean.

### P0.2 — Reward fields optional + default hints
**Behavior**
- Inside the `NFT Reward Configuration` section, add a one-line hint under each of `NFT Name`, `Symbol`, `Description` stating the backend default that applies if left blank:
  - NFT Name → `"Leave blank to use '{Title} - Campaign Complete'"`
  - Symbol → `"Leave blank to use 'CAMPAIGN'"`
  - Description → `"Leave blank to use 'Completed the {Title} campaign'"` (live-substitute current Title)
- Add a section header note: *"All fields below are optional — defaults are applied on mint."*
- No behavior change to save/claim (defaults already exist in `claim_campaign_reward`).

**Acceptance**
- [ ] Hints render under the three fields and update the `{Title}` substitution live.
- [ ] Saving with all three blank still mints with correct defaults (covered by existing claim path — no regression).
- [ ] wasm32 check clean.

### P0.3 — Organization ID → dropdown  ⚠️ has open access-control decision
**Behavior (frontend)**
- Replace the raw `<input>` with a `<select>` populated from a new `api::list_orgs()` call (mirror the event-picker mount-Effect pattern from P1 of plan 015).
- Local `orgs_list: Signal<Vec<OrgOption>>` + Effect on mount calling `api::list_orgs()`.
- `<option value="">` "— Select organization —" default; options sorted by name; `value` = org id, label = org name.
- On edit view, pre-select the campaign's existing `organization_id`.

**Open decision (must resolve before this serves non-super-admins):**
The existing `GET /orgs` is **SuperAdmin-only**. Plain organizers (who can create campaigns) will get `403`. Options:
- **(A) Widen read access** — allow any authenticated admin/organizer to `GET /orgs` (read-only list); keep create/update/delete SuperAdmin-only. *Recommended.* Small change in `handlers/orgs.rs::list_orgs` (drop/relax the role check, the route is already behind the admin-authed router). Security: listing org names/ids is low-sensitivity; mutating stays locked.
- **(B) Auto-resolve from session** — if Claims carry the user's org, pre-fill and hide the field. Needs investigation of whether `Claims`/admin profile has an org binding (not yet verified).
- **(C) Graceful fallback** — try `list_orgs`; on 403 fall back to the raw text input. Avoids backend change but keeps the bad UX for non-super-admins.

**Acceptance**
- [ ] As a SuperAdmin, the org field is a dropdown populated with all orgs; selecting one sets `organization_id` on save.
- [ ] Edit view pre-selects the existing org.
- [ ] (Per chosen option A/B/C) a plain organizer can either pick their org (A), has it auto-filled (B), or falls back cleanly (C) — **decide and document before implement**.
- [ ] wasm32 check clean; `cargo check -p worker` clean if backend touched.

---

## 3. P1 — Detail & Acceptance

### P1.1 — Advanced (optional) disclosure
- Move `Image URL`, `Metadata URI`, `Collection Mint` into a `<details><summary>"Advanced (optional)"</summary>` block inside the NFT section.
- Add a one-line explainer under Collection Mint: *"Optional. If set, NFTs in this collection score 3× on the leaderboard; otherwise 1×. Leave blank if unsure."*
- Collapsed by default.

### P1.2 — Required-field markers + inline validation
- Append ` *` (red) to labels for `Campaign ID (slug)`, `Title`, `Organization` (when shown).
- On Save: in addition to existing Title + slug checks, validate an org is selected/present (warning toast if blank) — matches existing toast pattern.

### P1.3 — Post-create nudge
- After a successful create, instead of (or in addition to) the success toast, navigate to the new campaign's **Detail → Events tab** with a visible banner: *"Campaign created as draft. Add events to activate."*
- Reuse existing navigation signals; no new state.

**Acceptance (P1)**: each item visually verified; wasm32 check clean.

---

## 4. P2 — Detail (defer until P0/P1 land)
- **P2.1 Slug uniqueness**: new `GET /campaigns/{id}/exists` (or reuse `get_campaign`) checked on slug blur; show inline "already taken" if collision.
- **P2.2 Preview**: render a small NFT preview card (name/symbol/image) live from the form fields.
- **P2.3 Status on create**: optional active/draft selector; only if draft-default proves problematic. Backend `create_campaign` currently hardcodes `'draft'`.

---

## 5. Out of Scope
- Changing `reward_config` schema.
- Multi-event linking inside the create flow (keep Events tab as the dedicated surface; P1.3 just nudges there).
- Removing the `Organization` concept or auto-deriving org from the event (events do carry `organization_id`, but cross-derivation is out of scope here).

---

## 6. Verification
- `cargo check -p worker --quiet` (if backend touched)
- `cargo clippy -p worker -- -D warnings` (CI gate)
- `cargo check --target wasm32-unknown-unknown --quiet`
- `cd frontend-leptos && bash build.sh`
- Manual click-through (admin OAuth): create a campaign end-to-end in <1 min without referencing any raw id.

---

## 7. Rollout
- Commit each tier separately on `feature/campaign_create_ux` (conventional messages: `feat(campaigns): ...`).
- PR/merge `develop` → `main` after manual verification.
- Frontend-only P0.1/P0.2/P1 deploy via `build.sh` + `deploy.sh`. P0.3 (if option A) requires worker redeploy.

---

## 8. Status
- [ ] P0.1 auto-slug
- [ ] P0.2 optional hints
- [ ] P0.3 org dropdown (blocked on §2 access decision)
- [ ] P1.1 advanced disclosure
- [ ] P1.2 required markers
- [ ] P1.3 post-create nudge
- [ ] P2 deferred

**Decision needed from user before P0.3:** choose option **A / B / C** for the org-dropdown access constraint (§2).
````
