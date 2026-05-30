---
version: alpha
name: BeThere
description: >
  Deposit-backed event check-in platform on Solana.
  Dark, minimal, tech-forward. Indigo/purple gradients as the signature accent.
  Think Linear meets Solana Explorer.

colors:
  # Backgrounds
  primary: "#13131b"
  bg-primary: "#13131b"
  bg-secondary: "#1a1a24"
  bg-tertiary: "#242430"
  bg-card: "#1e1e2a"
  bg-hover: "#2a2a38"
  bg-void: "#000000"

  # Text
  text-primary: "#e0e0e0"
  text-secondary: "#999999"
  text-muted: "#666666"
  text-heading: "#ffffff"
  text-primary-inverse: "#1a1a1a"

  # Accent (Indigo brand)
  accent: "#6366f1"
  accent-hover: "#818cf8"
  accent-light: "#a5b4fc"
  accent-purple: "#a78bfa"
  solana: "#9945ff"
  solana-green: "#14f195"
  color-x-brand: "#1da1f2"

  # Semantic
  success: "#22c55e"
  success-bright: "#4ade80"
  success-btn: "#15803d"
  success-btn-hover: "#166534"
  success-bg: "#1a2e1f"
  success-border: "#2d5a3a"
  warning: "#f59e0b"
  warning-bright: "#eab308"
  warning-dark: "#d97706"
  warning-bg: "#2e2610"
  warning-border: "#5a4520"
  danger: "#ef4444"
  danger-bright: "#f87171"
  danger-btn: "#b91c1c"
  danger-btn-hover: "#991b1b"
  danger-bg: "#2e1618"
  danger-border: "#5a2528"
  info: "#3b82f6"
  info-bright: "#60a5fa"
  info-bg: "#1a2230"
  info-border: "#2d4060"

  # Borders
  border: "#2a2a2a"
  border-hover: "#333333"

  # Code / Special
  code-bg: "#0d1117"
  code-text: "#c9d1d9"

  # Google OAuth
  bg-google-hover: "#f5f5f5"

typography:
  h1:
    fontFamily: Inter
    fontSize: 2.75rem
    fontWeight: "700"
    lineHeight: "1.2"
  h2:
    fontFamily: Inter
    fontSize: 1.25rem
    fontWeight: "600"
    lineHeight: "1.3"
  h3:
    fontFamily: Inter
    fontSize: 1rem
    fontWeight: "600"
    lineHeight: "1.4"
  body-md:
    fontFamily: Inter
    fontSize: 0.95rem
    fontWeight: "400"
    lineHeight: "1.6"
  body-sm:
    fontFamily: Inter
    fontSize: 0.9rem
    fontWeight: "400"
    lineHeight: "1.5"
  label-caps:
    fontFamily: Inter
    fontSize: 0.75rem
    fontWeight: "600"
    letterSpacing: "0.08em"
  text-sm:
    fontFamily: Inter
    fontSize: 0.8rem
    fontWeight: "400"
    lineHeight: "1.4"
  text-xs:
    fontFamily: Inter
    fontSize: 0.7rem
    fontWeight: "400"
    lineHeight: "1.3"
  brand-logo:
    fontFamily: Inter
    fontSize: 1.5rem
    fontWeight: "800"
    letterSpacing: "0.08em"
  brand-sub:
    fontFamily: Inter
    fontSize: 0.7rem
    fontWeight: "500"
    letterSpacing: "0.15em"

rounded:
  sm: 6px
  md: 10px
  pill: 9999px

spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 32px
  2xl: 48px

components:
  button-primary:
    backgroundColor: "#5558e6"
    textColor: "#ffffff"
    rounded: "{rounded.sm}"
    padding: "0.7rem 1.25rem"
    typography: "{typography.body-sm}"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
  button-outline:
    backgroundColor: transparent
    textColor: "{colors.text-primary}"
    rounded: "{rounded.sm}"
    padding: "0.7rem 1.25rem"
  button-outline-hover:
    backgroundColor: "{colors.bg-hover}"
  button-success:
    backgroundColor: "{colors.success-btn}"
    textColor: "#ffffff"
    rounded: "{rounded.sm}"
  button-danger:
    backgroundColor: "{colors.danger-btn}"
    textColor: "#ffffff"
    rounded: "{rounded.sm}"
  button-google:
    backgroundColor: "{colors.text-heading}"
    textColor: "{colors.text-primary-inverse}"
    rounded: "{rounded.md}"
    padding: "0.85rem 1.75rem"
  card:
    backgroundColor: "{colors.bg-card}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.md}"
    padding: "1.25rem"
  card-hover:
    backgroundColor: "{colors.bg-card}"
  badge-success:
    backgroundColor: "{colors.success-bg}"
    textColor: "{colors.success}"
    rounded: "{rounded.pill}"
  badge-warning:
    backgroundColor: "{colors.warning-bg}"
    textColor: "{colors.warning}"
    rounded: "{rounded.pill}"
  badge-danger:
    backgroundColor: "{colors.danger-bg}"
    textColor: "{colors.danger-bright}"
    rounded: "{rounded.pill}"
  badge-info:
    backgroundColor: "{colors.info-bg}"
    textColor: "{colors.info-bright}"
    rounded: "{rounded.pill}"
  stat-card:
    backgroundColor: "{colors.bg-card}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.md}"
    padding: "1rem"
  header:
    backgroundColor: "{colors.bg-primary}"
    height: "80px"
  brand-logo-text:
    textColor: "{colors.accent-hover}"
  input:
    backgroundColor: "{colors.bg-tertiary}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.sm}"
  nft-preview:
    backgroundColor: "{colors.bg-tertiary}"
    rounded: "{rounded.md}"
  tab:
    textColor: "{colors.text-secondary}"
  tab-active:
    textColor: "{colors.text-heading}"

---

## Overview

**BeThere** is a deposit-backed event check-in platform on Solana. The UI is dark, minimal, and tech-forward — inspired by Linear.app and Solana Explorer. The signature element is an indigo-to-purple gradient used for brand marks, hero headlines, and primary CTAs. No illustrations or photos — just typography, cards, and color. Emoji used sparingly as visual anchors.

Dark-mode only. No light mode.

## Colors

The palette is built on cool-toned dark backgrounds with high-contrast neutrals and an indigo accent system.

- **Backgrounds**: Deep blue-black (`#13131b`) with subtle blue undertones — not pure black. Cards and surfaces layer progressively lighter (`#1a1a24` → `#1e1e2a` → `#242430`).
- **Text**: Off-white (`#e0e0e0`) for body, pure white (`#ffffff`) for headings. Muted text at `#999` and `#666`.
- **Accent (Indigo)**: `#6366f1` is the primary action color. Used for buttons, links, active states, and brand gradients. Hover shifts to `#818cf8`.
- **Semantic**: Green (`#22c55e`) for success/check-in, Amber (`#f59e0b`) for warnings/pending, Red (`#ef4444`) for errors, Blue (`#3b82f6`) for info. Each has matching translucent `bg` and `border` variants. Button variants use darker shades (`--success-btn: #15803d`, `--danger-btn: #b91c1c`) to meet WCAG AA contrast. Badge text uses brighter variants (`--danger-bright: #f87171`, `--info-bright: #60a5fa`) for AA compliance on translucent backgrounds. Warning has `--warning-bright: #eab308` (VIP badges, pending status) and `--warning-dark: #d97706` (button backgrounds).
- **Solana Purple**: `#9945ff` reserved for Solana-specific branding elements. `--solana-green: #14f195` for Solana-specific green accents.
- **X/Twitter Brand**: `--color-x-brand: #1da1f2` for share-to-X components.
- **Code**: `--code-bg: #0d1117`, `--code-text: #c9d1d9` for adventure puzzle code blocks.

## Typography

**Inter** is the sole typeface — loaded via Google Fonts with weights 400–800. The system font stack (`-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica Neue, Arial, sans-serif`) serves as fallback.

- **H1**: `clamp(1.75rem, 5vw, 2.75rem)`, weight 700 — page titles, fluid responsive sizing
- **H2**: `1.25rem`, weight 600 — section headings
- **H3**: `1rem`, weight 600 — card titles, subsection labels
- **Body**: `0.9–0.95rem`, weight 400 — readable at 1.5–1.6 line-height
- **Labels**: `0.75rem`, weight 600, `0.08em` tracking — uppercase badges and meta
- **Text SM**: `0.8rem` (`--text-sm`), weight 400 — secondary text, small labels, badge text
- **Text XS**: `0.7rem` (`--text-xs`), weight 400 — fine print, hints, mockup micro-text
- **Brand logo**: `1.5rem`, weight 800, `0.08em` tracking — rendered as gradient text

## Layout

Mobile-first responsive design with four breakpoints:

| Breakpoint | Target |
|-----------|--------|
| 0–359px | Small phones (minimal support) |
| 360–480px | Standard phones (primary target for claim + scanner) |
| 481–767px | Large phones / small tablets |
| 768px+ | Tablets / desktop |

- **Container max**: `480px` (mobile flows), `960px` (landing page)
- **Card padding**: `1.25rem`
- **Header**: Sticky, `80px` height with blur backdrop
- **Admin**: Sidebar + content layout on desktop, bottom-sheet on mobile

## Elevation & Depth

Cards use `border: 1px solid var(--border)` with subtle `box-shadow: 0 2px 8px rgba(0,0,0,0.3)`. On hover, shadow deepens and card lifts `1px`. No heavy drop shadows.

The page background uses `linear-gradient(170deg, #13131b 0%, #14141e 40%, #111118 100%)` — not a flat color — giving subtle depth.

NFT preview cards get a gradient border (`linear-gradient(135deg, #6366f1, #a78bfa)`) using a `::before` pseudo-element technique.

## Shapes

Two radius levels:
- **`6px`** (`--radius-sm`): Buttons, badges, inputs, small elements
- **`10px`** (`--radius`): Cards, panels, modals
- **`9999px`**: Pills (status badges, filter chips)

## Components

### Buttons
Four variants: Primary (gradient accent), Outline (border only), Success (green), Danger (red). All use `0.2s` transitions with `translateY(-1px)` hover lift and `scale(0.97)` active press. Google OAuth button is inverted (white bg, dark text).

### Cards
Dark card bg + 1px border. Hover lifts with border color shift. Stat cards have semantic variants (success = green-tinted, warning = amber-tinted, info = blue-tinted).

### Badges
Pill-shaped with translucent colored backgrounds and matching borders. Used for status (checked-in, pending, not found).

### Tabs
Underline style. Active tab gets accent-colored bottom border. Used for scanner (Camera/Manual) and admin (In-Person/Online).

### Scanner
Full-screen camera viewfinder with corner bracket overlay and animated scan line. Bottom sheet panel for controls and results. Glass-card result overlay for scan outcomes.

### Forms
Dark input fields (`--bg-tertiary`) with subtle borders. Focus state brightens border. Inputs use `0.9rem` Inter.

## Do's and Don'ts

### Do
- Use gradient text (`linear-gradient(135deg, #818cf8, #6366f1, #a78bfa)`) only for brand marks and hero elements
- Keep whitespace generous — cards need breathing room
- Use emoji sparingly as visual anchors (🎫 ✨ 🎪 ✅ ⚠️)
- Maintain minimum 4.5:1 contrast ratio for all text
- Use `Inter` font exclusively
- Keep animations subtle: 200ms transitions, 1px lifts

### Don't
- Don't add illustrations or photos — the system is typography + cards + color only
- Don't use pure black (`#000`) for backgrounds — use `#13131b` for the blue undertone
- Don't introduce new accent colors — indigo is the brand
- Don't use box-shadow for emphasis beyond `0 2px 8px rgba(0,0,0,0.3)`
- Don't create a light mode — dark only
- Don't mix font families — Inter only
