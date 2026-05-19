# 023 — Organizer Platform Integration (White-Label / Partner API)

## Summary

Enable external organizer communities (e.g., Solana Thailand Genesis) to use BeThere as their event management backbone, replacing manual HTML/static-site event announcements with a self-service platform.

## Current State — Solana Thailand

Solana Thailand runs a Zola static site (`solana-thailand.github.io/genesis/`) with:
- **Manually coded event cards** — HTML edited directly in the repo
- **Event registration** via Luma (third-party, no deposit/commitment mechanism)
- **No attendee management** — Luma handles RSVPs, no check-in or NFT claiming
- **No deposit commitment** — free events, no skin-in-the-game
- **Community features**: Quest Board, Leaderboard, Rules of Engagement, Discord

### What BeThere Already Has

| Feature | Status |
|---------|--------|
| Multi-event management | ✅ Full CRUD API, per-event config |
| Organizer role | ✅ `organizer_emails` per event, access control |
| Attendee registration | ✅ Google Sign-In + sheet-based |
| Deposit commitment | ✅ USDC (on-chain escrow) + THB (slip verification) |
| QR check-in | ✅ Scanner page, walk-in registration |
| NFT badges | ✅ cNFT minting via Bubblegum |
| Public event pages | ✅ `/e/{slug}` — per-event landing |
| Landing page | ✅ Upcoming events, event cards |
| Event enrichment | ✅ tagline, location, badge image on cards |

## Integration Approaches

### Option A: Organizer Self-Service (Recommended — Low Effort)

Solana Thailand organizers sign up as BeThere users → get `organizer` role → create/manage events via the existing admin UI.

**Flow:**
```
1. Admin adds organizer emails to BeThere (super_admin or first event seed)
2. Organizer signs in → sees /staff dashboard
3. Creates event via admin UI (name, dates, location, Google Sheet, etc.)
4. Event appears on BeThere landing page AND gets a unique /e/{slug}
5. Solana Thailand site embeds or links to BeThere event pages
6. Organizer manages attendees, deposits, check-ins via BeThere admin
```

**Changes needed:**
- [ ] Allow organizer signup flow (not just super_admin creating events)
- [ ] Optional: embeddable event widget (`<iframe>` or JS snippet) for Solana Thailand site
- [ ] Optional: custom branding per organizer (logo, colors on `/e/{slug}`)

### Option B: Headless API (Medium Effort)

Solana Thailand builds their own frontend that calls BeThere API endpoints.

**Flow:**
```
1. Organizer authenticates via Google OAuth → gets JWT
2. Solana Thailand frontend calls:
   - POST /api/events (create event)
   - GET /api/public/events (list events for their site)
   - POST /api/public/register (register attendee)
3. BeThere handles: sheets, deposits, check-in, NFTs
```

**Changes needed:**
- [ ] API key or scoped JWT for non-browser clients
- [ ] CORS configuration for cross-origin requests
- [ ] Webhook for event state changes (optional)

### Option C: Multi-Tenant Platform (High Effort — Future)

Full white-label platform where each organizer gets their own branded space.

```
bethere.so/solana-thailand/  → custom landing, events, branding
bethere.so/xyz-dao/          → different org, different branding
```

**Changes needed:**
- [ ] Organization entity (name, slug, logo, custom domain)
- [ ] User-Organization membership model
- [ ] Per-organization landing page
- [ ] Custom CSS/branding per org
- [ ] Billing/metering

## Recommendation

Start with **Option A** — it requires zero code changes for the basic flow. Solana Thailand organizers:
1. Get added as `organizer_emails` on their events
2. Create events via the existing admin UI at `/staff`
3. Link from their static site to BeThere event pages (`/e/{slug}`)
4. Optionally embed BeThere event cards via iframe

**Quick wins to enhance Option A:**
- Add `organizer_name` / `organizer_logo_url` to `EventMeta` → show "Organized by Solana Thailand" on event cards
- Embeddable event list widget (JS snippet that fetches `/api/public/events` and renders cards)
- Open event creation to any `organizer` role user (currently restricted to super_admin)

## Refs

- `.handovers/066_upload_auth_landing_walkin_diagnostics.md` — this session's handover
- Solana Thailand site: https://solana-thailand.github.io/genesis/
- `worker/src/handlers/events.rs` — event CRUD
- `worker/src/auth.rs` — role-based access control
- `frontend-leptos/src/pages/landing.rs` — public event discovery
