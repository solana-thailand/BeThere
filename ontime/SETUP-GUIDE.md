# Ontime + BeThere Setup Guide

> **Event**: Solana Developer Thailand — April 26, 2026
> **Venue**: localhost (Ontime) + Cloudflare Workers (BeThere)
> **Google Sheet**: Shared between both systems

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Google Sheet                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ checkin  │  │  staff   │  │       ontime          │  │
│  │ (tab)    │  │  (tab)   │  │  (tab) ← NEW          │  │
│  │          │  │          │  │  Event schedule for    │  │
│  │ Attendee │  │ Staff    │  │  Ontime countdown &    │  │
│  │ data     │  │ emails   │  │  stage displays        │  │
│  └────┬─────┘  └────┬─────┘  └───────────┬──────────┘  │
└───────┼──────────────┼────────────────────┼─────────────┘
        │              │                    │
        ▼              ▼                    ▼
┌──────────────────┐              ┌─────────────────────┐
│ BeThere Worker   │              │ Ontime (localhost)  │
│ (Cloudflare)     │              │ http://localhost:4001│
│                  │              │                     │
│ /staff  Scanner  │              │ /editor   Control   │
│ /admin  Dashboard│              │ /timer    Countdown │
│ /api/*  REST API │              │ /backstage Schedule │
└──────────────────┘              │ /timeline Overview  │
                                  └─────────────────────┘
```

**Each system uses its own auth to access the SAME Google Sheet:**
- BeThere: Service account (already configured)
- Ontime: OAuth "TVs and Limited input device" flow (new setup below)

---

## Part 1: Add "ontime" Tab to Your Google Sheet

### Step 1.1 — Open your existing Google Sheet

Open the Google Sheet that BeThere uses (the one with `GOOGLE_SHEET_ID`).

### Step 1.2 — Create a new tab

1. Click the **+** button at the bottom left to add a new sheet/tab
2. Rename it to **`ontime`** (right-click the tab → Rename)

### Step 1.3 — Add headers (Row 1)

Copy these headers into Row 1, one per column (A through M):

| Column | Header |
|--------|--------|
| A | Time start |
| B | Link start |
| C | Duration |
| D | Cue |
| E | Title |
| F | Skip |
| G | Note |
| H | Colour |
| I | End action |
| J | Timer type |
| K | Time warning |
| L | Time danger |
| M | Presenter |

### Step 1.4 — Add the schedule data (Rows 2-9)

Paste this data starting from Row 2:

| Row | A (Time start) | B (Link start) | C (Duration) | D (Cue) | E (Title) | F (Skip) | G (Note) | H (Colour) | I (End action) | J (Timer type) | K (Time warning) | L (Time danger) | M (Presenter) |
|-----|----------------|-----------------|--------------|---------|-----------|----------|----------|------------|----------------|----------------|------------------|-----------------|---------------|
| 2 | 09:30 | FALSE | 00:30 | 1 | Registration | FALSE | Registration desk opens | grey | load-next | count-down | 00:05:00 | 00:01:00 | |
| 3 | 10:00 | FALSE | 00:10 | 2 | Opening by Solana Developer Thailand & Solana Thailand DAO | FALSE | Community Roadmap & Intro to the Dev Series | green | load-next | count-down | 00:02:00 | 00:01:00 | Solana Developer Thailand & Solana Thailand DAO |
| 4 | 10:10 | FALSE | 00:50 | 3 | Rust AI and Gaming Ep. 2 | FALSE | Deep dive session | blue | load-next | count-down | 00:05:00 | 00:02:00 | Katopz |
| 5 | 11:00 | FALSE | 00:10 | 4 | Group Photo Session | FALSE | Everyone gather for group photo | yellow | load-next | count-down | 00:02:00 | 00:00:30 | |
| 6 | 11:10 | FALSE | 00:30 | 5 | Hands-on: Solana Account Model & Building Your First NFT with Metaplex | FALSE | Workshop session | purple | load-next | count-down | 00:05:00 | 00:02:00 | Golf |
| 7 | 11:40 | FALSE | 00:15 | 6 | The Future of On-chain Gaming & Ephemeral Rollups | FALSE | Tech talk | orange | load-next | count-down | 00:02:00 | 00:01:00 | Andy (Magicblock) |
| 8 | 11:55 | FALSE | 00:15 | 7 | APAC Ecosystem Spotlight - Foundation Updates & Opportunities | FALSE | Ecosystem updates | green | load-next | count-down | 00:02:00 | 00:01:00 | Chaerin (Solana Foundation) |
| 9 | 12:10 | FALSE | 00:50 | 8 | Networking Session | FALSE | Open networking and discussions | grey | none | count-down | 00:05:00 | 00:02:00 | |

**Important notes on format:**
- **Time start**: Use plain text `09:30` format, NOT a Google Sheets time value. Select column A → Format → Number → Plain Text
- **Duration**: Use plain text `00:30` format. Same — Format → Number → Plain Text
- **Link start / Skip**: Must be uppercase `FALSE` or `TRUE`
- **Colour**: Named CSS colours (`grey`, `green`, `blue`, etc.) or hex (`#FF5500`)
- **End action**: `load-next` (auto-loads next event), `play-next` (auto-plays next), or `none`
- **Timer type**: `count-down` is the standard for event talks

### Step 1.5 — Get the Sheet ID

From your Google Sheet URL:
```
https://docs.google.com/spreadsheets/d/SHEET_ID_HERE/edit
```

Copy the `SHEET_ID_HERE` part. You'll need it for Ontime.

### Step 1.6 — Share the sheet (if needed)

If Ontime uses a different Google account than the sheet owner:
1. Click **Share** button (top right)
2. Add the Google account you'll use with Ontime
3. Set permission to **Editor**

---

## Part 2: Install Ontime on localhost

### Option A: npm (fastest — you have Node.js)

```bash
# Install globally
npm install -g @getontime/cli

# Start Ontime
ontime
```

### Option B: npx (no install, runs directly)

```bash
npx @getontime/cli
```

### Option C: macOS Desktop App

1. Go to [https://www.getontime.no/](https://www.getontime.no/)
2. Download **macOS ARM** (Apple Silicon) or **macOS Intel**
3. Open the `.dmg` file, drag Ontime to Applications
4. Launch Ontime from Applications

### Verify

Open your browser and go to:
```
http://localhost:4001/editor
```

You should see the Ontime Editor interface.

---

## Part 3: Set Up Google Sheets Sync in Ontime

This is a one-time setup. Ontime uses its own OAuth credentials (separate from BeThere's service account).

### Step 3.1 — Create a Google Cloud Project

1. Go to [https://console.cloud.google.com/](https://console.cloud.google.com/)
2. Click **"Select a project"** → **"New Project"**
3. Name it: `Ontime Sync` (or any name you like)
4. Click **Create**
5. Select the newly created project

### Step 3.2 — Enable the Google Sheets API

1. Open the sidebar → **"APIs & Services"** → **"Library"**
2. Search for **"Google Sheets API"**
3. Click it → **Enable**

### Step 3.3 — Configure OAuth Consent Screen

1. Go to **"APIs & Services"** → **"OAuth consent screen"**
2. Choose **External** (unless you have a Google Workspace)
3. Fill in:
   - App name: `Ontime Sync`
   - User support email: your email
   - Developer contact email: your email
4. Click **Save and Continue**

### Step 3.4 — Add Scopes

1. On the Scopes step, click **"Add or Remove Scopes"**
2. Filter by `sheets`
3. Enable: `.../auth/spreadsheets` (read and write your spreadsheets)
4. Click **Update** → **Save and Continue**

### Step 3.5 — Add Test Users

1. On the Test Users step, click **"Add Users"**
2. Add the Gmail address you'll use with Ontime
3. Click **Save and Continue**

### Step 3.6 — Create OAuth Credentials

1. Go to **"APIs & Services"** → **"Credentials"**
2. Click **"+ Create Credentials"** → **"OAuth client ID"**
3. Application type: **"TVs and Limited input devices"**
4. Name: `Ontime`
5. Click **Create**
6. **Download the JSON file** (click the download icon)
7. Save it somewhere accessible, e.g. `~/Downloads/ontime-credentials.json`

### Step 3.7 — Connect Ontime to Google Sheets

1. In Ontime Editor (`http://localhost:4001/editor`)
2. Go to **Project settings** (gear icon or menu)
3. Find **"Sheet Sync"** section
4. Upload the credentials JSON file you downloaded
5. Enter your **Google Sheet ID** (from Step 1.5)
6. Click **Connect**
7. Ontime will show a code — copy it
8. Click the authentication link — Google will ask you to authorize
9. Paste the code and authorize
10. Ontime is now connected!

### Step 3.8 — Import the schedule

1. In Ontime, go to **Sheet Sync** settings
2. Select the **"ontime"** worksheet/tab
3. Map the columns:
   - `Time start` → Time start
   - `Duration` → Duration
   - `Cue` → Cue
   - `Title` → Title
   - `Note` → Note
   - `Colour` → Colour
   - `End action` → End action
   - `Timer type` → Timer type
   - `Time warning` → Time warning
   - `Time danger` → Time danger
   - `Presenter` → Custom field
4. Click **Import**

You should now see all 8 events in the Ontime Editor!

### Step 3.9 — Enable bidirectional sync (optional)

If you want changes in Ontime to write back to the Google Sheet:
1. In Sheet Sync settings, enable **"Sync changes back to sheet"**
2. This lets you edit the schedule from either Ontime OR the Google Sheet

---

## Part 4: Run the Show (April 26)

### Step 4.1 — Start Ontime

```bash
# If using CLI
ontime

# If using desktop app, just open it
```

### Step 4.2 — Open the views you need

Open these in separate browser tabs/windows:

| View | URL | Who / Purpose |
|------|-----|---------------|
| **Editor** | `http://localhost:4001/editor` | YOU — control the show |
| **Stage Timer** | `http://localhost:4001/timer` | Projector/screen for presenter — shows countdown |
| **Backstage** | `http://localhost:4001/backstage` | Backstage monitor — shows full schedule |
| **Timeline** | `http://localhost:4001/timeline` | Visual timeline overview |
| **Cuesheet** | `http://localhost:4001/cuesheet` | Collaborative view for team |
| **Countdown** | `http://localhost:4001/countdown` | Simple countdown to next event |

### Step 4.3 — Display on external screens

If you have external monitors/projectors:
1. Open the desired view URL on the target display
2. Press **F11** for fullscreen (or use browser fullscreen)
3. Each view URL can be customized with URL parameters

### Step 4.4 — Run the show

1. At **09:30**, go to the Editor
2. Click **▶ Play** on the first event ("Registration")
3. Ontime starts the countdown
4. When "Registration" ends → it auto-loads "Opening" (because `load-next`)
5. Click **▶ Play** to start the next talk
6. Repeat through the day

**Tip**: If running late, add a **Delay entry** in the Editor — it pushes all subsequent times forward and shows the delay on all views.

### Step 4.5 — Roll Mode (fully automatic)

If you don't want to manually press Play for each event:
1. In the Editor, enable **Roll Mode**
2. Ontime auto-tracks based on the real clock
3. It automatically advances events as time passes
4. Great for "set it and forget it" — but you lose manual control

---

## Part 5: BeThere Check-In Integration

The BeThere worker and Ontime use the **same Google Sheet but different tabs**. They operate independently:

```
Google Sheet (same SHEET_ID)
├── "checkin" tab  →  BeThere worker reads/writes attendee data
├── "staff" tab    →  BeThere worker reads staff email list
└── "ontime" tab   →  Ontime reads/writes event schedule
```

### Running both systems together

```bash
# Terminal 1: Start Ontime
ontime

# Terminal 2: Start BeThere worker (dev mode)
cd event-checkin/worker && ./deploy.sh dev
```

- **Ontime**: `http://localhost:4001` — schedule & timers
- **BeThere**: `http://localhost:8787` — check-in & staff management

### Sharing with team on the same network

Other devices on your local network can access:
- Ontime views: `http://YOUR_IP:4001/timer` (for the stage display)
- BeThere admin: `http://localhost:8787/staff` (for scanning QR codes)

> Note: BeThere on Cloudflare Workers is accessible publicly via its workers.dev URL. Only Ontime needs localhost access for the timer views.

---

## Part 6: Quick Reference

### Event Schedule (April 26, 2026)

| # | Time | Duration | Title | Speaker |
|---|------|----------|-------|---------|
| 1 | 09:30 | 30 min | Registration | — |
| 2 | 10:00 | 10 min | Opening by Solana Developer Thailand & Solana Thailand DAO | — |
| 3 | 10:10 | 50 min | Rust AI and Gaming Ep. 2 | Katopz |
| 4 | 11:00 | 10 min | Group Photo Session | — |
| 5 | 11:10 | 30 min | Hands-on: Solana Account Model & Building Your First NFT with Metaplex | Golf |
| 6 | 11:40 | 15 min | The Future of On-chain Gaming & Ephemeral Rollups | Andy (Magicblock) |
| 7 | 11:55 | 15 min | APAC Ecosystem Spotlight - Foundation Updates & Opportunities | Chaerin (Solana Foundation) |
| 8 | 12:10 | 50 min | Networking Session | — |

### Ontime URLs

```
Editor:      http://localhost:4001/editor
Timer:       http://localhost:4001/timer
Backstage:   http://localhost:4001/backstage
Timeline:    http://localhost:4001/timeline
Cuesheet:    http://localhost:4001/cuesheet
Operator:    http://localhost:4001/op
Countdown:   http://localhost:4001/countdown
Studio:      http://localhost:4001/studio
Project:     http://localhost:4001/project
```

### BeThere URLs

```
Login:       http://localhost:8787/
Scanner:     http://localhost:8787/staff
Admin:       http://localhost:8787/admin
API Health:  http://localhost:8787/api/health
```

### Troubleshooting

| Problem | Solution |
|---------|----------|
| Ontime won't start | Check if port 4001 is in use: `lsof -i :4001` |
| Google Sheet sync fails | Re-upload credentials JSON in Ontime settings |
| Schedule not importing | Check that column headers in sheet match exactly (case-insensitive) |
| Timer not counting | Make sure event has `count-down` timer type and a valid duration |
| Can't access from other devices | Ensure all devices are on the same WiFi/network |
| BeThere + Ontime port conflict | They use different ports (8787 vs 4001), no conflict |

### Files in this directory

```
event-checkin/ontime/
├── SETUP-GUIDE.md                    ← This file
└── solana-dev-thailand-26apr.csv     ← CSV backup (import fallback)
```

---

## Checklist for Tomorrow

- [ ] Ontime installed and running at `localhost:4001`
- [ ] Google Sheet has `ontime` tab with schedule data
- [ ] Google Cloud OAuth credentials created for Ontime
- [ ] Ontime connected to Google Sheet (Sheet Sync)
- [ ] Schedule imported — all 8 events visible in Editor
- [ ] Stage Timer view tested on projector/external display
- [ ] BeThere worker running (`./deploy.sh dev`)
- [ ] Staff can access BeThere scanner at `/staff`
- [ ] Timer warning colours tested (yellow at warning, red at danger)
- [ ] Roll Mode tested (optional — if using automatic mode)