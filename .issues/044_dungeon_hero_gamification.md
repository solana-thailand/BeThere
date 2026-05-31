# Issue 044: Dungeon & Hero — Event Gamification Layer

> **Status: 📐 Design Doc** — Concept approved, ready for phased implementation.

## Problem

BeThere has a functional event lifecycle: register → deposit → check-in → claim NFT. But it feels like **admin work**, not an experience. The Rust Adventures game (#006) proved that interactive, themed mechanics engage attendees. The question is: **can we apply the same game metaphor to the core event flow itself?**

The attendee journey is already game-like — you enter, overcome challenges (show up, pay attention), and earn a reward (NFT). We just don't frame it that way.

## Solution

Re-theme the existing event flow as a **dungeon crawl**. Events are dungeons. Attendees are heroes. The mechanical flow stays identical — this is a **UI/UX reskin + thin progression layer**, not a rewrite.

```
Before:  Register → Pay Deposit → Check-in → Take Quiz → Claim NFT
After:   Enter Dungeon → Pay Entry Fee → Arrive at Dungeon → Prove Worth → Claim Loot
```

### Why This Works Now

- **Rust Adventures engine already exists** — tile renderer, puzzle system, progress tracking, star ratings
- **Adventure domain models already exist** — `AdventureConfig`, `AdventureProgress`, `LevelScore` in `domain/src/models/adventure.rs`
- **Star ratings on levels already map** to hero XP/leveling
- **cNFT badges already are "loot"** — just need thematic renaming
- **Deposit/escrow already is "entry fee"** — framing, not new code

---

## Metaphor Map → Existing Code

| Event Concept | RPG Equivalent | Existing Code | Change Needed |
|---------------|---------------|---------------|---------------|
| Event | **Dungeon** | `EventConfig`, `EventMeta` | Display name only (UI) |
| Attendee | **Hero** | `Attendee`, Google Auth profile | New: `HeroProfile` aggregate |
| Registration | **Enter Dungeon** | Public event page, registration form | UI copy + hero card creation |
| Deposit (USDC/THB) | **Entry Fee (Gold)** | `DepositStatus`, escrow flow | UI copy only |
| Check-in (QR scan) | **Arrive at Dungeon** | Scanner page, QR code | UI copy + hero status update |
| Quiz / Adventure | **Prove Worth (Trial)** | Quiz system, Rust Adventures | Already built |
| NFT Badge (cNFT) | **Loot (Badge)** | Claim page, Solana cNFT mint | UI copy only |
| Organizer | **Dungeon Master** | Admin panel, staff emails | UI label only |
| Multi-event attendance | **Campaign** | Events list, per-event tickets | New: cross-event aggregation |
| Event completion | **Dungeon Cleared** | Check-in + claim status | New: completion badge |
| Adventure star rating | **Hero XP** | `LevelScore.stars` | New: XP aggregation |

---

## New Data Model

### Hero Profile

Aggregated from existing data — no new KV entries for core fields. Computed client-side from existing APIs.

```rust
/// Hero profile — computed client-side from existing event/attendee data.
/// Stored in KV only for the cross-event aggregation (dungeons_cleared, total_xp).
///
/// KV key: `hero:{email}`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeroProfile {
    /// Hero name (from Google Auth display name or chosen alias).
    pub name: String,
    /// Hero title — earned from achievements.
    pub title: HeroTitle,
    /// Total XP across all events.
    pub total_xp: u32,
    /// Hero level (derived from XP: level = sqrt(xp / 50)).
    pub level: u32,
    /// Dungeons cleared (events where attendee checked in + claimed).
    pub dungeons_cleared: u32,
    /// Badges earned (cNFT NFTs claimed).
    pub badges_earned: u32,
    /// Star ratings from adventures.
    pub adventure_stars: u8,
    /// Achievement IDs unlocked.
    pub achievements: Vec<String>,
    /// First event date.
    pub started_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum HeroTitle {
    /// Default — no events yet.
    Stranger,
    /// 1 dungeon cleared.
    Adventurer,
    /// 3 dungeons cleared.
    Explorer,
    /// 5 dungeons cleared.
    Veteran,
    /// 10 dungeons cleared.
    Legend,
    /// Completed a Rust Adventure (any level).
    CodeWarrior,
    /// 3-star rating on any adventure level.
    PuzzleMaster,
    /// First event as Dungeon Master (organizer).
    DungeonMaster,
}

impl HeroTitle {
    pub fn display(&self) -> &str {
        match self {
            Self::Stranger => "Stranger",
            Self::Adventurer => "Adventurer",
            Self::Explorer => "Explorer",
            Self::Veteran => "Veteran",
            Self::Legend => "Legend",
            Self::CodeWarrior => "Code Warrior",
            Self::PuzzleMaster => "Puzzle Master",
            Self::DungeonMaster => "Dungeon Master",
        }
    }

    pub fn emoji(&self) -> &str {
        match self {
            Self::Stranger => "🌌",
            Self::Adventurer => "⚔️",
            Self::Explorer => "🗺️",
            Self::Veteran => "🛡️",
            Self::Legend => "👑",
            Self::CodeWarrior => "💻",
            Self::PuzzleMaster => "🧩",
            Self::DungeonMaster => "🎲",
        }
    }
}
```

### XP System

XP is earned per-event, not per-action. Keeps the system simple and prevents gaming.

| Action | XP | Condition |
|--------|-----|-----------|
| Enter Dungeon (register) | +10 | Registration confirmed |
| Pay Entry Fee (deposit) | +20 | Deposit verified |
| Arrive at Dungeon (check-in) | +30 | QR scanned |
| Prove Worth (quiz passed) | +15 | Quiz score ≥ threshold |
| Prove Worth (adventure passed) | +25 | Required levels completed |
| Claim Loot (NFT claimed) | +25 | cNFT minted |
| Dungeon Cleared (check-in + claim) | +50 bonus | Both check-in and claim done |
| Adventure Stars (per star) | +5 per star | 1-3 stars per level |

**Max XP per simple event:** 150 (register + check-in + claim + cleared bonus)
**Max XP per adventure event:** 185 (register + check-in + adventure + claim + cleared bonus)

**Level formula:** `level = floor(sqrt(total_xp / 50))`

| Total XP | Level | Title Available |
|----------|-------|-----------------|
| 0 | 1 | Stranger |
| 50 | 1 | — |
| 150 | 2 | — |
| 450 | 3 | — |
| 800 | 4 | — |
| 1250 | 5 | — |

### Quest System (Per-Event)

Quests are per-event challenges defined by organizers. A thin config layer on top of existing event data.

```rust
/// Quest configuration for an event.
/// Stored in KV: `event:{id}:quests`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestConfig {
    /// Whether quests are enabled for this event.
    pub enabled: bool,
    /// Quests defined by the organizer.
    pub quests: Vec<Quest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub name: String,
    pub description: String,
    /// How the quest is verified.
    pub quest_type: QuestType,
    /// XP bonus for completing this quest.
    pub xp_reward: u32,
    /// Whether this quest is required to "clear" the dungeon.
    pub required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QuestType {
    /// Check in at the event (automatic — already tracked).
    CheckIn,
    /// Attend a specific session/talk (manual verification by scanning a session QR).
    AttendSession { session_name: String },
    /// Visit a booth or sponsor (scan a booth-specific QR).
    VisitBooth { booth_name: String },
    /// Complete the quiz/adventure.
    CompleteTrial,
    /// Claim the NFT badge.
    ClaimLoot,
    /// Custom quest verified by staff scanning attendee's QR.
    Custom { verifier_note: String },
}

/// Per-attendee quest progress for an event.
/// KV key: `event:{id}:quest_progress:{attendee_api_id}`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestProgress {
    pub attendee_id: String,
    pub completed_quests: Vec<String>,
    pub all_required_done: bool,
    pub completed_at: Option<String>,
}
```

---

## UI Changes

### 1. Hero Card (New Component)

A compact profile card shown in the header/nav area. Displays hero name, title, level, XP bar, and badges.

```
┌──────────────────────────────┐
│ 🗡️ Adventurer Lv.2          │
│ ████████░░░░░░ 185/300 XP   │
│ 🏆 3 Dungeons ⭐ 5 Stars     │
└──────────────────────────────┘
```

**Location:** `frontend-leptos/src/components/hero_card.rs` (new)
**Data source:** Client-side aggregation from existing APIs + `hero:{email}` KV entry

### 2. Dungeon Event Card (Reskin)

The existing public event card gets RPG-themed labels:

| Before | After |
|--------|-------|
| "Register" | "Enter Dungeon" |
| "Deposit: 5 USDC" | "Entry Fee: 5 Gold" |
| "Check in" | "Arrive at Dungeon" |
| "Claim NFT" | "Claim Loot" |
| "Quiz" | "Trial of Worth" |
| Event status badges | Dungeon status (🔒 Locked / 🚪 Open / ✅ Cleared) |

**Implementation:** Themed copy in the public event page components. Controlled by a `gamification_enabled` flag on `EventConfig` (default: false, admin toggle).

### 3. Quest Tracker (New Component)

Shown on the ticket page when quests are enabled for an event. A checklist of quests with completion status.

```
┌─ Quests ─────────────────────┐
│ ✅ Enter Dungeon              │
│ ✅ Pay Entry Fee              │
│ ✅ Arrive at Dungeon          │
│ ☐ Visit 3 Sponsor Booths     │
│ ☐ Complete the Trial         │
│ ☐ Claim Your Loot            │
│                               │
│ Progress: 3/6  XP: +40 more  │
└──────────────────────────────┘
```

**Location:** `frontend-leptos/src/components/quest_tracker.rs` (new)

### 4. Dungeon Cleared Screen

When all required quests are done and NFT is claimed, show a "Dungeon Cleared" overlay with:
- ⭐ Star rating (1-3 based on quest completion speed/completeness)
- 🏆 Badge earned
- XP gained this dungeon
- Share to X button

### 5. Campaign Page (New Route: `/hero`)

A profile page showing the hero's journey across all events:

```
🗡️ Hero Profile — [Name]
━━━━━━━━━━━━━━━━━━━━━━━━━━
⚔️ Veteran  Lv.5
████████████░░ 1,250/1,800 XP

🏆 Dungeons Cleared: 7
⭐ Adventure Stars: 12
🎖️ Badges: 5
🧩 Puzzles Solved: 23

Recent Dungeons:
  ✅ Solana Bangkok Meetup #12  — Cleared ⭐⭐⭐
  ✅ Rust Workshop Weekend     — Cleared ⭐⭐
  🚪 Superteam Hackathon       — In Progress
```

**Location:** `frontend-leptos/src/pages/hero.rs` (new)
**Route:** `/hero`

---

## Architecture

### What's New

```
domain/src/models/
  hero.rs              — HeroProfile, HeroTitle, XP calculation
  quest.rs             — QuestConfig, Quest, QuestType, QuestProgress

frontend-leptos/src/
  components/
    hero_card.rs       — Hero profile card component
    quest_tracker.rs   — Per-event quest checklist
    dungeon_card.rs    — RPG-themed event card wrapper
  pages/
    hero.rs            — /hero — campaign/profile page

worker/src/handlers/
  hero.rs              — GET /api/hero/profile, POST /api/hero/sync
  quest.rs             — GET/POST /api/quests/{event_id}
```

### What Changes (Existing Files)

| File | Change |
|------|--------|
| `domain/src/models/event.rs` | Add `gamification_enabled: bool` to `EventConfig` |
| `domain/src/models/mod.rs` | Add `hero`, `quest` modules |
| `frontend-leptos/src/pages/public_event/` | RPG-themed copy when gamification enabled |
| `frontend-leptos/src/pages/ticket/page.rs` | Quest tracker component when quests enabled |
| `frontend-leptos/src/pages/claim.rs` | "Claim Loot" + Dungeon Cleared screen |
| `frontend-leptos/src/pages/mod.rs` | Add `/hero` route |
| `frontend-leptos/src/api/` | New API client functions for hero/quest |
| `worker/src/lib.rs` | Register hero/quest routes |

### What Stays The Same

- **Registration flow** — Same form, same Google Sheets backend
- **Deposit/escrow** — Same USDC/THB mechanics
- **Check-in/scanner** — Same QR scanning
- **NFT minting** — Same cNFT via Helius
- **Auth** — Same Google OAuth + JWT
- **Admin** — Same panel with new "Gamification" toggle

---

## Phased Implementation

### Phase 1 — Hero Profile & Card (2-3 days)

The foundation: hero data model, KV storage, profile page, header card.

- [ ] `domain/src/models/hero.rs` — HeroProfile, HeroTitle, XP formula
- [ ] `worker/src/handlers/hero.rs` — GET/POST /api/hero/profile
- [ ] `frontend-leptos/src/components/hero_card.rs` — Compact hero card
- [ ] `frontend-leptos/src/pages/hero.rs` — /hero campaign page
- [ ] Hero profile aggregation (client-side: sum events attended, NFTs claimed, adventure stars)
- [ ] `gamification_enabled` flag on EventConfig (default false)

**Delivers:** Attendees see a hero profile that levels up as they attend events. No behavior change yet.

### Phase 2 — Dungeon Reskin (1-2 days)

RPG-themed labels on the existing event flow. Controlled by `gamification_enabled`.

- [ ] `frontend-leptos/src/components/dungeon_card.rs` — RPG event card
- [ ] Public event page: "Enter Dungeon", "Entry Fee", themed status
- [ ] Ticket page: "Arrive at Dungeon", "Prove Worth", "Claim Loot"
- [ ] Claim page: "Dungeon Cleared" overlay with XP gain summary
- [ ] Event config admin toggle for gamification

**Delivers:** The core flow feels like a dungeon crawl. Zero mechanical changes.

### Phase 3 — Quest System (3-4 days)

Per-event quests defined by organizers. The real engagement driver.

- [ ] `domain/src/models/quest.rs` — QuestConfig, Quest, QuestType, QuestProgress
- [ ] `worker/src/handlers/quest.rs` — GET/POST quest config + progress
- [ ] `frontend-leptos/src/components/quest_tracker.rs` — Quest checklist on ticket page
- [ ] Admin UI for defining quests per event
- [ ] Booth QR codes for "Visit Booth" quests (reuse scanner infra)
- [ ] Quest completion → XP award → hero level up

**Delivers:** Attendees have specific goals within events. Organizers can drive booth traffic, session attendance.

### Phase 4 — Social & Sharing (1-2 days)

- [ ] "Share Dungeon Cleared" → auto-generated image for X/Twitter
- [ ] Hero profile shareable link
- [ ] Leaderboard per event (optional, organizer toggle)
- [ ] Party system (group registration) — future consideration

**Delivers:** Viral loop — attendees share their dungeon clears, attracting new heroes.

---

## Design Tokens Additions

Add to `DESIGN.md` and CSS `:root`:

```css
/* Gamification */
--xp-bar-bg: var(--bg-tertiary);
--xp-bar-fill: linear-gradient(90deg, var(--accent), var(--accent-purple));
--hero-gold: #f59e0b;
--hero-gold-bright: #fbbf24;
--dungeon-locked: var(--text-muted);
--dungeon-open: var(--accent);
--dungeon-cleared: var(--success);
--quest-complete: var(--success);
--quest-pending: var(--text-secondary);
```

## Risks

| Risk | Mitigation |
|------|------------|
| Gamification feels forced/cringy | `gamification_enabled` is per-event, off by default. Organizers choose the theme. |
| Quest system too complex for simple events | Quests are optional. An event with no quests = current behavior. |
| Hero profile KV reads on every page load | Cache in session state. KV read on login, refresh on quest/deposit/check-in events. |
| RPG theme alienates non-gamer audiences | Only UI copy changes. Mechanics stay familiar. Can use "Achievements" language instead of "Quests" for corporate events. |
| Scope creep into full game | Hard scope: no real-time multiplayer, no leaderboards (Phase 4 optional), no item economy. XP is cosmetic only. |

## Out of Scope

- Real-time multiplayer / party system
- In-game currency or marketplace
- On-chain hero profile (stays in KV for now, D1 after #037)
- Leaderboards (consider for Phase 4)
- AR/VR dungeon visuals
- Competitive modes between attendees

## Refs

- `.issues/006_rust_adventures.md` — Existing adventure engine
- `.issues/038_curriculum_design_vision.md` — Curriculum/credit system
- `.issues/035_ticket_state_hero_claim_redirect.md` — Ticket page state-driven hero (already uses "hero" terminology)
- `.issues/031_capstone_project_definition_market_analysis.md` — Target market: web3 community organizers
- `.design/DESIGN.md` — Design system
