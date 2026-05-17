# 063 — Design System: Brief Drift Fix + CI Lint + Visual Audit

## What Happened

Executed P0 (brief drift fix + CI lint) and P1 (full visual audit) for the BeThere design system.

### P0 — Brief Drift Fix
- `.design/brief.md` documented **pure grays** (`#0f0f0f`, `#1a1a1a`, `#242424`, `#1e1e1e`, `#2a2a2a`) for 5 background tokens
- Actual CSS `:root` uses **blue-tinted darks** (`#13131b`, `#1a1a24`, `#242430`, `#1e1e2a`, `#2a2a38`)
- Fixed all 5 tokens in brief.md + 4 Stitch prompt references (`#0f0f0f` → `#13131b`)
- Added missing tokens table (WCAG-safe button colors, heading, void, solana)
- Fixed typography: font now correctly says "Inter" (not system stack), weight 700 (not 800)

### P0 — CI Lint Step
- Created `.github/workflows/design-lint.yml`
- Runs on PRs touching DESIGN.md, brief.md, or style.css
- Step 1: `npx @google/design.md lint .design/DESIGN.md` — fails on errors only
- Step 2: Bash drift checker — extracts bg tokens from both files, fails on mismatch

### P1 — Visual Audit (full report from sub-agent)

## Where Is the Plan/Code/Test

- **Code**: `.design/brief.md` (drift fixed), `.github/workflows/design-lint.yml` (new)
- **Audit**: Inline in this handover

## Reflection / Struggling / Solved

- **Struggle**: `edit_file` couldn't match multi-line strings in Stitch prompt code blocks — solved with `sed -i`
- **Struggle**: DESIGN.md spec schema doesn't support `borderColor`, `gradient`, `rgba()` — 20 orphaned token warnings are expected and can't be fixed without upstream spec changes

## Visual Audit Findings

### Summary Stats
| Metric | Count |
|--------|-------|
| Hardcoded color instances (outside `:root`) | ~97 |
| Unique non-token hex values | ~28 |
| `border-radius` not matching tokens (6/10/9999px) | ~38 |
| Missing `:hover` states | 3 |
| Missing `:focus-visible` states | ~8 |

### Missing `:root` Tokens (should be added)
| Token | Value | Used In |
|-------|-------|---------|
| `--accent-purple` | `#a78bfa` | ~12 occurrences (key-badge, tile-key, puzzle-opt, brand gradients) |
| `--accent-light` | `#a5b4fc` | Button hover gradients |
| `--warning-bright` | `#eab308` | VIP badge, ticket status pending |
| `--warning-dark` | `#d97706` | Adventure claim button |
| `--code-bg` | `#0d1117` | Puzzle code blocks (~6 uses) |
| `--code-text` | `#c9d1d9` | Puzzle code text (~6 uses) |
| `--color-x-brand` | `#1da1f2` | Share X components |

### Top Hardcoded Color Replacements Needed
- `#fff` → `var(--text-heading)` (~6 occurrences)
- `#666` → `var(--text-muted)` (~10 occurrences)
- `#888` → `var(--text-muted)` (~8 occurrences)
- `#6366f1` → `var(--accent)` (~15 occurrences)
- `#a78bfa` → `var(--accent-purple)` once tokenized (~8 occurrences)
- `#3b82f6` → `var(--info)` (~5 occurrences)
- `#22c55e` → `var(--success)` (~5 occurrences)

### Non-Design-System Colors (need decision)
- `#ff6b6b` in `.error-msg` — use `var(--danger-bright)` (#f87171)
- `#ff6b35` in `.claim-adventure-gate h2` — use `var(--warning)` or new token
- `#e2e8f0` in `.claim-wallet-divider` — light-theme gray, use `var(--border)`
- `#14f195` in `.form-section-badge-recommended` — Solana green, new token
- `#9ca3af` in badges — Tailwind gray-400, use `var(--text-muted)`
- `#444` in `.filter-pill:hover` — use `var(--border-hover)`
- Tailwind slate palette `#1e293b`, `#94a3b8`, `#334155` in `.adventure-banner-info`

### Spacing Issues
- All badges/pills using `20px` or `16px` should be `9999px` per DESIGN.md
- All cards/panels using `12px` should be `var(--radius)` (10px)
- All elements using `8px` radius should be `var(--radius-sm)` (6px)

## Remain Work

1. ~~**Add missing `:root` tokens**~~ ✅ DONE (8 tokens added)
2. ~~**Replace hardcoded colors** with var() references~~ ✅ DONE (~97 instances replaced)
3. ~~**Fix border-radius** inconsistencies~~ ✅ DONE (~38 instances fixed)
4. ~~**Add missing hover/focus-visible** states~~ ✅ DONE (hover on .claim-share-x, 7 focus-visible selectors)
5. ~~**Update DESIGN.md** to include the new tokens~~ ✅ DONE
6. ~~Decide on non-system colors~~ ✅ DONE (all mapped to tokens)

### Remaining (P2 — future)
- Build script: `DESIGN.md → tokens.css → @import in style.css` to prevent future drift
- Component visual reference page (static HTML showing every component)
- Expand DESIGN.md components from 20 to cover scanner, adventure, deposit, quiz editor
- Tokenize remaining `rgba()` values used outside `:root` (~50 instances of translucent accent/success/warning/danger tints)

## Issues Ref

- Follows commit `98943b2` (DESIGN.md + WCAG fixes)
- Related to `.design/DESIGN.md` spec alignment

## How to Dev/Test

```bash
# Verify brief drift is fixed
grep '#0f0f0f' .design/brief.md  # should return nothing

# Verify CSS tokens match brief
grep 'bg-primary' frontend-leptos/style.css | head -1  # should be #13131b

# Run CI lint locally
npx @google/design.md lint .design/DESIGN.md

# Run drift checker locally (from project root)
bash -c '
  css_val=$(grep -oP "(?<=--bg-primary:\s*)#[0-9a-fA-F]+" frontend-leptos/style.css)
  brief_val=$(grep -oP "(?<=--bg-primary.*\|\s\`)#[0-9a-fA-F]+" .design/brief.md | head -1)
  echo "CSS=$css_val Brief=$brief_val"
  [ "$css_val" = "$brief_val" ] && echo "MATCH ✓" || echo "DRIFT ✗"
'
```
