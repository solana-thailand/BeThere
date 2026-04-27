# BeThere — UI/UX Design Brief

> Design mockups for stakeholder review. Keep the existing dark-mode + gradient vibe.
> Output: high-fidelity mockups for 4 core flows, mobile-first where noted.

---

## 1. Design System (Existing — Keep This Vibe)

### Colors
| Token | Value | Usage |
|-------|-------|-------|
| `--bg-primary` | `#0f0f0f` | Page background |
| `--bg-secondary` | `#1a1a1a` | Cards, sections |
| `--bg-tertiary` | `#242424` | Elevated surfaces |
| `--bg-card` | `#1e1e1e` | Card backgrounds |
| `--bg-hover` | `#2a2a2a` | Hover states |
| `--text-primary` | `#e0e0e0` | Body text |
| `--text-secondary` | `#999` | Descriptions |
| `--text-muted` | `#666` | Hint text |
| `--accent` | `#6366f1` | Primary brand (indigo) |
| `--accent-hover` | `#818cf8` | Hover accent |
| `--success` | `#22c55e` | Check-in success |
| `--warning` | `#f59e0b` | Pending states |
| `--danger` | `#ef4444` | Errors |
| `--info` | `#3b82f6` | Info, locked wallet |
| `--border` | `#2a2a2a` | Subtle borders |

### Gradient (Brand Mark)
```
linear-gradient(135deg, #818cf8 0%, #6366f1 40%, #a78bfa 100%)
```
Used for: logo text, hero headlines, step number circles, brand accent.

### Typography
- Font: System stack (`-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif`)
- H1: `clamp(1.75rem, 5vw, 2.75rem)`, weight 800
- H2: `1.5rem`, weight 700
- Body: `0.9–1.1rem`, weight 400
- Labels: `0.75–0.85rem`, weight 600

### Spacing & Radius
- Border radius: `10px` (cards), `6px` (badges, small elements)
- Container max: `480px` (mobile), `960px` (landing)
- Card padding: `1.5–2rem`
- Section padding: `3–5rem` vertical

### Components
- **Cards**: `bg-card` + `border` + `radius` + subtle `box-shadow`
- **Buttons**: Primary (accent bg), Outline (border + text), Success (green), Danger (red)
- **Badges**: `border-radius: 9999px`, colored bg + border
- **Tabs**: Underline style, active = accent border-bottom

### Mood
Dark, minimal, tech-forward. Think: Linear meets Solana Explorer. Indigo/purple gradients as the signature accent. Emoji used sparingly as visual anchors (🎫 ✨ 🎪). Clean whitespace. No illustrations or photos — just typography, cards, and color.

---

## 2. Flow 1 — Landing Page (Desktop-first, 960px max)

### Current State
Functional but basic. Sections: nav → hero → problem → how-it-works → organizer/attendee CTAs → footer.

### What to Improve
- Hero needs more visual punch — consider an animated gradient orb, particle effect, or abstract Solana logo integration
- Problem section feels generic — needs stronger visual contrast
- How-it-works steps need connecting visual flow (dotted line, arrow, or timeline)
- Missing social proof / credibility section (event count, attendee count logos)
- Missing CTA for "Already have a claim link?" attendee entry point
- Footer needs more personality

### Mockup Requirements

**Viewport**: Desktop (1440px width, 960px content), then mobile (375px)

**Sections (top to bottom)**:

1. **Sticky Nav**
   - Logo "BeThere" (gradient text) left
   - "Sign In" button right (outline style)
   - Backdrop blur, subtle border-bottom

2. **Hero Section**
   - Large gradient orb/glow behind the headline (subtle, not overpowering)
   - Emoji ticket icon (🎫) or animated Solana-inspired graphic
   - H1: "Check in. Mint." (line break) gradient: "Prove you were there."
   - Subtitle: "Solana-powered event check-ins with compressed NFTs as proof of attendance."
   - Two CTAs: "Get Started" (primary), "How It Works" (outline)
   - Optional: small attendee claim link entry ("Have a claim link? Paste it here →")

3. **Problem Section**
   - H2: "Attendance tracking is broken"
   - Three cards with emoji icons (📋 Paper Wristbands, 📊 Spreadsheets, 🤷 No Lasting Proof)
   - Each card has subtle red/orange warning accent (problem = danger/warning tone)

4. **How It Works**
   - H2: "How it works" — subtitle: "Three steps. Under a minute."
   - Three step cards with numbered circles (1→2→3), each with gradient accent:
     - Step 1: Register Event (indigo circle)
     - Step 2: Scan & Check In (green circle)
     - Step 3: Claim NFT (purple circle)
   - **Key improvement**: Connect the steps with a dotted/gradient line or arrows

5. **For Organizers / For Attendees**
   - Two side-by-side cards
   - Organizer card: 🎪 icon, description, "Start Your Event" CTA
   - Attendee card: ✨ icon, description, "Learn More" CTA
   - Consider adding a feature list with checkmarks

6. **Social Proof Section (NEW)**
   - "Trusted by X events" or placeholder event logos
   - Or a testimonial quote card

7. **Footer**
   - "BeThere × Solana Thailand" left
   - GitHub link + "Powered by Solana" right
   - Consider adding: Twitter/X link, Discord link

### States to Show
- Desktop (1440px)
- Mobile (375px) — stacked layout, hamburger nav optional

---

## 3. Flow 2 — Claim Page (Mobile-first, 480px max)

### Current State
Functional. States: Loading → NotFound → Ready → Minting → Success → AlreadyClaimed → MintError. Has wallet match guard (locked wallet indicator). Pre-fills wallet when locked.

### What to Improve
- Loading state needs skeleton/animation, not just spinner
- Ready state: wallet input area feels sparse, needs visual hierarchy
- Locked wallet indicator could be more prominent
- Success state needs celebration — confetti, checkmark animation, share button
- NFT preview card should show placeholder artwork
- Already claimed state needs to show the NFT asset (or link to explorer)
- Overall: more delight moments

### Mockup Requirements

**Viewport**: Mobile (375px width, 480px content max)

**States to mock (one screen each)**:

1. **Loading**
   - Skeleton cards with shimmer animation
   - BeThere logo (gradient) centered
   - Pulsing dots or loading bar

2. **Ready — Wallet Input**
   - Welcome card: attendee name, pixel avatar, checked-in timestamp
   - "Claim your NFT" heading
   - Wallet address input with paste button
   - **Locked state variant**: Pre-filled input, "Locked 🔒" badge, info border
   - **Unlocked state variant**: Empty input, "Paste your Solana address" hint
   - NFT preview card (placeholder image with gradient border)
   - "Claim NFT" primary button (full width)

3. **Minting (In Progress)**
   - Same layout as Ready but button shows spinner + "Minting..."
   - Progress indication (indeterminate bar or Solana logo spinning)
   - "This may take a few seconds" helper text

4. **Success** 🎉
   - Large animated checkmark (green circle + white check SVG)
   - H2: "You're in! NFT Claimed"
   - Asset ID (monospace, truncated with copy button)
   - Link to Solana Explorer / Solscan
   - **NEW**: "Share your NFT" button (opens share sheet)
   - "View on Explorer" link
   - Confetti animation or particle burst (subtle)

5. **Already Claimed**
   - Warning card with ⚠️ icon
   - "This NFT has already been claimed"
   - Show claim timestamp
   - Link to Solana Explorer for the asset

6. **Error**
   - Red-tinted error card
   - Clear error message
   - "Try Again" button
   - Wallet mismatch variant: shows masked wallet hint ("Locked to BxRW...3KjF")

### Key Interactions
- Paste button reads from clipboard
- Wallet input validates on blur (length 32–44, base58 chars)
- Locked wallet: input is read-only, shows lock icon

---

## 4. Flow 3 — Scanner Flow (Mobile-only, staff-facing)

### Current State
Two tabs: Scanner (camera) and Manual (text input). Camera uses BarcodeDetector + jsQR fallback. Shows result panel after scan.

### What to Improve
- Camera viewfinder needs better framing guides (corners, scanning area highlight)
- Result panel after scan needs clearer success/error states
- Manual tab feels disconnected — should be a fallback, not equal tab
- Needs haptic/visual feedback on successful scan (flash, vibration)
- Staff identity confirmation is weak
- Check-in counter ("X checked in today") should be prominent
- Recent scan history (last 3-5 scans)

### Mockup Requirements

**Viewport**: Mobile (375px width, full screen)

**States to mock**:

1. **Scanner — Ready (Camera Active)**
   - Full-screen camera view
   - Top bar: "← Back" | "Scanner" | staff avatar/initials
   - Center: viewfinder frame with animated scan line (green gradient)
     - Four corner brackets (like a QR scanner overlay)
     - Semi-transparent overlay outside scan area
   - Bottom panel (slide-up card):
     - "Point camera at QR code" instruction
     - Today's check-in count: "12 checked in" (green badge)
     - "Enter manually" link (switches to Manual tab)
     - Recent scans: last 2-3 names with timestamps

2. **Scanner — Just Scanned (Success)**
   - Camera pauses/froze
   - Green flash overlay (brief)
   - Result card slides up over bottom panel:
     - ✅ Large green check
     - Attendee name (bold)
     - Participation type badge (In-Person / Online)
     - "Checked in!" with timestamp
     - "Scan Next" primary button
     - Auto-dismisses after 3 seconds OR tap to dismiss

3. **Scanner — Already Checked In (Warning)**
   - Same slide-up card but yellow/warning themed
   - ⚠️ Warning icon
   - "Already checked in"
   - Show original check-in time
   - "Force Re-check-in" danger button (admin only)
   - "Dismiss" outline button

4. **Scanner — Error (Not Found)**
   - Red-tinted card
   - ❌ Error icon
   - "Attendee not found"
   - "This QR code is not registered for this event"
   - "Try Again" button

5. **Manual Entry Tab**
   - Top: same header bar
   - Input field: "Enter attendee ID or email"
   - Search button
   - Results list (if multiple matches)
   - Each result: name, email, participation type, "Check In" button

6. **Scanner — Camera Denied**
   - Camera icon with slash
   - "Camera access denied"
   - "Please allow camera access in Settings"
   - "Enter Manually" CTA (switches to manual tab)

---

## 5. Flow 4 — Admin Dashboard (Desktop-first, responsive)

### Current State
Tabs: In-Person / Online. Stats grid (total, checked-in, percentage, remaining). Attendee list with search. QR generation. Recent check-ins.

### What to Improve
- Stats cards need sparkline charts or trend indicators
- Attendee list needs bulk actions (select multiple, export)
- QR generation should be inline, not a separate action
- Missing: event timeline / activity feed
- Missing: export functionality (CSV)
- Tab switching loses context — need persistent filters
- Mobile layout is cramped

### Mockup Requirements

**Viewport**: Desktop (1440px), then tablet (768px)

**Layout**:
- Sticky header with logo, nav tabs, user menu
- Two-column layout on desktop (sidebar stats + main content)
- Single column on mobile

**States to mock**:

1. **Dashboard — Overview Tab**
   - **Stats Bar** (top, horizontal):
     - Total Registered: 150 (icon: 👥)
     - Checked In: 87 (icon: ✅, green accent)
     - Check-in Rate: 58% (progress bar)
     - Remaining: 63 (icon: ⏳, warning accent)
   - **Activity Feed** (below stats):
     - Real-time check-in events: "John D. checked in — 2 min ago"
     - Last 10 events, scrollable
     - Each item: avatar, name, time, participation type badge
   - **Quick Actions** card:
     - "Open Scanner" → links to /scanner
     - "Generate QR" → inline QR generation
     - "Export CSV" → downloads attendee list

2. **Dashboard — Attendees Tab**
   - **Search bar**: "Search by name or email..."
   - **Filter pills**: All | Checked In | Not Checked In | Staff
   - **Table/List**:
     - Desktop: table with columns (Name, Email, Type, Status, Check-in Time, Actions)
     - Mobile: card list
     - Each row:
       - Pixel avatar + name
       - Email (truncated)
       - Participation badge (In-Person = green, Online = blue)
       - Status badge (Checked In ✅ / Not Yet ⏳)
       - "Generate QR" button (if not checked in)
       - "View Details" arrow
   - **Bulk Actions** (when rows selected):
     - "Generate QR for selected"
     - "Export selected"

3. **Dashboard — QR Generation**
   - Inline modal or slide-over panel
   - Search/select attendee
   - Generated QR code display (large, centered)
   - "Copy Claim Link" button
   - "Download QR Image" button
   - Force regenerate toggle (for lost/expired links)

4. **Dashboard — Settings/Event Tab (NEW)**
   - Event name, date, location (read-only, from config)
   - NFT configuration status (configured ✅ / not configured ⚠️)
   - Staff management (add/remove staff emails)
   - Event link: "Share this link with attendees: bethere.app/e/xyz"

### Mobile Variant (375px)
- Single column
- Stats as horizontal scroll cards
- Tabs become bottom navigation bar
- Attendee list = card layout
- Floating action button: "Open Scanner"

---

## 6. Cross-Cutting Design Elements

### Animations (Subtle, Not Distracting)
- Page transitions: fade-in (200ms ease)
- Card hover: slight lift + shadow increase
- Button press: scale(0.97)
- Scanner line: continuous vertical sweep
- Success state: checkmark draws in (SVG stroke animation)
- Loading: skeleton shimmer (left-to-right gradient sweep)

### Responsive Breakpoints
| Breakpoint | Target |
|-----------|--------|
| 0–359px | Small phones (minimal) |
| 360–480px | Standard phones |
| 481–767px | Large phones / small tablets |
| 768px+ | Tablets / desktop |

### Dark Mode Only
No light mode needed for now. The entire app is dark-first.

### Accessibility
- Minimum contrast ratio 4.5:1 for text
- Touch targets minimum 44px
- Focus rings on interactive elements
- Screen reader friendly semantic HTML

---

## 7. Design Prompts for Google Stitch

### Prompt 1 — Landing Page
```
Design a dark-mode landing page for "BeThere" — a Solana-powered event check-in
and NFT minting platform. Color palette: #0f0f0f background, #6366f1 indigo
accent, gradient from #818cf8 to #a78bfa. Style: minimal, tech-forward,
similar to Linear.app. Include: sticky nav with logo, hero section with gradient
headline "Check in. Mint. Prove you were there.", problem section with 3 cards,
3-step "How it Works" flow with connecting lines, organizer/attendee CTA cards,
and footer. Desktop 1440px viewport, max-width 960px content.
```

### Prompt 2 — Claim Page
```
Design a mobile-first claim page (375px) for BeThere event NFTs. Dark theme
(#0f0f0f bg, #6366f1 accent). Show 4 states: (1) Loading with skeleton cards,
(2) Ready with wallet input + locked wallet indicator, (3) Success with animated
checkmark + asset ID + share button, (4) Error with retry. Include welcome card
with attendee name, NFT preview with placeholder art, and "Claim NFT" button.
Style: clean cards, rounded corners, gradient accents.
```

### Prompt 3 — Scanner Flow
```
Design a full-screen mobile QR scanner (375px) for BeThere event check-in.
Dark theme (#0f0f0f bg, #6366f1 accent). Camera viewfinder with corner bracket
overlay and animated scan line. Bottom panel: check-in count, recent scans,
"Enter manually" link. Show result states: (1) Success — green flash, attendee
name, "Checked in!", (2) Already checked in — yellow warning, (3) Not found —
red error card. Auto-dismiss success after 3 seconds.
```

### Prompt 4 — Admin Dashboard
```
Design a desktop admin dashboard (1440px) for BeThere event management.
Dark theme (#0f0f0f bg, #6366f1 accent). Two-column layout: stats sidebar +
main content. Stats: Total Registered, Checked In (green), Rate (progress bar),
Remaining (amber). Main area: attendee table with search, filter pills
(All/Checked In/Not Checked In), pixel avatars, participation badges. Include
activity feed, quick actions (Open Scanner, Generate QR, Export CSV).
Mobile variant: single column with bottom nav.
```

---

## 8. Priority Order for Mockups

| Priority | Flow | Reason |
|----------|------|--------|
| 🔴 P0 | Claim Page | Most attendee-facing, high-impact UX |
| 🔴 P0 | Scanner Flow | Staff-facing during live events, must be bulletproof |
| 🟡 P1 | Landing Page | First impression, but not functional |
| 🟢 P2 | Admin Dashboard | Internal tool, functional is fine for now |