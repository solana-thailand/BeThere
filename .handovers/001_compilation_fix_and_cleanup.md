# 001 - Compilation Fix & Project Cleanup

**Date**: 2025-04-19
**Status**: ✅ Complete
**Branch**: main

## What Happened

The project had 26 compilation errors and 8 warnings from the initial scaffolding. All errors were rooted in **Rust 2024 edition** stricter type inference rules and several API mismatches with dependency versions.

## Errors Fixed

### 1. Closure Type Annotations in Rust 2024 (14 errors)

`src/handlers/attendee.rs` and `src/handlers/qr.rs` had explicit closure type annotations like `|a: &Attendee|` that conflicted with Rust 2024's match ergonomics changes.

**Root cause**: `.iter()` yields `&Attendee`. `.filter()` passes `&Item` → `&&Attendee`. But `.map()` and `.filter_map()` pass `Item` → `&Attendee`. Explicit annotations caused mismatches.

**Fix**: Removed ALL explicit closure type annotations. Let Rust infer them. Rust 2024 handles this correctly without annotations.

### 2. Axum 0.8 `Request` Type (1 error)

`src/handlers/auth.rs:108` — `Request` missing generic parameter.

**Fix**: Changed `use axum::http::Request` to `use axum::extract::Request` (Axum 0.8 re-exports the concrete `Request<Body>` here).

### 3. `const fn` with `matches!` on `&str` (6 errors)

`src/handlers/auth.rs:161` — `const fn is_public_route()` used `matches!` with `&str` comparisons, which is not stable in const context.

**Fix**: Changed `const fn` to `fn`.

### 4. RSA Signature Conversion (1 error)

`src/sheets/client.rs:55` — `signature.as_ref()` not available on `rsa::pkcs1v15::Signature`.

**Fix**: Used `Box::from(signature)` to convert via the `From<Signature> for Box<[u8]>` impl, then pass `&sig_bytes` to the base64 encoder.

### 5. Missing `base64::Engine` Import (1 error)

`src/qr/generator.rs:19` — `.encode()` is a trait method requiring `use base64::Engine;`.

**Fix**: Added `use base64::Engine;` at the top.

### 6. Router State Type Mismatch (3 errors)

`src/main.rs:37` — `axum::serve()` expects `Router<()>`, but `routes()` returned `Router<AppState>`.

**Fix**: Changed return type of `routes()` from `Router<AppState>` to `Router<()>`. The `.with_state(state)` call already converts `Router<AppState>` → `Router<()>`.

### 7. Unused Import Warnings (8 warnings)

Removed unused re-exports from `models/mod.rs`, `auth/mod.rs`, `sheets/mod.rs`, `models/attendee.rs`, `models/auth.rs`. Used `#[cfg(test)]` gating for test-only re-exports in `config/mod.rs`. Added `#[allow(dead_code)]` to utility functions kept for future use.

### 8. Clippy Warning

Replaced redundant closure `|a| AttendeeResponse::from_attendee(a)` with method reference `AttendeeResponse::from_attendee`.

## Project Cleanup

- Removed duplicate nested `event-checkin/event-checkin/` directory
- Removed stale `worker/` directory (old TypeScript scaffolding)

## Final State

```
cargo clippy  → 0 warnings, 0 errors
cargo test    → 16/16 tests pass
```

### Test Breakdown

| Module | Tests | Description |
|--------|-------|-------------|
| `auth::google` | 3 | OAuth URL generation, staff allowlist |
| `auth::jwt` | 3 | JWT create/verify/wrong-secret |
| `qr::generator` | 6 | QR URL building, base64 encoding, filtering logic |
| `sheets::client` | 4 | Base64 encoding, PEM parsing, serialization |

## Project Structure (Clean)

```
event-checkin/
├── Cargo.toml
├── .env.example
├── .gitignore
├── src/
│   ├── main.rs
│   ├── config/{mod.rs, types.rs}
│   ├── auth/{mod.rs, google.rs, jwt.rs}
│   ├── models/{mod.rs, api.rs, attendee.rs, auth.rs}
│   ├── handlers/{mod.rs, health.rs, auth.rs, attendee.rs, checkin.rs, qr.rs}
│   ├── sheets/{mod.rs, client.rs}
│   └── qr/{mod.rs, generator.rs}
├── frontend/
│   ├── index.html
│   ├── staff.html
│   ├── admin.html
│   ├── css/style.css
│   └── js/{app.js, scanner.js, admin.js}
├── .handovers/
└── .issues/
```

## Key Lessons Learned

1. **Rust 2024 closures**: Don't annotate closure parameter types in iterator chains. Let the compiler infer them.
2. **Axum 0.8 migration**: Use `axum::extract::Request` (not `axum::http::Request`). Router state is consumed by `.with_state()`.
3. **RSA 0.9 signing**: `Signature` → bytes via `Box::from(sig)`, not `.as_ref()`.
4. **`const fn` limitations**: String comparison in `matches!` is not const-stable yet.

## Next Steps

1. Set up Google Cloud credentials (OAuth + Service Account)
2. Copy `.env.example` to `.env` and fill in values
3. Run `cargo run` for smoke test
4. Test full OAuth login flow
5. Test QR code generation + check-in
6. Deploy to VPS/cloud