# Browser-Test Checklist — Plan 015 (Campaigns UX Completion)

> Companion to `.handovers/124_campaigns_ux_completion.md`.
> Verifies every P0 / P1 / P2 change shipped on `feature/campaigns_claim_button` (merge commit `1eaf4bc`, now on `develop`).
> Production-grade: every step uses real services (Helius RPC + DAS, Solana Explorer, D1). No mocks, no stubs.

---

## 0. Scope

| Tier | Change | Test Cases |
|------|--------|-----------|
| 🔴 P0 | Campaign reward **Claim** button + cNFT mint | TC-01 → TC-07 |
| 🟡 P1.1 | Event picker **dropdown** (replace raw ID input) | TC-08 → TC-10 |
| 🟡 P1.2 | `completion_criteria` descriptive-only label | TC-11 |
| 🟢 P2.1 | **Draft-status** warning banner | TC-12 → TC-14 |
| 🟢 P2.2 | `badge` reward option removed (frontend) | TC-15 → TC-16 |
| 🟢 P2.3 | Per-event **attendance chips** in Progress tab | TC-17 → TC-19 |

Pass criteria: **every** TC marked Pass. Any Fail blocks `develop` → `main` merge and redeploy.

---

## 1. Prerequisites

### 1.1 Environment
- [ ] Worker deployed and reachable (note base URL: `_________________________________`).
- [ ] Frontend deployed and reachable (note URL: `_________________________________`).
- [ ] Confirm target cluster from worker config (`config.solana.rpc_url`): ☐ devnet ☐ mainnet-beta.
  - Strongly recommended: run the **full suite on devnet first**; perform a single mainnet smoke (TC-03 + TC-04) as the final gate.
- [ ] Helius API key valid for the target cluster (worker config `config.solana.api_key`).
- [ ] Solana Explorer reachable at `https://explorer.solana.com/?cluster=<cluster>`.

### 1.2 Accounts / sessions
- [ ] **Admin OAuth session** (Google login with admin role) — needed for campaign setup, Events tab, Progress tab.
- [ ] **Developer account** (separate Google login) — the test "attendee/claimer". Record developer email: `_________________________________`.
- [ ] **Two browser profiles** (or one normal + one incognito) so admin and developer sessions coexist without cookie collision.

### 1.3 Wallet
- [ ] Browser wallet extension installed (backpack / solflare / phantom).
- [ ] A wallet connected in the **developer** browser profile; record its address: `_________________________________`.
- [ ] **Fee model note (correction to original prereq):** the P0 claim mints via Helius `mintCompressedNft` with `"owner": <wallet_address>` and **Helius' managed Merkle tree** (see `worker/src/solana.rs` L121-125). The on-chain mint is therefore **Helius-sponsored** — the recipient wallet is the **owner**, not the payer, and does **not** need SOL for the claim mint itself.
  - ⇒ Replace the "connected wallet with SOL for fees" prereq with: **valid, connected wallet (recipient/owner)**. SOL is only needed if a *different* flow requires a wallet signature; the claim flow does not.

### 1.4 Test campaign
- [ ] A real campaign exists with:
  - `status = active`
  - `reward_type = nft_certificate`
  - `reward_config.collection_mint = <valid collection mint on target cluster>`: `_________________________________`
  - `reward_config` contains `name`, `symbol`, `description`, `image_url`, `metadata_uri` (used by the mint — record them so you can verify on-chain):
    - name: `_________________________________`
    - symbol: `_________________________________`
    - image_url: `_________________________________`
    - metadata_uri: `_________________________________`
  - At least **1 required event** linked (`campaign_events.required = 1`).
- [ ] The developer account is **checked in** to all required events for that campaign (`developer_campaign_progress.is_complete = 1`, `reward_claimed_at IS NULL`).

> If no such campaign/progress exists, create it via the admin UI first (or reuse the campaign from the §"Test Data Setup" runbook at the end).

---

## 2. P0 — Claim Button

### TC-01 — Claim Reward button is visible for a completed, unclaimed campaign
**Preconditions:** §1 fully satisfied; campaign completed by the developer; reward not yet claimed.

**Steps**
1. In the **developer** browser profile, navigate to the Dev Dashboard.
2. Connect the wallet (extension prompt → approve). Confirm the wallet address label matches §1.3.
3. Locate the campaign in the "My Campaign Progress" list.

**Expected**
- [ ] A **"Claim Reward"** button renders on the row (replaces the old static "Reward available to claim!" text).
- [ ] Button is **enabled** (not greyed out, not "Claiming…").

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-02 — Click Claim triggers mint (Helius cNFT)
**Steps**
1. Click **Claim Reward** on TC-01's row.
2. Watch the button + toast.

**Expected**
- [ ] Button label switches to **"Claiming…"** and becomes disabled.
- [ ] **All other** Claim buttons in the list also disable while the request is in flight (per-list `claiming_id` signal).
- [ ] No page navigation; no full reload.

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-03 — cNFT actually mints (on-chain verification via Helius DAS)
**Steps**
1. Wait for the success toast. Record returned values from the response (check DevTools → Network → the `claim` response body, or the toast which shows the truncated `asset_id`):
   - `asset_id`: `_________________________________`
   - `signature`: `_________________________________`
2. Verify the asset via Helius DAS `getAsset` (replace `<RPC>` = `config.solana.rpc_url`, `<KEY>` = `config.solana.api_key`):
   ```bash
   curl -sX POST "<RPC>/?api-key=<KEY>" -H "Content-Type: application/json" -d '{
     "jsonrpc":"2.0","id":1,"method":"getAsset","params":{"id":"<asset_id>"}
   }' | jq
   ```
3. Verify the transaction on Solana Explorer:
   - URL: `https://explorer.solana.com/tx/<signature>?cluster=<cluster>`

**Expected**
- [ ] `getAsset` returns the asset with:
  - `ownership.owner` == the developer wallet (§1.3).
  - `compression.compressed == true`.
  - `content.metadata.name` == §1.4 `name`; `content.metadata.symbol` == §1.4 `symbol`.
  - `content.files[0].uri` / `content.json_uri` matches §1.4 `metadata_uri`/`image_url`.
  - `grouping[0].collection_value` (or `collection.id`) == §1.4 `collection_mint`.
- [ ] Explorer shows the transaction as **Finalized / Success**, program = Bubblegum (or the Helius delegated mint path), no error logs.

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-04 — Row flips to "Reward claimed" + success toast
**Steps**
1. After TC-03 succeeds, observe the row + toast without reloading.

**Expected**
- [ ] **Claim Reward button disappears** on this row.
- [ ] Row now displays **"Reward claimed"** (and the persisted timestamp / truncated asset_id per UI).
- [ ] Success toast appears with the (truncated) `asset_id`.
- [ ] The progress list refreshed its data automatically (no manual reload needed) — progress reflects `reward_claimed_at` set.

**Verify (D1):** run against the worker's D1 (or admin UI) — `developer_campaign_progress.reward_claimed_at IS NOT NULL`, `reward_asset_id == <asset_id>`, `reward_signature == <signature>`.

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-05 (edge) — Already-claimed campaign shows NO Claim button
**Steps**
1. Reload the Dev Dashboard on the developer profile.
2. Look at the row from TC-03/04.

**Expected**
- [ ] No "Claim Reward" button rendered (gated by `reward_claimed_at.is_none()`).
- [ ] Row shows "Reward claimed".
- [ ] Clicking anywhere on the row does **not** trigger any mint request (check Network tab: zero POST to claim endpoint).

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-06 (edge) — Incomplete campaign shows NO Claim button
**Steps**
1. As admin, create/locate a second campaign with **2 required events**.
2. As developer, check in to only **1** of them (so `is_complete = 0`).
3. Open the Dev Dashboard.

**Expected**
- [ ] For this row, **no Claim Reward button** is shown (gated by `is_complete`).
- [ ] Progress reads e.g. "1 / 2" and no claim affordance.

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-07 (edge) — Helius failure surfaces a readable toast (502)
**Goal:** exercise the `AppError::External { service: "helius", status: 502 }` path and confirm the UI shows a clear, non-crashing message.

**Setup (pick one):**
- **(a) Bad collection_mint** — as admin, create a new campaign whose `reward_config.collection_mint` is a valid-base58 but **nonexistent** mint on the target cluster. Complete it for the developer (check in to all required events).
- **(b) Bad API key** — temporarily deploy a worker with an invalid `config.solana.api_key` (devnet only). Roll back after the test.

**Steps**
1. On the Dev Dashboard, click **Claim Reward** on the sabotaged campaign.
2. Observe button state, toast, and the underlying response.

**Expected**
- [ ] Button returns to "Claim Reward" (no longer "Claiming…") after failure.
- [ ] Response status = **502**; body references the Helius external service.
- [ ] Toast shows a **readable** error (e.g. "Helius mint failed: …" / "Reward mint failed, please try again") — no raw stack trace, no white screen.
- [ ] Row state unchanged (still claimable); `reward_claimed_at` still NULL.

**Rollback:** restore the valid `collection_mint` / API key before proceeding.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 3. P1.1 — Event Picker Dropdown

### TC-08 — Dropdown shows event names, not raw IDs
**Steps**
1. As **admin**, open the campaign → **Events** tab.
2. Inspect the "Add event" control.

**Expected**
- [ ] Control is a **`<select>` dropdown**, not a text input.
- [ ] Each `<option>` shows the **event name** (not a bare numeric id).
- [ ] Unnamed events fall back to their `id` (acceptable), but named events show the name.
- [ ] Options are sorted by name (ascending).

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-09 — Already-linked events are excluded
**Steps**
1. Still on Events tab, note which events are already linked (the linked-events list).
2. Open the dropdown.

**Expected**
- [ ] **None** of the already-linked events appear in the dropdown (reactive filter against `campaign_detail.events`).
- [ ] Available (unlinked) events still appear.

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-10 — Selecting an event links it
**Steps**
1. Pick an unlinked event from the dropdown.
2. Submit.

**Expected**
- [ ] Event is linked to the campaign (appears in the linked-events list).
- [ ] Dropdown **reactively removes** it from the available list.
- [ ] If this was the last unlinked event, the dropdown is empty / disabled.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 4. P1.2 — `completion_criteria` Descriptive-Only

### TC-11 — Label/placeholder clarify it is descriptive only
**Steps**
1. As admin, open the campaign **create** (or edit) form.
2. Find the `completion_criteria` field.

**Expected**
- [ ] Label clearly indicates the field is **descriptive/informational** (e.g. "Completion Criteria (descriptive — not enforced)").
- [ ] Placeholder/Helper text explains the actual rule is "attend all required events".
- [ ] No UI language implying the text is parsed/evaluated as a rule.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 5. P2.1 — Draft-Status Warning Banner

### TC-12 — Banner appears on a draft campaign with linked events
**Steps**
1. As admin, set a campaign to `status = draft` and ensure it has ≥1 linked event.
2. Open the campaign **Detail** view.

**Expected**
- [ ] A **warning banner** renders at the top of the Detail view.
- [ ] Banner text explains check-ins won't count toward scoring/progress until the campaign is activated.
- [ ] Banner contains an **"Activate now"** button.

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-13 — "Activate now" flips status and dismisses the banner
**Steps**
1. On the banner from TC-12, click **Activate now**.

**Expected**
- [ ] Campaign `status` flips to **active** (verify in DB / campaign header).
- [ ] Banner **disappears** (reactive on `status == "draft"`).
- [ ] No page reload required.

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-14 — Banner does NOT show in irrelevant states
**Steps**
1. As admin, inspect: (a) a `draft` campaign with **no** linked events; (b) an `active` campaign (with or without events).

**Expected**
- [ ] (a) Draft + zero events → **no banner**.
- [ ] (b) Active → **no banner** in all cases.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 6. P2.2 — Badge Reward Option Removed (frontend) + Backend Backward-Compat

### TC-15 — Reward Type `<select>` has no Badge option
**Steps**
1. As admin, open the campaign **create** form.
2. Open the **Reward Type** dropdown.

**Expected**
- [ ] **No "Badge" option** is present.
- [ ] `nft_certificate` (and any other valid types) still present and selectable.

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-16 (regression) — Existing `badge` campaign is still editable
**Rationale:** per `.handovers/124` §3, the backend `validate_reward_type` still accepts `"badge"` so legacy campaigns remain editable. This test guards that decision.

**Steps**
1. As admin, open an **existing** campaign whose `reward_type = badge` (create one directly in D1 if none exists).
2. Open the Edit form; change an unrelated field (e.g. description); Save.

**Expected**
- [ ] Form loads without error (the stored `badge` value does not break the form even though the `<select>` lacks the option).
- [ ] Save succeeds (no validation rejection); backend `validate_reward_type("badge")` still accepts.
- [ ] No 400/422 returned.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 7. P2.3 — Per-Event Attendance Chips

> Requires the **worker redeploy** (new `list_campaign_attendance` query, `.handovers/124` §4). If the worker is not yet redeployed, the Events column will be empty for all rows (because `#[serde(default)]` yields `[]`) — that itself is a signal the redeploy is missing.

### TC-17 — Progress tab shows an Events column with chips
**Steps**
1. Ensure worker has been redeployed with the P2.3 backend change.
2. As **admin**, open a campaign with ≥2 linked events where the developer checked in to a subset → **Progress** tab.
3. Find the developer's row.

**Expected**
- [ ] A new **Events** column exists in the progress table.
- [ ] One **chip per linked event** renders for that developer.
- [ ] Each chip shows the event name (or falls back to event id if the event has no name).

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-18 — Chip state matches check-in status
**Steps**
1. On TC-17's row, compare each chip to the developer's actual check-ins.

**Expected**
- [ ] Event the developer **attended** → chip with **green check** + event name.
- [ ] Event the developer **did not attend** → chip with **muted circle** + event name.
- [ ] State matches the `attendees.checked_in_at` rows in D1 for that (developer, event).

**Result:** ☐ Pass ☐ Fail — Notes:

---

### TC-19 — Required events are visually emphasized
**Steps**
1. On TC-17's row, identify which events are `required` (vs optional).

**Expected**
- [ ] **Required** events are visually emphasized (bold / distinct color / icon) vs optional events.
- [ ] Emphasis is consistent across rows.

**Result:** ☐ Pass ☐ Fail — Notes:

---

## 8. Test Data Setup Runbook (only if §1.4 campaign is missing)

Run in order, on the target cluster (devnet recommended):

1. **Admin → New Campaign**
   - title: `Plan 015 BT`
   - status: `active`
   - reward_type: `nft_certificate`
   - reward_config:
     ```json
     {
       "name": "Plan 015 BT Reward",
       "symbol": "P15BT",
       "description": "Browser-test reward for plan 015",
       "image_url": "https://<your-host>/badge_production.svg",
       "metadata_uri": "https://<your-host>/metadata/p15bt.json",
       "collection_mint": "<valid collection mint on target cluster>"
     }
     ```
2. **Admin → Events tab** → link 2 events (both `required`).
3. **Admin → create / reuse 2 events** in the system (each with a future date so check-in is allowed).
4. **Developer profile** → check in to event #1 only (via scanner / QR) → row 1 is now `is_complete = 0` (1/2). Use this for **TC-06**.
5. Create a **third** campaign with **1** required event; check the developer in → `is_complete = 1`, `reward_claimed_at NULL`. Use for **TC-01 → TC-04**.
6. For **TC-07**, create a fourth campaign with a deliberately invalid `collection_mint` and complete it for the developer.
7. For **TC-16**, insert one legacy `reward_type = 'badge'` row directly in D1:
   ```sql
   -- Inspect, don't blind-run; adapt ids to your env.
   UPDATE campaigns SET reward_type='badge' WHERE id='<some-test-campaign-id>';
   ```

---

## 9. Sign-Off

| Field | Value |
|-------|-------|
| Tester | _________________________________ |
| Date (UTC) | _________________________________ |
| Cluster | ☐ devnet ☐ mainnet-beta |
| Worker bundle / deploy hash | _________________________________ |
| Frontend bundle hash | _________________________________ |
| Helius API key (last 4) | _______ |
| All TCs Pass | ☐ Yes ☐ No (list failures below) |
| Failures / follow-ups | _________________________________ |

**Gate:** all 19 TCs must be Pass before `develop` → `main` merge and worker+frontend redeploy. Any failure → file an issue referencing this checklist and the relevant TC id before proceeding.