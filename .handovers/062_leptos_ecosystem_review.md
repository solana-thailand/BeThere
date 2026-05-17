# Handover 062 — Leptos Ecosystem Review & Code Quality Analysis

## What Happened

Reviewed [awesome-leptos](https://github.com/leptos-rs/awesome-leptos) ecosystem (50+ libraries) and [Loom neural computer paper](https://arxiv.org/abs/2604.08816v1) for applicability to BeThere. Cross-referenced against current `frontend-leptos/Cargo.toml` (Leptos 0.8, CSR, Trunk) and source code.

**Outcome**: Most libraries are not worth adding (current code is solid). But the code review surfaced 5 concrete code quality improvements. Updated 2 docs, created 1 issue.

## What Changed

### Docs Updated (commit `1169d88`)

1. **`docs/ux_roadmap.md`** (+58 lines):
   - Added leptos library refs to P2-1 (SSE), P3-2 (PWA), P3-3 (darkmode), P3-4 (i18n)
   - New section: "Leptos Ecosystem — Recommended Libraries" with Phase 1/2/3 tables + skip list
   - Added cross-reference to `research_technology_review.md` §14
   - Updated date to 2026-06-12

2. **`docs/research_technology_review.md`** (+50 lines):
   - New entry #14: "Awesome Leptos — Ecosystem Library Review"
   - Ranked awesome-leptos #2, Loom #13 in relevance table
   - Added action item: "Add `leptos-use` + `leptos-captcha` to frontend Cargo.toml (Phase 1)"
   - Added references for Awesome Leptos and Loom

### Issue Created

3. **`.issues/021_frontend_code_quality.md`** — 5 code quality improvements identified during review

## Code Quality Findings

| # | Issue | File | Lines Saved | Effort |
|---|-------|------|-------------|--------|
| 1 | DRY `api_post_json` / `api_put_json` | `api.rs` L392-527 | ~30 | 15 min |
| 2 | Timer leak in countdown | `public_event.rs` L393-410 | bug fix | 10 min |
| 3 | Extract `local_storage()` helper | `auth.rs` L14-32 | ~8 | 5 min |
| 4 | Simplify `format_timestamp` JS interop | `utils.rs` L144-177 | ~17 | 10 min |
| 5 | Split `api.rs` into domain modules | `api.rs` (2865 lines) | structural | 30 min |

### Finding Details

**1. DRY violation in api.rs**: `api_post_json` and `api_put_json` are ~70 lines of near-identical code. Only difference is `Request::post` vs `Request::put`. Extract `api_json_with_body(method, path, body)`.

**2. Timer leak in public_event.rs**: `set_interval` inside `Effect::new` fires on every resource refetch without clearing the previous interval. Also never stops when countdown reaches zero — interval fires forever every second. See `.issues/021_frontend_code_quality.md` Step 2 for fix.

**3. Repeated web_sys chains in auth.rs**: `window().and_then(|w| w.local_storage().ok()).flatten()` appears 3 times in 30 lines (get/set/clear token). Extract `fn local_storage() -> Option<web_sys::Storage>`.

**4. Verbose JS interop in utils.rs**: `format_timestamp` uses 25 lines of individual `js_sys::Reflect::set` calls to build a JS options object. Replace with a `js_object(&[(&str, &str)])` helper — cuts to ~8 lines.

**5. God file**: `api.rs` is 2865 lines. Splitting by domain (client, types, event, attendee, deposit, claim, admin) would improve compile times — changing deposit types wouldn't recompile adventure page.

## Loom Paper Assessment

**Not recommended** for BeThere. Loom is a research paper showing transformers can execute C programs via analytically derived weights. ~10ms/step GPU latency. No practical integration path. Listed in `research_technology_review.md` at rank 13 (lowest relevance).

## Library Recommendations (for future, not now)

### Phase 1 (if going public)
- `leptos-captcha` — self-hosted PoW captcha for reservation flow (no reCAPTCHA dependency)
- `leptos-use` — reactive browser API hooks (only if doing Leptos version upgrade)

### Phase 2 (when feature is needed)
- `leptos_i18n` — compile-time type-safe EN/TH translations (when Thai content exists)
- `leptos_sse` — for P2-1 real-time admin dashboard (when SSE endpoint is built)
- `leptos-hotkeys` — for scanner keyboard shortcuts (when adding them)

### Not recommended (skip)
- Thaw/leptix/shadcn — full rewrite required, custom design system works
- leptos-tea — architecture mismatch with existing signal-based approach
- leptos-image — no image optimization pipeline in the project

## What's Not Changed

- No code changes — all findings documented for next thread
- No dependency additions — libraries listed as recommendations only
- No changes to backend/worker code

## How to Dev/Test

```bash
# Verify current build
cargo check -p event-checkin-frontend --target wasm32-unknown-unknown

# After implementing fixes from .issues/021:
cargo check -p event-checkin-frontend --target wasm32-unknown-unknown
cargo clippy --all-targets
cargo test -p event-checkin-frontend

# Manual: countdown timer on /e/{slug} still ticks correctly
```

## Reflection

**Solved**: Evaluated 50+ Leptos libraries against actual codebase needs. Identified that most are not worth the dependency cost — current hand-rolled code is correct and well-documented.

**Struggled**: Initially over-recommended libraries (Phase 1/2/3 plan). On reflection, most solve problems that don't exist yet or add dependency risk for marginal DX improvement. The honest value was in the code quality review the library comparison prompted.

**Key insight**: The useful output of this session wasn't "which libraries to add" but "what the code review revealed about code quality." The 5 improvements are all zero-dependency refactors.

## Remain Work

- [ ] Implement fixes from `.issues/021_frontend_code_quality.md` (Steps 1-5)
- [ ] Manual test countdown timer on public event page
- [ ] Consider `leptos-captcha` if going public without bot protection

## Issues Ref

- `.issues/021_frontend_code_quality.md` — created (5 code quality improvements)
