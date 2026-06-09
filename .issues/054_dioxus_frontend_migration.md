# 054: Dioxus Frontend Migration (Plan)

## Status: Planned

## Summary
Fork the repo and replace Leptos 0.8 frontend with Dioxus framework.

## Motivation
- Developer preference for Dioxus DX
- Similar RSX/component model, different API ergonomics
- Self-contained `frontend-leptos/` directory makes migration clean

## Scope
- Replace `frontend-leptos/` with `frontend-dioxus/` (or rename)
- Migrate: signals → `use_signal`, `view!{}` → `rsx!{}`, `leptos_router` → `dioxus-router`
- Keep: all `web-sys`/`wasm-bindgen` interop (QR, clipboard, media), API fetch layer
- Backend (worker/) untouched — no frontend coupling

## What Stays the Same
- Backend worker, D1/KV data layer, API endpoints
- WASM build output, deployment pipeline

## Migration Map
| Leptos 0.8 | Dioxus |
|---|---|
| `leptos` (CSR) | `dioxus` + `dioxus-web` |
| `leptos_router` | `dioxus-router` |
| `leptos_meta` | built-in or manual `<head>` |
| `Trunk.toml` + `trunk` | `Dioxus.toml` + `dx` CLI |
| `signal()` | `use_signal()` |
| `view!{}` | `rsx!{}` |
| `Effect::new()` | `use_effect()` |
| `Memo::new()` | `use_memo()` |

## Approach
1. Create `frontend-dioxus/` side-by-side first
2. Port pages one at a time (admin is the most complex)
3. Swap over when feature-complete
4. Remove `frontend-leptos/` after verification

## Dependencies
- None — can be done independently

## Related Issues
- None
