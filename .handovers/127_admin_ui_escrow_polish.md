# Handover 127 — Admin UI Escrow Polish (Init Escrow panel + searchable event selector + scroll/dropdown polish)

> Continuation thread for the **BeThere mainnet launch prep — admin UI bug fixes**.
> Authored after the prior session left 5 uncommitted files; this session committed them,
> diagnosed a mobile layout regression, and built a searchable event selector to replace the
> native `<select>` that was unreadable with 30+ events.

---

## 1. What happened (this session)

Four phases, all converging on the admin UI. **No Rust program code touched** — only
`frontend-leptos` + `worker` assets. All work local on `develop`, **7 commits ahead of
`origin/develop`**, working tree clean.

### 1.1 Phase 1 — Commit the 5 uncommitted files from prior session
Reviewed and committed the dangling work as two atomic commits:

| Commit | Type | Summary |
|---|---|---|
| `ab3f77e` | `feat` | **Init Escrow panel** added to admin escrow lifecycle (branches on `escrow_status`, reuses shared signing/simulate helpers from existing escrow UI) |
| `bd78ccd` | `fix` | **Four production bug fixes**: (a) per-row refund proof state via `HashMap` keyed by `attendee_id`, (b) admin sidebar "Home" button, (c) dedupe `Online`/`Virtual` badge on online event page, (d) mobile event selector layout |

### 1.2 Phase 2 — Diagnose mobile layout regression
After Phase 1, observed that on mobile the **event selector stayed crammed in a horizontal
scroll bar** instead of breaking onto its own row. Root cause:

- `.admin-sidebar` on mobile is `flex-direction: row` with `overflow-x: auto`.
- That combination forces `flex-wrap: nowrap` by default.
- The prior fix had set `flex-basis: 100%` on the sidebar sections expecting them to wrap —
  but **`flex-basis: 100%` does nothing without `flex-wrap: wrap` on the parent**.

Fix: `d2ff6f5` — single-line CSS addition: `flex-wrap: wrap` on `.admin-sidebar` mobile media
query. The `flex-basis: 100%` rules already on the children then take effect and force the
break onto separate rows.

### 1.3 Phase 3 — Searchable, sortable event selector (the major work)
The native `<select>` for choosing the active admin event was unreadable with 30+ events and
gave no way to filter or see dates. Replaced it with a custom combobox.

**New file: `frontend-leptos/src/pages/admin_event_selector.rs`**

Features:
- **Text filter** — filters by event name or slug, case-insensitive; Enter selects top match
- **Smart sort priority** — Active+upcoming (soonest first) → Active+past (most recent) →
  Draft (soonest) → Completed (most recent); **Archived hidden entirely**
- **Relative date hints** — "in 3d", "2d ago", "today", "Mar 15" (relative if <14d, absolute otherwise)
- **Expandable panel** — `width: max-content; min-width: 100%` so long event names don't
  truncate; capped at 440px or viewport limit
- **Close behaviors** — outside click (fixed backdrop), Escape key, **scroll** (capture-phase
  window listener — excludes internal list scroll via `closest(".admin-evt-panel")`)
- **`title` attributes** — hover reveals full event name on trigger and on list items
- **Keyboard** — Enter selects top match; Escape closes

Required `#[derive(PartialEq)]` on `EventMeta` because `Memo<T>` constrains `T: PartialEq`
and the sorted/filtered list is wrapped in a `Memo`.

Five commits, iterated through four bugs:

| Commit | Type | Summary |
|---|---|---|
| `037d5c3` | `feat` | Initial searchable, sortable event selector dropdown + `PartialEq` on `EventMeta` |
| `1b275f8` | `fix` | **Color-theme bug** — replaced guessed non-existent CSS variables (`--surface`, `--text`) and hardcoded fallbacks (`#fff`/`#888`) with real project dark-theme vars (`--bg-secondary`, `--text-primary`, etc.) |
| `dc70312` | `feat` | **Expandable panel** (`width: max-content; min-width: 100%`) + `title` tooltips — fixed truncation of long event names |
| `d693a5f` | `fix` | **Scroll-detachment bug** — close dropdown on scroll (capture-phase window listener, excludes internal list scroll) so panel doesn't float away from trigger when the page scrolls |

### 1.4 Phase 4 — Verify and deploy each fix to local `:8787` wrangler dev

- `cargo check --target wasm32-unknown-unknown` — **0 errors, 0 warnings**
- `bash build.sh` — **0 errors**; verified WASM (`eee4629c85f97871`, 4.5 MB), JS (74 KB),
  and CSS (`7fd3fddf6c208c04`, 310 KB) all present in `dist/` and matching `index.html` hashes
  **before** killing the worker (avoided the partial-build blank-page trap)
- Worker restarted on `:8787`, all asset bundles returning HTTP 200
- Clippy: 197 warnings (186 pre-existing baseline + 11 from new code following existing
  `clone_on_copy` patterns — **not fixed** to match codebase convention)

---

## 2. Where is the plan / code / test

| Artifact | Location | State |
|---|---|---|
| Init Escrow panel | `frontend-leptos/src/pages/admin/...` (escrow lifecycle view) | ✅ committed `ab3f77e` |
| Per-row refund proof | `frontend-leptos/src/pages/admin/...` (Refund Queue) — `HashMap<attendee_id, RefundProofState>` | ✅ committed `bd78ccd` |
| Admin sidebar Home button | `frontend-leptos/src/pages/admin.rs` | ✅ committed `bd78ccd` |
| Online badge dedupe | online event page view | ✅ committed `bd78ccd` |
| Mobile `flex-wrap` fix | `frontend-leptos/src/styles/...` — `.admin-sidebar` mobile media query | ✅ committed `d2ff6f5` |
| **Event selector component** | **`frontend-leptos/src/pages/admin_event_selector.rs`** (new file) | ✅ committed `037d5c3` + 4 follow-up fixes |
| Worker dist bundles | `worker/dist/{index.html, *.js, *.wasm, *.css}` | ✅ built locally, served on `:8787` |

**No new tests added** — UI component, manual verification only. Existing test suite not
re-run (no Rust program code touched, no worker handler changes).

---

## 3. Reflection — struggling / solved

**Solved:**
1. **Stale worker / blank page trap.** A worker started 38 minutes before a `build.sh` will
   serve stale assets referencing purged WASM filenames. Fixed by always restarting the worker
   after a successful build, and by verifying `.js` + `.wasm` + `.css` all exist and match
   `index.html` hashes *before* killing the worker.
2. **Partial-build race.** A transient trunk race can emit CSS but skip WASM/JS, leaving
   `index.html` referencing non-existent bundles → blank page. Fixed by always checking all
   three artifact types after `build.sh`.
3. **White-on-white color bug.** First iteration of the event selector used guessed CSS
   variables (`--surface`, `--text`) that don't exist in this project, plus hardcoded
   `#fff`/`#888` fallbacks — unreadable on the dark theme. Solved by reading the actual CSS
   variable definitions first (`--bg-secondary`, `--text-primary`, `--text-muted`,
   `--border-hover`, `--accent`, `--radius-sm`, etc.) before writing component CSS.
4. **`flex-basis: 100%` no-op.** Discovered that `flex-basis: 100%` on children has no effect
   unless the parent has `flex-wrap: wrap`. Single-line fix.
5. **`Memo<T>` constraint.** `EventMeta` needed `#[derive(PartialEq)]` because the
   filtered/sorted list is wrapped in `Memo<Vec<EventMeta>>`.
6. **Scroll-detachment of popover.** An absolutely-positioned dropdown inside a scrollable
   container detaches from its trigger when the container scrolls (the standard Radix/Headless
   UI pattern is to **close on scroll**). Implemented via capture-phase window listener
   (`addEventListener("scroll", cb, true)`), with internal list scroll excluded via
   `closest(".admin-evt-panel")`.
7. **`Closure` is `!Send`.** Raw `wasm_bindgen::Closure` cannot go in `on_cleanup` (requires
   `Send + Sync + 'static`). Used `.forget()` for one-time listeners, matching the existing
   `admin.rs` keydown pattern.

**Struggled with / open nuance:**
- **`scroll` events don't bubble.** Capture phase (`useCapture=true`) is the only way to
  catch nested scrolls from `window`. Initially tried bubble phase and missed inner scrolls.
- **web-sys method-name proliferation.** `Window` (via `EventTarget`) has
  `add_event_listener_with_callback_and_bool(cb, useCapture)` and the matching
  `remove_event_listener_with_callback_and_bool` — the simpler legacy API. The
  `_and_add_event_listener_options` / `_and_event_listener_options` variants exist for add
  but not consistently for remove. Stick to the `_and_bool` pair.
- **11 new clippy warnings** (clone_on_copy on `ReadSignal`/`WriteSignal`) **not fixed** to
  match existing codebase convention. Could clean up if user wants consistency.

---

## 4. Remaining work

### ① User verification on `:8787` (blocking push) — USER ACTION
Hard-refresh (Cmd+Shift+R) and verify:

- **Event selector**:
  - Colors match the dark theme (no white panels / light text)
  - Dropdown opens, filters by text, sorts upcoming-active-first
  - Panel expands wider than trigger for long event names
  - Scrolling the page closes the dropdown; scrolling the list itself does NOT
  - Mobile: Home link + event selector each on their own full-width row
- **4 production fixes** (visible on `:8787` since the worker is in `--remote` mode against
  production KV/D1/R2):
  - Refund Queue: type in row A → rows B/C/D must NOT update
  - Admin sidebar: "Home" button appears and works
  - Online event page: only ONE `Online` indicator
  - Mobile view: event selector on its own full-width row

### ② Push to `origin/develop` — USER ACTION (after ①)
```sh
git push origin develop
```

### ③ Still-open triage items — NEED USER INPUT
| Item | What's needed |
|---|---|
| **Issue #1** THB submit silent fail | `wrangler tail` output or affected attendee's browser console |
| **Issue #2** Deposit link wrong event | The two URLs (admin-produced vs manually-typed) to confirm `event_id` mismatch |
| **Blank `/ticket/...` redirect** | DevTools Console + Network + Service Workers tabs |
| **Issue #7** Button-flow audit | Scope confirmation before starting |

### ④ Autonomous work available — NO USER INPUT NEEDED
- **NONE remaining on the mainnet code path.** Per handover #126 §1.1: "mainnet is an ops
  problem now, not a code problem." The canary-deploy mitigation runbook
  (`docs/mainnet_canary_mitigation_runbook.md`) **already exists and is reconciled to PR #21**
  (commits `39c32bd` + `d37d9dc`) — do NOT re-draft it.
- Post-merge cleanup of `feature/event_recap` is blocked on PRs #19 + #20 merging first
  (user/team decision).

---

## 5. Issues ref / plan status

- **Issue 001** (deposit commitment & refund): escrow layer implemented & devnet-validated;
  admin UI side polished this session (Init Escrow panel + per-row refund proof state).
- **Plan 004 §3.3** (3 verified-build items): still BLOCKED on Docker absent on host — see
  handover #126 §1.2. Not touched this session.
- **Plans 005/008/016**: still have unchecked items (manual/ops ACs). Not touched this session.
- **No new `.issues/` files created** — all work this session was in-scope bug fixes plus a
  requested UX improvement (searchable selector), not new plan items.

---

## 6. How to dev / test

### Restart the worker after a build (avoid stale-asset trap)
```sh
# 1. Build
bash build.sh

# 2. VERIFY all three artifact types exist and match index.html hashes BEFORE killing worker
eza -la worker/dist/  # expect matching .js, .wasm, .css

# 3. Kill the old worker and restart
procs wrangler | awk 'NR>1 {print $1}' | xargs kill 2>/dev/null
sleep 2
cd worker && nohup bash deploy.sh dev > /tmp/wrangler-dev.log 2>&1 &

# 4. Hard-refresh browser (Cmd+Shift+R)
```

### ⚠️ `deploy.sh dev` connects to PRODUCTION data
`bash deploy.sh dev` (without `--local`) reads AND writes to the remote production KV/D1/R2.
For a fully isolated sandbox:
```sh
bash deploy.sh dev --local
```

### Use the correct project CSS variables
This is a dark-themed app. **Background `#13131b`, text `#e0e0e0`.**

Real CSS variables (use these):
- `--bg-primary`, `--bg-secondary`, `--bg-card`, `--bg-hover`
- `--text-primary`, `--text-secondary`, `--text-muted`
- `--border`, `--border-hover`
- `--accent`, `--accent-bg`
- `--success-bright`, `--info-bright`
- `--radius-sm`

**Do NOT use**: `--surface`, `--text` (don't exist), or hardcoded fallbacks like `#fff`/`#888`.
Always grep the existing CSS variable definitions before writing new component CSS.

### Cargo check (fast wasm target check)
```sh
cd frontend-leptos && cargo check --target wasm32-unknown-unknown
```

---

## 7. Branch state at handoff

```
develop @ d693a5f (HEAD, 7 ahead of origin/develop)   ← current working context, clean
├── ab3f77e feat(admin): Init Escrow panel
├── bd78ccd fix(admin): per-row refund proof + home btn + online badge dedupe + mobile selector
├── d2ff6f5 fix(admin): flex-wrap:wrap on mobile sidebar
├── 037d5c3 feat(admin): searchable event selector + PartialEq EventMeta
├── 1b275f8 fix(admin): dark-theme CSS vars in event selector
├── dc70312 feat(admin): expandable dropdown panel + tooltips
└── d693a5f fix(admin): close dropdown on scroll

origin/develop @ 44cfb5c   ← last pushed commit (Audit Arena submission package)

PRs still open (unchanged this session):
  #19 chore/plan005-staging-scaffold → develop   (CLEAN, green CI)
  #20 feat/plan008-event-lifecycle    → develop   (CLEAN, green CI)
  feature/event_recap                              (PR #16 CLOSED, deletable post-merge of #19+#20)
```

Worker: `:8787` running in `--remote` mode (production KV/D1/R2) at handoff time. Kill and
restart with `--local` for isolated testing.

---

**TL;DR for the next thread:** 7 commits sit local on `develop` awaiting user browser
verification on `:8787` (in `--remote` mode), then `git push`. After that, the open triage
items (Issues #1, #2, blank `/ticket/` redirect, #7) all need user-supplied diagnostics.
Mainnet code work is DONE — do not invent more. Canary runbook already shipped and reconciled.