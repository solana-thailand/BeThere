# Handover 081: PDPA Consent + WASM Optimization

## What Happened

Two logical chunks committed to main:

### Commit 1: `feat: PDPA consent phases A/B/C + privacy policy page` (b2cb1a7)
Full-stack PDPA (Thailand Personal Data Protection Act) compliance across 3 phases:
- **Phase A**: Mandatory data consent checkbox (`consent_given`) — always shown, always required
- **Phase B**: Per-event photo/media consent toggle (`require_photo_consent` + `photo_consent_given`) — organizer opts in
- **Phase C**: Public `/privacy` page with 11-section PDPA-compliant notice

Sheet columns grew from 30 (A–AD) to 32 (A–AF). All tests updated.

### Commit 2: `perf: remove flate2/crc32fast from frontend, feature-gate domain QR` (5c25c23)
Frontend WASM size optimization:
- Replaced `flate2` ZlibEncoder with stored DEFLATE blocks (zero compression overhead for tiny QR PNGs)
- Replaced `crc32fast` with compile-time CRC32 lookup table
- Feature-gated `qrcode` + `base64` behind `qr` feature in domain crate (only worker enables it)
- Removed dependency tree: `miniz_oxide` + `simd-adler32` + `adler2` + `cfg-if`

## Files Changed

### PDPA (15 files, +329/-12)
| Layer | File | Change |
|-------|------|--------|
| Domain | `models/attendee.rs` | `ConsentGiven`/`PhotoConsent` ColumnKeys, 32-col mapping, tests |
| Domain | `models/event.rs` | `require_photo_consent` field on EventConfig, Create/UpdateEventRequest |
| Worker | `handlers/register.rs` | Validate + pass consent fields to sheet write |
| Worker | `handlers/public_event.rs` | Return `require_photo_consent` in event config |
| Worker | `sheets/write.rs` | Write consent values to AE/AF columns |
| Worker | `event_store/write.rs` | `require_photo_consent` in create/seed |
| Worker | `handlers/deposit/escrow/status.rs` | `require_photo_consent` in UpdateEventRequest construction |
| Frontend | `pages/privacy.rs` | New file — 11-section privacy policy |
| Frontend | `pages/mod.rs` | Add privacy route |
| Frontend | `lib.rs` | Wire privacy route in router |
| Frontend | `api/event.rs` | `require_photo_consent` in event types |
| Frontend | `pages/event_form.rs` | Photo consent toggle in organizer event form |
| Frontend | `pages/public_event/page.rs` | Pass photo consent config to registration form |
| Frontend | `pages/public_event/registration_form.rs` | Consent checkboxes + validation |
| Frontend | `pages/public_event/types.rs` | `consent_given`, `photo_consent_given` fields |

### WASM Optimization (5 files, +130/-22)
| File | Change |
|------|--------|
| `domain/Cargo.toml` | Feature-gate `qrcode` + `base64` behind `qr` |
| `domain/src/lib.rs` | `#[cfg(feature = "qr")]` on `pub mod qr` |
| `worker/Cargo.toml` | Explicit `features = ["qr"]` on domain dep |
| `frontend-leptos/Cargo.toml` | Remove `flate2` + `crc32fast` |
| `frontend-leptos/src/utils/qr_gen.rs` | Stored DEFLATE + compile-time CRC32 table |

## Tests
- 26/26 domain tests pass (including updated column mapping tests)
- Frontend compiles clean (`cargo check --target wasm32-unknown-unknown`)
- Worker compiles clean

## Remaining Work

### Immediate (pre-deploy)
- [ ] Add `consent_given` (AE) and `photo_consent` (AF) headers to Google Sheets attendee tabs
- [ ] Test registration flow end-to-end on devnet
- [ ] Build frontend with `trunk build` and verify WASM size reduction

### Backlog
| Priority | Issue | Effort |
|----------|-------|--------|
| 🔴 P2 | #042 Solana Mobile / MWA | ~1 week |
| 🟡 P2 | #040 Escrow test coverage | ~3 days |
| 🟡 P2 | #043 Phase D: Data retention & deletion API | ~4 hours |
| 🟢 P3 | Move QR to JS | ~4 hours |
| 🟢 P3 | #021 Frontend code quality | ~2 days |

## How to Dev/Test
1. `cargo test -p event-checkin-domain` — verify all 26 tests pass
2. `cargo check -p event-checkin-worker` — verify worker compiles
3. `cd frontend-leptos && cargo check --target wasm32-unknown-unknown` — verify frontend compiles
4. Deploy worker + frontend, then register for a test event to verify consent flow
