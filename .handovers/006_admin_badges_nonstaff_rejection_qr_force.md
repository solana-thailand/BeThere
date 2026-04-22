# Handover 006: Admin Badges, Non-Staff Rejection, QR Force Regenerate

**Date**: 2026-04-21
**Branch**: main
**Commits**: `dbb67de`, `95c9951`, `7e9da77`

## What Happened

User reported 3 issues/requests:
1. Admin page should show separate participation type badges (In-Person vs Online) for each attendee
2. Non-staff Google accounts (e.g. participants) trying to login need better rejection UX
3. QR Code Generation was skipping all attendees — needed investigation and fix

## Changes Made

### 1. Admin Participation Type Badges (`dbb67de`)

**Files**: `frontend/css/style.css`, `frontend/js/admin.js`

- Added `getParticipationBadge()` helper that parses long participation_type strings (e.g. "In-Person (Bangkok): Attend at the venue...") into short labels
- In-Person → `badge-info` (blue), Online → `badge-warning` (amber)
- Each attendee card now shows participation badge + check-in status badge
- Recent Check-Ins tab also shows participation badge (looks up from `allAttendees` by `api_id`)
- Added 2 new stat cards: In-Person count and Online count (computed client-side)
- Stats grid changed from 3-column to 6-column layout (each card spans 2 cols, participation cards centered on 2nd row)
- Separated API errors from rendering errors in `loadDashboard()` for better diagnosis

### 2. Non-Staff Login Rejection (`95c9951`)

**Files**: `frontend/js/app.js`, `frontend/index.html`

- Improved `not_authorized` error message: "⛔ Access Denied — This system is for authorized staff only..."
- Added `.not-authorized` CSS class with thicker border, larger text, centered layout
- Added `role="alert"` and `aria-live="assertive"` to error message div
- Backend already had 3-layer protection (OAuth callback, auth middleware, login page) — no backend changes needed
- Updated `api.generateQrs()` to accept `force` boolean parameter

### 3. Force QR Regenerate + Diagnostics (`7e9da77`)

**File**: `src/handlers/qr.rs`

- Added `force` query parameter: `POST /api/generate-qrs?force=true`
- When `force=true`, regenerates QR URLs for ALL approved attendees (overwrites existing)
- Added comprehensive diagnostic logging:
  - Total fetched, total approved, approval status distribution
  - Count with existing QR URLs vs without
  - Sample existing QR URL values (first 3)
  - Per-attendee skip reasons when all are skipped
- Response now includes detailed skip reasons per attendee
- Admin UI shows "🔄 Force Regenerate All" button after first generation attempt

## Key Findings

### QR "All Skipped" Root Cause
Most likely cause: **Column K already has values** in the Google Sheet. The `generate_qr_urls()` filter skips any attendee with a non-empty `qr_code_url`. This happens when:
- QR codes were previously generated
- Column K has other data/formulas
- Someone manually entered data

The `force=true` parameter overwrites existing values.

### "Failed to Load Dashboard" Error
User reported this after server restart. Root cause analysis:
- The original `loadDashboard()` catch block caught ALL errors including rendering errors
- If new rendering code (stat card creation, badge display) threw, it showed generic "failed to load dashboard data"
- **Fixed**: separated API errors (network/server) from rendering errors (JS bugs)
- May also be caused by browser caching old JS files → user should hard-refresh (Cmd+Shift+R)

## Plan / Code / Test

### Code Location
| Feature | Backend | Frontend |
|---------|---------|----------|
| Participation badges | N/A (data already in API response) | `frontend/js/admin.js` — `getParticipationBadge()`, `renderStats()`, `renderAttendeeList()`, `renderRecentCheckIns()` |
| Stats grid layout | N/A | `frontend/css/style.css` — `.stats-grid`, `.stat-card:nth-child(4/5)` |
| Non-staff rejection | `src/handlers/auth.rs` (already existed) | `frontend/js/app.js` — `showLoginError()`, `frontend/index.html` |
| Force QR regenerate | `src/handlers/qr.rs` — `GenerateQrQuery`, `generate_qrs()` | `frontend/js/app.js` — `api.generateQrs(force)`, `frontend/js/admin.js` — `handleGenerateQrs(force)` |

### How to Test
1. **Badges**: Login as staff → Admin page → verify each attendee shows In-Person (blue) or Online (amber) badge
2. **Stats**: Check 2nd row of stats grid shows In-Person and Online counts
3. **Non-staff rejection**: Login with a non-staff Gmail → should see prominent error message
4. **QR Generate**: Admin → "Generate QR Codes" → if all skipped, use "Force Regenerate All"
5. **Dashboard error**: Hard-refresh (Cmd+Shift+R) admin page → check browser console (F12) for any errors
6. **Server logs**: Check `RUST_LOG=info` output for QR diagnostic details

### Tests
```
cargo test — 25 passed, 0 failed
node -c frontend/js/admin.js — syntax OK
node -c frontend/js/app.js — syntax OK
```

## Reflections / Struggles

1. **Admin.js changes were interleaved** — badges, QR UI, and error handling changes couldn't be cleanly separated into independent commits. Accepted some overlap.
2. **"Failed to load dashboard" debugging** — couldn't reproduce server-side since it requires auth. Improved error separation so next occurrence shows which layer failed.
3. **Stats grid layout** — used CSS 6-column grid with `grid-column: span 2` and `nth-child` positioning to center the 2 participation cards on the second row. Works but somewhat fragile if more cards are added later.

## Remain Work

- [ ] User to test admin page with real data (hard-refresh browser first)
- [ ] User to test QR generation — check server logs for diagnostic output
- [ ] User to verify participation type values match their Google Sheet column Y values
- [ ] If "failed to load dashboard" persists, check browser console for exact error and server logs for Google Sheets API errors
- [ ] Consider adding `participation_type` filter to search in admin (filter by In-Person/Online)
- [ ] Consider only generating QR codes for In-Person attendees (currently generates for both)
- [ ] Leptos frontend rewrite (see handover 005 for plan)

## Issues Ref

- QR code generation skipping all attendees
- Admin dashboard "failed to load dashboard data" on refresh
- Non-staff Google account login attempt handling