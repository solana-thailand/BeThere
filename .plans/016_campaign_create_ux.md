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
- [x] Typing `"Solana Hacker Series 2025"` in Title fills slug with `solana-hacker-series-2025`.
      (Code-trace verified 2026-07-08: `slugify` at `campaigns_page.rs:52-74` lowercases each
      ASCII alphanumeric char and replaces runs of non-`[a-z0-9]` with a single `-`.
      Tracing `"Solana Hacker Series 2025"` char-by-char: `Solana`→`solana`, space→`-`
      (prev_dash=false, out non-empty), `Hacker`→`hacker`, space→`-`, `Series`→`series`,
      space→`-`, `2025`→`2025`. No trailing dash. Result: `solana-hacker-series-2025`. ✓)
- [x] Manually editing the slug stops auto-fill for the rest of the session.
      (Code-trace verified 2026-07-08: slug input `on:input` at `campaigns_page.rs:847-850`
      calls `set_slug_manually_edited.set(true)`. Title input at L860-868 guards with
      `if editing_id.get().is_none() && !slug_manually_edited.get()`. The flag is reset
      only in `reset_form` (L225-239), which fires on new-campaign — so manual edits
      persist for the session as required.)
- [x] Empty/whitespace title leaves slug empty (no `---` garbage).
      (Code-trace verified 2026-07-08: `slugify("")` → loop never pushes, returns `""`.
      `slugify("   ")` → spaces are not alphanumeric; the dash-push branch requires
      `!out.is_empty()`, so no dash is pushed when out is empty. Returns `""`. No
      `---` possible.)
- [x] Edit view never overwrites the existing slug.
      (Code-trace verified 2026-07-08: Title handler guard at L865 requires
      `editing_id.get().is_none()` — auto-fill only runs on create. Slug input at
      L845 is `disabled=move || editing_id.get().is_some()` — field is read-only on
      edit. Both paths blocked on edit view.)
- [x] `cargo check --target wasm32-unknown-unknown` clean (commit `4c99532`).

### P0.2 — Reward fields optional + default hints
**Behavior**
- Inside the `NFT Reward Configuration` section, add a one-line hint under each of `NFT Name`, `Symbol`, `Description` stating the backend default that applies if left blank:
  - NFT Name → `"Leave blank to use '{Title} - Campaign Complete'"`
  - Symbol → `"Leave blank to use 'CAMPAIGN'"`
  - Description → `"Leave blank to use 'Completed the {Title} campaign'"` (live-substitute current Title)
- Add a section header note: *"All fields below are optional — defaults are applied on mint."*
- No behavior change to save/claim (defaults already exist in `claim_campaign_reward`).

**Acceptance**
- [x] Hints render under NFT Name, Symbol, and Description, each showing the literal default template applied on mint (e.g. `'{Title} - Campaign Complete'`, `'CAMPAIGN'`, `'Completed the {Title} campaign'`). `{Title}` is shown as a placeholder, not substituted live.
      (Code-trace verified 2026-07-08: `campaigns_page.rs` L948 (`Leave blank to use '{Title} - Campaign Complete' on mint.`),
      L957 (`Leave blank to use 'CAMPAIGN' on mint.`), L966 (`Leave blank to use
      'Completed the {Title} campaign' on mint.`). `{Title}` is a literal string
      in all three, not a format substitution. Section header note at L940:
      `All fields below are optional — sensible defaults are applied on mint.`)
- [x] Saving with all three blank still mints with correct defaults (covered by existing claim path — no regression).
      (Code-trace verified 2026-07-08: backend defaults still present in
      `worker/src/handlers/campaigns.rs` L637 (`format!("{} - Campaign Complete",
      campaign.title)`), L647 (`.unwrap_or("CAMPAIGN")`), L650
      (`format!("Completed the {} campaign", campaign.title)`). Frontend P0.2 change
      adds hints only — no save/claim behavior change. Defaults applied when fields
      blank, exactly as the hints advertise.)
- [x] wasm32 check clean (commit `4c99532`).

**Honest correction (2026-08-20, while building P2.2):** both acceptance notes
above were verified by *code trace* and both were wrong on a point of fact.

1. **There is no `'CAMPAIGN'` symbol default.** The note cited
   `campaigns.rs:647` as `.unwrap_or("CAMPAIGN")`. No such line exists: the
   string `"CAMPAIGN"` appears nowhere in `worker/src` or `domain/src`, and
   `solana::MintRequest` has no symbol field at all — `symbol` is never sent to
   the mint. The shipped hint "Leave blank to use 'CAMPAIGN' on mint" was
   therefore misleading. It now reads: *"Stored on the campaign for your own
   reference. Not part of the minted metadata."*
2. **Blank fields did not fall back to defaults.** `handle_save` serialises
   untouched fields as `""`, not as absent keys, and the mint path's
   `.get(k).and_then(as_str).unwrap_or(&default)` reads `""` as a deliberate
   value. A campaign saved with a blank NFT name minted an NFT named `""`,
   while the hint promised the title-based default. Fixed in P2.2 by
   `domain::models::campaign::resolve_reward`, which treats blank and
   whitespace-only as unset.

The lesson is not that code-tracing is useless but that it was done against
remembered line numbers rather than a re-read of the file — the same failure
mode as the "3× leaderboard" claim corrected in P1.1.

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

**Decision (resolved):** Option **A** — widened read access on `GET /orgs` to any authenticated admin/organizer (`worker/src/handlers/orgs.rs`: role check dropped from `list_orgs`; create/update/delete + single-org detail remain SuperAdmin-only). Mutating endpoints untouched.

**Acceptance**
- [x] As a SuperAdmin, the org field is a dropdown populated with all orgs; selecting one sets `organization_id` on save.
- [x] (Option A) a plain organizer can pick their org from the same dropdown — read access widened, no `403`.
- [~] Edit view pre-selects the existing org — **n/a**: org is immutable post-create (`UpdateCampaignRequest` has no `organization_id`), and the field is intentionally hidden on edit (`<Show when=editing_id.is_none()>`). The dropdown only renders on create; edit-view behavior is unchanged.
- [x] wasm32 check clean; `cargo check -p worker` clean; `cargo clippy -p worker -- -D warnings` clean.

**Deploy note:** requires a **worker redeploy** for the access widening to take effect for non-super-admins (a frontend-only deploy is not sufficient — plain organizers would still get `403` until the worker is redeployed).

---

## 3. P1 — Detail & Acceptance

### P1.1 — Advanced (optional) disclosure
- Move `Image URL`, `Metadata URI`, `Collection Mint` into a `<details><summary>"Advanced (optional)"</summary>` block inside the NFT section.
- Add a one-line explainer under Collection Mint.
  **Honest correction:** the original "3× leaderboard scoring" claim was **verified false**. A `grep` across `worker/src` for `collection_mint|multiplier|leaderboard|score` found **no** scoring logic — `collection_mint` is never read for any multiplier. Its only real usage is in `frontend-leptos/src/pages/dev_dashboard.rs`, where it classifies an NFT as a campaign reward vs an event NFT (counting, not a multiplier). The implemented hint is therefore accurate rather than speculative: *"Optional. Groups minted NFTs into an on-chain Solana collection and is used to tell campaign rewards apart from event NFTs. Leave blank if unsure."*
- Collapsed by default (`<details>` with no `open` attribute).

### P1.2 — Required-field markers + inline validation
- Append ` *` (red) to labels for `Campaign ID (slug)`, `Title`, `Organization` (when shown).
- On Save: in addition to existing Title + slug checks, validate an org is selected/present (warning toast if blank) — matches existing toast pattern.

**Acceptance**
- [x] Red `*` appended to `Campaign ID (slug)`, `Title`, and `Organization` labels via a reusable `.required-marker` CSS class (`color: var(--danger)`).
- [x] On Save (create path): org validation added — blank org → `ToastType::Warning` "Organization is required", returns without saving. Mirrors the existing title/slug guard pattern.
- [x] wasm32 check clean; no new clippy findings (verified: 10 pre-existing findings unchanged, 0 introduced).
- [x] **Click-through verified 2026-08-20** against the staging deployment
      (`bethere-staging`, version `71091cdb` glue, same commit) in a scripted
      Chromium session. Observed: 3 `.required-marker` asterisks on the create
      form (`Campaign ID (slug)*`, `Title*`, `Organization*`); saving with the
      Organization dropdown untouched produced the toast "Organization is
      required" and stayed on the form without creating anything.

### P1.3 — Post-create nudge
- After a successful create, instead of (or in addition to) the success toast, navigate to the new campaign's **Detail → Events tab** with a visible banner: *"Campaign created as draft. Add events to activate."*

**Honest deviation from "no new state":** the plan called for reusing existing navigation signals with no new state. The implementation adds **one** local `Signal<bool>` (`draft_nudge`) — a one-shot flag set `true` on the pure-create success path and cleared on `handle_view` / `handle_back` / `handle_create_new` (and dismissible via a "Dismiss" button). This is the minimal local state needed for a dismissible, one-shot banner; no global or persistent state was introduced.

**Acceptance**
- [x] On a pure create (non-promote), success navigates to the new campaign's Detail → Events tab (previously: returned to List view). The success toast is unchanged.
- [x] A `.campaign-nudge` banner renders at the top of the Events tab when `draft_nudge` is true: *"Campaign created as draft. Add events to activate it."* with a "Dismiss" button.
- [x] Banner auto-clears on navigation away (handle_view/handle_back/handle_create_new) and is dismissible inline.
- [x] The promote-from-event path is unchanged (it already navigates to Detail and auto-links the source event; the nudge is intentionally skipped there since a campaign promoted from an event already has an event linked).
- [x] wasm32 check clean; no new clippy findings.
- [x] **Click-through verified 2026-08-20** (staging). After a create the app
      landed on Detail → Events with the `.campaign-nudge` banner and a Dismiss
      button; clicking Dismiss removed the banner (`.campaign-nudge` count 1 → 0).
      **This click-through found a real defect** — see "Defects found" in §9.

**Acceptance (P1)**: each item wasm32-check clean (verified); manual click-through still pending for all P1 items.

---

## 4. P2 — Detail

### P2.1 — Inline slug uniqueness  ✅ implemented

**Chosen shape:** a **new** `GET /api/campaigns/{id}/exists` rather than reusing
`get_campaign`. Reuse was rejected on inspection: `get_campaign` runs
`require_org_access` against the campaign's owning org, so a slug held by
*another* org answers `403`, and a free slug answers `404` — the frontend would
have to infer availability from error codes, and a plain organizer could never
distinguish "taken elsewhere" from "denied". Campaign ids are the primary key of
`campaigns` and therefore globally unique, so the probe must be global too.

**Backend** (`worker/src/handlers/campaigns.rs`, `worker/src/db/campaigns.rs`,
`worker/src/handlers/mod.rs`)
- `campaign_id_exists` returns `{ "exists": bool }` and nothing else — no
  campaign data, not even the owning org. It sits behind the authenticated
  admin router, so it discloses no more than the create attempt it replaces.
- Deliberately **not** org-scoped (see above); the reasoning is recorded in the
  handler doc comment so it is not "fixed" later by mistake.
- `db::campaigns::campaign_exists` selects a constant (`SELECT 1 ... LIMIT 1`)
  instead of the whole row — nothing about the campaign is needed.
- New `validate_campaign_id` (`[A-Za-z0-9_-]`, 1..=64 chars) with 6 unit tests
  in an inline `#[cfg(test)]` module, per the convention documented in
  `worker/tests/do_claim_lock.rs`.

**Security fix beyond the plan's letter (deliberate, flagged):**
`db::campaigns::create_campaign` interpolates `id` straight into the `INSERT`
(`VALUES ('{id}', ...)`) — like every other helper in that module it does not
bind parameters. The plan did not call for touching create, but shipping the
probe alone would have left the two disagreeing: `/exists` would reject a slug
shape that `create` still accepted, so the form would tell the organizer one
thing and the server do another. `validate_campaign_id` is therefore enforced on
**both** paths, which also closes a pre-existing SQL-injection vector on an
authenticated admin endpoint.
**Not fixed, and still open:** the same interpolation applies to `title`,
`description`, `completion_criteria`, `reward_type` and `reward_config` in
`create_campaign`/`update_campaign`, and to ids across the rest of
`db/campaigns.rs`. That is a module-wide parameter-binding refactor, out of
scope here — tracked as a follow-up, not silently absorbed.

**Frontend** (`api/campaign.rs`, `pages/campaigns_page.rs`, `style.css`)
- `api::campaign_exists(id)` over the generic `api_get_json<T>`; the id is
  URL-encoded so a hand-typed `a/b` reaches the handler as one segment and
  returns a shape error (400) rather than a router 404.
- `SlugStatus` = `Unchecked | Checking | Available | Taken | Malformed | CheckFailed`.
- Probe fires on **blur of both the slug and the title** field. Title matters
  because it auto-fills the slug (P0.1): checking only the slug field would miss
  the common path where an organizer types a title and saves without ever
  focusing the slug. Blur is already low-frequency, so no debounce was added.
- Any edit that changes the slug resets the status to `Unchecked`, so a stale
  verdict is never shown against a different slug. In-flight answers are
  discarded if the slug moved on while the request was outstanding.
- Save is blocked only on `Taken` and `Malformed` (both are certain failures).
  `CheckFailed` and `Unchecked` deliberately still save — an offline probe must
  not stop an organizer from creating a campaign.

**Acceptance**
- [x] `GET /campaigns/{id}/exists` returns `{"exists": true}` for a live slug and
      `{"exists": false}` for a free one, for any authenticated admin regardless
      of which org owns the campaign.
- [x] A malformed slug returns `400` (not `404`/`500`) with a shape message.
- [x] The create form shows "Available" / "Already taken" / a shape hint under
      the Campaign ID field after blurring either Title or Campaign ID.
- [x] Editing the slug clears the previous verdict rather than showing it stale.
- [x] Save is blocked with a warning toast on a known collision; an unchecked or
      unreachable probe still saves.
- [x] `validate_campaign_id` rejects quotes, semicolons, whitespace, `/` and
      non-ASCII, and accepts `x--y` (a doubled dash is a legal slug, not a SQL
      comment — `--` can only open a comment outside a string literal, and
      quoting is impossible). 202 worker unit tests pass.
- [x] `cargo clippy -p worker -- -D warnings` clean; frontend wasm32 `--lib`
      clippy `-D warnings` clean; `bash build.sh` succeeds.
- [x] **Click-through verified 2026-08-20** (staging). Observed in the form:
      typing "Plan016 Final Probe" auto-filled the slug `plan016-final-probe`;
      blurring a free slug rendered "Available"; entering the seeded, occupied
      slug `p016-taken-slug` rendered "Already taken — pick a different Campaign
      ID."; entering `bad slug!` rendered the shape hint; and pressing Save on a
      known collision was blocked with a warning toast, leaving the form intact.
      Prod (`587dfa2a`): `GET /api/campaigns/{id}/exists` returns 401 where it
      returned 404 before the deploy, confirming the route is live.

**Deploy note:** requires a **worker redeploy** — the endpoint does not exist in
prod, so until the worker ships, the probe returns 404 and the form degrades to
`CheckFailed` ("Could not check availability — you can still save"), which is
non-blocking by design.

### P2.2 — NFT preview card  ✅ implemented

**The problem the plan did not anticipate.** A preview card is a promise:
"this is what will be minted". Building one exposed two ways the existing code
broke that promise — no symbol is ever minted, and blank fields did not resolve
to their advertised defaults (both written up under P0.2 above). Shipping a
card that rendered the *intended* defaults would have made the UI lie more
confidently, so the defaults were fixed first.

**Shared SSOT rather than a mirror** (`domain/src/models/campaign.rs`, new)
- `resolve_reward(title, config) -> ResolvedReward { name, description, image_url }`
  is the single implementation of "what does this reward mint", called by
  **both** `worker::handlers::campaigns::claim_campaign_reward` and the admin
  preview card. A frontend copy of the defaults would have been a mirror with
  no guard; the repo already has an `ssot_mirror_audit` test precisely because
  that pattern drifts.
- `reward_config_field` collapses missing / non-string / blank / whitespace-only
  into `None`, which is the actual bug fix.
- `KEY_NAME`/`KEY_DESCRIPTION`/`KEY_IMAGE_URL` are exported constants used by
  the resolver *and* by the form's `build_reward_config`, so a key rename is a
  compile-time break instead of a silent all-defaults regression.
- The worker's hand-rolled default block is deleted, not duplicated.

**Card** (`campaigns_page.rs::nft_preview_card`, `style.css`)
- Shows only the three fields that reach `MintRequest`. Symbol, Metadata URI
  and Collection Mint are stored but never minted, so previewing them would
  imply otherwise; a hint says so explicitly.
- Values the organizer left blank are rendered with a `default` tag, so a
  default is never mistaken for something they typed.
- With no title yet, both textual defaults would interpolate to
  `" - Campaign Complete"`, which reads as a bug — the card prompts for a title
  instead.
- A dead artwork URL falls back to the placeholder icon underneath rather than
  a broken-image glyph (`on:error` hides the `<img>`).

**Acceptance**
- [x] Card renders live under the NFT section when `reward_type == nft_certificate`.
- [x] Preview values are produced by the same function the mint path calls, so
      they cannot drift; the worker no longer computes defaults itself.
- [x] Blank name/description resolve to the title-based defaults (previously
      minted as `""`) and are tagged `default` in the card.
- [x] Blank/whitespace/non-string/absent all resolve identically.
- [x] Unminted fields (symbol, metadata_uri, collection_mint) never affect the
      resolved metadata and are absent from the card.
- [x] 15 domain tests (`domain/tests/campaign_reward.rs`) + 8 frontend inline
      tests (`campaigns_page.rs`, covering `slugify` and the
      `build_reward_config` → `resolve_reward` contract).
- [x] **Click-through verified 2026-08-20** (staging). Selecting reward type
      "NFT Certificate" rendered `.nft-preview-card` reading
      `Plan016 Live Probe - Campaign Complete` + `DEFAULT` tag and
      `Completed the Plan016 Live Probe campaign` + `DEFAULT`; typing a name
      updated the card live to `Live Probe Badge` and dropped that field's tag.
      The saved campaign's `reward_config` then read `"name":"Live Probe Badge"`,
      matching the card.
      **Not covered by this click-through:** mint-time resolution itself, which
      needs a completed enrolment plus a Crossmint mint. That path is covered
      only by the 15 domain tests.

### P2.3 — Status on create  ✅ implemented

The plan gated this on "only if draft-default proves problematic"; it was
requested directly, so it is implemented. **Finding worth recording:** `draft`
is very nearly cosmetic today. The only status gate anywhere in the worker is
`db::campaigns::campaign_collection_mints` (`WHERE status = 'active'`, used to
classify NFTs). Check-in enrolment (`on_event_checkin`), progress tracking and
reward claiming do **not** consult status — a draft campaign still accrues
progress and still mints rewards. The selector's hint says so rather than
implying a visibility guarantee that does not exist. Making `draft` actually
gate anything is a separate decision, deliberately not taken here.

**Backend**
- `CreateCampaignRequest.status`, defaulted to `"draft"` via serde so a client
  that omits it behaves exactly as before this change.
- `validate_create_status` accepts `draft|active` only — narrower than
  `validate_campaign_status` on purpose: nothing is complete at create time, and
  a campaign born `completed` could never be progressed through.
  `update_campaign_status` still accepts `completed`.
- `db::create_campaign` takes `status` instead of hardcoding `'draft'`; it is
  interpolated like every other column in that module, which is safe only
  because the value is checked against a closed set first (documented on the fn).

**Frontend**
- Draft/Active `<select>` on the create form only (status is changed afterwards
  from the campaign list, which already has activate/complete controls).

**Acceptance**
- [x] Creating with Active yields an `active` campaign; the default is still `draft`.
- [x] A request omitting `status` deserializes to `draft` (pinned by test).
- [x] `completed` is rejected on create but still allowed as a transition.
- [x] 4 worker unit tests covering the above; 206 worker tests pass.
- [x] **Click-through verified 2026-08-20** (staging). The "Initial status"
      select rendered with Draft/Active and defaulted to `draft`; choosing
      Active and saving produced a campaign the API then reported as
      `status=active` (`GET /api/campaigns/plan016-final-probe`).

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
- [x] P0.1 auto-slug (wasm32 check clean; click-through verified 2026-08-20 — see §9)
- [x] P0.2 optional hints (wasm32 check clean; click-through verified 2026-08-20 — see §9)
- [x] P0.3 org dropdown — **option A** (wasm32 + worker check + clippy `-D warnings` clean; click-through verified 2026-08-20; **requires worker redeploy** for non-super-admin access)
- [x] P1.1 advanced disclosure (wasm32 check clean; the false "3× leaderboard" claim was corrected — see §3 P1.1)
- [x] P1.2 required markers (wasm32 check clean; added org validation on create; click-through verified 2026-08-20 — see §9)
- [x] P1.3 post-create nudge (wasm32 check clean; honest deviation — one local `draft_nudge` signal added, see §3 P1.3; click-through verified 2026-08-20 — see §9)
- [x] P2.1 slug uniqueness (new `GET /campaigns/{id}/exists`; worker clippy + 202 unit tests + wasm32 clippy + `build.sh` clean; click-through verified 2026-08-20; **requires worker redeploy**)
      Includes a deliberate, flagged security fix: `validate_campaign_id` is now enforced on `create_campaign` too, closing a pre-existing SQL-injection vector on the campaign id. See §4 P2.1.
- [x] P2.2 preview card (shared `domain::models::campaign` SSOT; 15 domain + 8 frontend tests; click-through verified 2026-08-20 — see §9)
      Fixed two false claims carried by P0.2 — no `'CAMPAIGN'` symbol default exists, and blank fields minted `""` instead of the advertised defaults. See §2 P0.2 correction and §4 P2.2.
- [x] P2.3 status on create (draft/active selector; `draft` default preserved for older clients; 4 worker tests; click-through verified 2026-08-20 — see §9)
      Finding: `draft` currently gates only NFT classification — progress and reward claiming ignore status. See §4 P2.3.

**P0.3 decision (resolved):** Option **A** — widened `GET /orgs` read access to any authenticated admin; mutations stay SuperAdmin-only. Worker redeploy required.

All "manual click-through pending" items are now closed — see §9 for how, and for
what the click-through found that no amount of code-tracing had.

---

## 9. Deployment & click-through verification (2026-08-20)

### Deployed
| Env | Worker | Version | Content-Type check |
|---|---|---|---|
| Staging | `bethere-staging` | `c439a555` → reverified after fixes | `/` text/html, JS text/javascript ✅ |
| Prod | `bethere` | `587dfa2a` | `/` text/html, JS text/javascript, wasm application/wasm, css text/css ✅ |

Prod D1 (`bethere-db`) was exported to `~/bethere-backups/` before deploying, per
the standing pre-deploy rule. The backup lives outside the repo — it contains PII.

**Prod route liveness:** `GET /api/campaigns/{id}/exists` returned **404 before**
the deploy and **401 after**, which is the discriminator that proves the new
route shipped rather than merely that the worker responded. `dev-token` is
rejected on prod (401) — `DEV_MODE=0` there, as it must be.

### How the click-throughs were done
Against **staging**, not prod. Staging runs `DEV_MODE=1`, so a browser carrying
`localStorage.event_checkin_token = "dev-token"` authenticates as the configured
super admin; a scripted Chromium session (Playwright) then drove the real
deployed UI. Staging has its own D1/KV/R2, so the probe campaigns never touched
production data, and all fixtures (one org, four campaigns) were deleted
afterwards — `GET /api/orgs` returns `{"orgs":[]}` again.

**Prod's UI was not click-through tested.** That needs a Google OAuth admin
session in a real browser. Prod verification is limited to the route/content-type
evidence above.

### Defects found — none of which the code-trace passes had caught
1. **P1.3 banner contradicted P2.3** *(introduced by this plan; fixed)*. Creating
   an Active campaign still announced "Campaign created as draft." P1.3's copy
   was written when draft was the only outcome, and P2.3 made status a choice
   without revisiting it. `draft_nudge` went from `Signal<bool>` to
   `Signal<Option<String>>` carrying the created status; Active now reads
   "Campaign created and active. Add events so attendees can make progress."
2. **Reward Type defaulted to `""`, so the default happy path 400'd**
   *(pre-existing; fixed)*. `form_reward_type` initialised to `String::new()`
   while the `<select>`'s first option is `value="none"` — so the control
   *displayed* "None" while the signal stayed empty, and
   `POST /api/campaigns` answered `invalid reward_type:  (expected
   none/nft_certificate/badge)`. A first-time organizer who never touched that
   dropdown could not create a campaign at all. Directly defeats this plan's
   stated goal, and every prior verification pass missed it because the API was
   always exercised with an explicit `reward_type`. Signal now defaults to
   `"none"` in both the initialiser and `reset_form`.
3. **`GET /api/campaigns/{id}/stats` returns 500 for a campaign with no
   enrolments** *(pre-existing; NOT fixed — out of scope, flagged)*. In
   `db::campaigns::campaign_completion_stats`, `SUM(CASE WHEN is_complete = 1
   ...)` is NULL over an empty set, and `TotalsRow.total_completed` is a
   non-optional `i64`, so deserialization fails. Every newly created campaign
   therefore 500s on its Stats tab. The frontend degrades gracefully (logs a
   warning, renders no stats), which is why it went unnoticed. One-line fix:
   `COALESCE(SUM(...), 0)`. Left for a separate change since it predates this
   plan and touches the stats path, not the create UX.
````
