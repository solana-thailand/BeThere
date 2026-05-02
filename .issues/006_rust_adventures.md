# Issue 006: Rust Adventures — Interactive Learning Game

> **Status: 🚧 Phase 2 COMPLETE** — Core engine + interactive puzzles + multi-level support implemented.

## Problem

The current quiz system (Issue 002) is a flat multiple-choice gate — functional but not engaging. For **Rust programming education**, static MCQs don't build muscle memory or deep understanding. Learners need progressive, hands-on practice that mirrors how Vim Adventures teaches Vim: start simple, unlock skills gradually, solve puzzles with what you've learned.

Additionally, events centered on Rust workshops (e.g., Solana bootcamps) want a richer assessment than "5 questions, 60% pass." They want attendees to **demonstrate skills** before earning the NFT badge.

## Solution

Build **Rust Adventures** — a tile-based puzzle game embedded in the existing Leptos frontend at `/adventure`. Players navigate a grid world, collect Rust keyword keys, solve code puzzles, and progress through levels that teach Rust concepts from zero to intermediate.

**Key principle:** No game engine (Bevy). Pure Leptos + CSS Grid. Tile-based discrete movement. Same WASM binary, same deployment, same auth/KV infra.

### Flow

```
Standalone:   /adventure → Play levels → Learn Rust → Track progress in KV
Claim-linked: /claim/{token} → Redirect to /adventure?token=xxx&event=yyy
              → Complete required level → Quiz status = Passed → Claim NFT
```

---

## Game Design

### Core Mechanics

| Mechanic | Description |
|----------|-------------|
| **Grid movement** | Arrow keys / WASD / hjkl — tile-by-tile, no physics |
| **Key collection** | Pick up Rust keywords (e.g., `let`, `mut`, `fn`) that unlock abilities |
| **Code puzzles** | Arrange code snippets, fix errors, complete expressions |
| **NPCs** | Characters that give hints and explain concepts |
| **Doors/gates** | Blocked by puzzles — solve to proceed |
| **Progressive unlock** | Each level teaches 2-3 new concepts; can't skip |

### Level Progression — Rust Curriculum

| Level | Name | Concept | Keys Collected | Boss Puzzle |
|-------|------|---------|----------------|-------------|
| 1 | Hello World | Basic program structure | `fn`, `let`, `println!` | Write a hello world |
| 2 | Variables | Immutability, mut, shadowing | `mut`, `const`, `shadow` | Fix compile errors |
| 3 | Types | Primitives, strings, inference | `i32`, `String`, `bool`, `as` | Type the correct type |
| 4 | Control Flow | if/else, match, loops | `if`, `match`, `loop`, `for` | Navigate using conditionals |
| 5 | Functions | Params, return, visibility | `pub`, `->`, `return` | Build function bridges |
| 6 | Ownership | Move, borrow, clone | `&`, `&mut`, `clone`, `move` | Pass values through gates |
| 7 | Structs & Enums | Data structures | `struct`, `enum`, `impl` | Construct objects |
| 8 | Pattern Matching | Destructuring, guards | `Some`, `None`, `Ok`, `Err` | Match patterns to defeat bugs |
| 9 | Error Handling | Result, Option, ? operator | `Result`, `?`, `unwrap` | Handle errors to proceed |
| 10 | Traits | Trait definitions, impl, derive | `trait`, `impl`, `derive` | Implement traits for final door |

### Tile Types

| Tile | Symbol | Behavior |
|------|--------|----------|
| Floor | `.` | Walkable |
| Wall | `#` | Blocked |
| Player start | `@` | Player spawn |
| Exit | `>` | Level complete (when conditions met) |
| Key | `k` | Collectible keyword (name stored in metadata) |
| NPC | `n` | Talk on bump (dialog stored in metadata) |
| Gate | `=` | Blocked until puzzle solved |
| Code block | `c` | Interactive — shows code puzzle |
| Water | `~` | Impassable (visual variety) |
| Sign | `s` | Read-only hint text |

### Map Data Format

Each level is a JSON object:

```json
{
  "id": "level_01",
  "name": "Hello World",
  "concept": "Basic program structure",
  "width": 12,
  "height": 10,
  "grid": [
    "############",
    "#..........#",
    "#.@..k(fn).#",
    "#..........#",
    "#....n.....#",
    "#..........#",
    "#..=(puzzle)#",
    "#..........#",
    "#.........>#",
    "############"
  ],
  "keys": [
    { "pos": [2, 3], "name": "fn", "description": "fn declares a function" }
  ],
  "npcs": [
    { "pos": [5, 4], "name": "Ferris", "dialog": "In Rust, every program starts with fn main()!" }
  ],
  "puzzles": [
    {
      "id": "hello_world",
      "gate_pos": [3, 6],
      "type": "arrange",
      "instruction": "Arrange these lines to create a valid Rust program:",
      "pieces": ["}", 'println!("Hello!");', "fn main() {", "    "],
      "solution": "fn main() {\n    println!(\"Hello!\");\n}",
      "hint": "Functions start with fn, body goes inside braces"
    }
  ],
  "exit_pos": [10, 8],
  "required_keys": ["fn"],
  "intro_text": "Welcome to Rustland! Collect the `fn` key to begin.",
  "completion_text": "You wrote your first Rust program!"
}
```

---

## Architecture

### Frontend — Leptos Components

```
frontend-leptos/src/
  pages/
    adventure.rs         — Main game page (route: /adventure)
    adventure/
      mod.rs             — Re-exports
      engine.rs          — Game loop, input handling, movement, collision
      renderer.rs        — CSS Grid map renderer, tile sprites
      puzzle.rs          — Code puzzle UI (arrange, fill-blank, fix-error)
      dialog.rs          — NPC dialog boxes
      hud.rs             — Heads-up display (level, keys, score)
      level_select.rs    — Level selection screen
      types.rs           — Tile, Level, Puzzle, GameState types
```

### Rendering Approach — CSS Grid

No Canvas, no WebGL. Pure DOM elements in a CSS Grid:

```html
<div class="game-grid" style="grid-template-columns: repeat(12, 32px)">
  <div class="tile tile-wall">█</div>
  <div class="tile tile-floor">·</div>
  <div class="tile tile-player">@</div>
  ...
</div>
```

**Why CSS Grid over Canvas?**
- Leptos reactivity — signals update tiles directly
- Keyboard accessibility for free
- DOM animations (CSS transitions for movement)
- Smaller bundle (no canvas rendering code)
- Touch-friendly (tap tiles on mobile)
- Vim Adventures uses Canvas, but its maps are larger. Our 12x10 grid is fine with DOM.

**Tile size:** `32px` desktop, `24px` mobile (responsive via CSS variable)

### State Management

```rust
// Core game state as Leptos signals
struct GameState {
    current_level: usize,
    player_pos: (usize, usize),
    collected_keys: HashSet<String>,
    solved_puzzles: HashSet<String>,
    npc_dialogs_read: HashSet<(usize, (usize, usize))>,
    levels_completed: Vec<String>,
    moves_count: u32,
    active_dialog: Option<NpcDialog>,
    active_puzzle: Option<PuzzleState>,
}
```

### Persistence — Cloudflare KV

Progress saved per-user (auth) or per-claim-token (claim flow):

**Key schema:**
```
event:{id}:adventure:config          → AdventureConfig (JSON) — level data
event:{id}:adventure:progress:{uid}  → AdventureProgress (JSON) — user state
```

**AdventureProgress:**
```json
{
  "user_id": "email@example.com",
  "claim_token": "uuid-or-null",
  "levels_completed": ["level_01", "level_02"],
  "total_keys_collected": ["fn", "let", "mut", "println!"],
  "scores": {
    "level_01": { "moves": 42, "puzzles_solved": 1, "time_seconds": 120 },
    "level_02": { "moves": 35, "puzzles_solved": 2, "time_seconds": 180 }
  },
  "last_played_at": "2025-01-15T10:30:00Z"
}
```

### Backend — New API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/adventure` | No (or Cookie) | Get adventure levels for event |
| GET | `/api/adventure/progress` | Cookie | Get current user's progress |
| POST | `/api/adventure/progress` | Cookie | Save level completion |
| PUT | `/api/admin/adventure` | Cookie + Staff | Create/update adventure config |
| GET | `/api/adventure/{token}/status` | No | Check if claim token has completed required level |

### Claim Integration

**Option A: Redirect pattern (recommended)**
```
/claim/{token}
  → check quiz_status
  → if adventure_required: redirect to /adventure?token={token}&event={id}&required_level={n}
  → after completing level: POST /api/adventure/progress → mark quiz as passed → redirect back
```

**Option B: Embedded pattern**
```
/claim/{token}
  → claim page embeds the adventure component inline
  → no redirect needed, but heavier claim page
```

**Recommended: Option A** — cleaner separation, adventure page can be full-screen, existing quiz stays as fallback.

**EventConfig extension:**
```rust
pub struct EventConfig {
    // ... existing fields ...
    pub adventure_enabled: bool,           // default false
    pub adventure_required_level: Option<u32>,  // e.g., 5 = must complete level 5
}
```

---

## Puzzle Types

### 1. Arrange (Drag-to-order)
Player arranges shuffled code lines into correct order.
```
Given:  ["}", 'println!("Hi!");', "fn main() {"]
Solve:  "fn main() {\n    println!(\"Hi!\");\n}"
```

### 2. Fill-in-the-blank
Code with blanks, player selects from keyword options.
```
Given:  "___ x = 5;"  options: ["let", "mut", "fn"]
Solve:  "let x = 5;"
```

### 3. Fix-the-error
Code with a bug, player identifies the fix.
```
Given:  "fn main() {\n    let x: i32 = \"hello\";\n}"
Fix:    Change i32 to &str (select from options)
```

### 4. Match-type
Match expressions with their types (memory card flip).
```
Pairs: ["42" → "i32", "\"hello\"" → "&str", "true" → "bool"]
```

### 5. Navigate-with-code
Player writes a short expression that controls game behavior.
```
"If x is 5, the gate opens. What goes in the blank?"
Code: "if x __ 5 { gate.open(); }"
Answer: "=="
```

---

## Mobile Support

| Feature | Implementation |
|---------|---------------|
| Movement | D-pad overlay (4 directional buttons) |
| Tiles | Smaller size (24px), scrollable viewport |
| Puzzles | Full-screen modal, touch-friendly |
| Viewport | Camera follows player, shows 7x5 tile window |

### D-pad Controller

```
        [↑]
    [←] [·] [→]
        [↓]
```

Floating overlay, bottom-right corner. Auto-shows on touch devices (`@media (hover: none)`).

---

## Phased Implementation Plan

### Phase 1 — Core Engine (MVP)
**Goal:** Playable test level with grid movement.

- [ ] Tile types and level data model (`types.rs`)
- [ ] CSS Grid renderer (`renderer.rs`)
- [ ] Keyboard input + player movement (`engine.rs`)
- [ ] Collision detection (walls, gates)
- [ ] Key collection mechanic
- [ ] Basic HUD (level name, keys collected)
- [ ] Route `/adventure` with test level
- [ ] D-pad for mobile

### Phase 2 — Puzzles & NPCs
**Goal:** Interactive code puzzles and NPC dialogs.

- [ ] NPC dialog system (`dialog.rs`)
- [ ] Code puzzle UI — arrange type (`puzzle.rs`)
- [ ] Gate mechanic (blocked until puzzle solved)
- [ ] Level completion detection
- [ ] Level transition animation
- [ ] All 5 puzzle types implemented

### Phase 3 — Level Content
**Goal:** Rust levels 1-5 with real educational content.

- [ ] Level data for levels 1-5 (Hello World through Functions)
- [ ] Level selection screen (`level_select.rs`)
- [ ] Tile sprites / visual polish (emoji-based to start)
- [ ] Intro/outro text per level
- [ ] Score tracking (moves, time, puzzles solved)

### Phase 4 — Persistence & Backend
**Goal:** Save progress to KV, admin configuration.

- [ ] KV storage for adventure config and progress
- [ ] API endpoints (GET/POST progress, PUT admin config)
- [ ] Auto-save on level completion
- [ ] Admin UI for adventure configuration

### Phase 5 — Claim Integration
**Goal:** Adventure-gated claim flow.

- [ ] EventConfig fields (`adventure_enabled`, `adventure_required_level`)
- [ ] Claim page redirect to adventure
- [ ] Adventure completion marks quiz as passed
- [ ] End-to-end flow: check-in → adventure → claim → NFT

### Phase 6 — Polish & Content
**Goal:** Production-ready with levels 1-10.

- [ ] Levels 6-10 (Ownership through Traits)
- [ ] Animations (key pickup, gate open, level complete)
- [ ] Sound effects (optional, toggle)
- [ ] Mobile polish (viewport camera, touch responsiveness)
- [ ] Accessibility (keyboard-only, screen reader hints)

---

## Technical Decisions

### Why Not Bevy?
- Bevy compiles to WASM but produces 5-10MB+ bundles (vs current ~500KB Leptos)
- Two WASM modules (Leptos + Bevy) can't share memory — need JS bridge
- Tile-based 2D grid doesn't need physics, ECS, or rendering pipeline
- Leptos signals + CSS Grid is sufficient and keeps single deployment

### Why Not Canvas?
- Vim Adventures uses Canvas, but our grids are small (12x10)
- CSS Grid gives free keyboard accessibility, responsive layout, and Leptos reactivity
- Canvas would require custom hit-testing, no semantic HTML for screen readers
- If performance becomes an issue at higher tile counts, we can migrate to Canvas later

### Why Standalone Route (Not Separate Project)?
- Reuses auth (Google OAuth + JWT), KV, event system
- Single `wrangler deploy` for everything
- Adventure progress and quiz progress share the same KV namespace pattern
- Same CI/CD pipeline

### Why CSS Emoji Sprites (Not Pixel Art)?
- No asset pipeline needed
- Cross-platform, no image loading
- Small bundle (text only)
- Can upgrade to SVG sprites or pixel art later without engine changes

---

## Risks

| Risk | Mitigation |
|------|------------|
| Leptos WASM performance for game loop | Tile-based is low-frequency (move per keypress, not 60fps). Signal updates are cheap. |
| Puzzles too hard / too easy | Start with 3 levels, iterate based on feedback. Admin can configure difficulty. |
| Mobile controls awkward | D-pad overlay + swipe support. Touch devices auto-detect. |
| Bundle size increase | Levels are JSON data (~2-5KB each). No images. Total engine ~15-20KB gzipped. |
| Code puzzle security | Puzzles compare against stored solution text, not executing user code. |

---

## Refs

- Inspiration: https://vim-adventures.com/
- Existing quiz: `.issues/002_quiz_gated_claim.md`
- Multi-event system: `.issues/004_multi_event_management.md`
- Design brief: `.design/brief.md`
