# Issue 005: Growth & Marketing Plan

> **Priority**: P0 — Business Critical
> **Status**: Planning
> **Owner**: Product Growth Lead
> **Created**: 2025-07-01

## Context

BeThere is a **deposit-backed event check-in platform** on Solana. The core value proposition:

> "Put down a small deposit. Show up. Get your money back. Don't show up? Organizer keeps it."

The product is technically mature (multi-event, QR scanner, claim page, quiz-gated NFT, waitlist backend). Now we need **customers** — specifically, event organizers who will run events on BeThere.

---

## Current State

### Product Readiness
| Feature | Status |
|---------|--------|
| Landing page + waitlist | ✅ Live |
| Google OAuth login | ✅ Live |
| QR scanner (staff) | ✅ Live |
| Admin dashboard | ✅ Live |
| Multi-event management | ✅ Live |
| Quiz-gated claim | ✅ Live |
| cNFT minting (devnet) | ✅ Live |
| SOL + USDC refund | ✅ Live |
| Self-serve demo (/demo) | ❌ Missing |
| Social sharing (claim page) | ❌ Missing |
| SEO meta tags | ❌ Missing |
| Analytics tracking | ❌ Missing |
| Case study page | ❌ Missing |
| NFT artwork (production) | ❌ Missing |

### Landing Page Analysis (Current Copy)

**Hero:** "Commit. Show up. / Get your money back."
- ✅ Clear value prop — deposit/refund angle
- ✅ "Powered by Solana" pill adds credibility
- ⚠️ Could be more specific about WHO it's for

**Problem Section:** "Events have a no-show problem" (40% stat)
- ✅ Strong problem framing — organizer-pain-focused
- ✅ Three cards: paper wristbands, spreadsheets, no-shows waste money
- ✅ Clear "no accountability" narrative

**How It Works:** 4 steps — Deposit → Show Up & Scan → Quiz → Get Money Back + Badge
- ✅ Comprehensive
- ⚠️ 4 steps feels like a lot — could simplify perception

**Organizer/Attendee Split:** Two cards with clear CTAs
- ✅ Good dual-audience targeting
- ✅ Organizer CTA = "Start Your Event"

**Waitlist:** "Ready to end no-shows?" with email capture → Google Sheets
- ✅ Working backend with duplicate detection
- ✅ Clear value exchange

**Social Proof:** "Alpha · Building with Solana Developer Thailand"
- ✅ Has initial credibility signal
- ⚠️ Only one partner shown

**Footer:** 3 columns (Brand, Product, Community) + Solana logo
- ✅ Professional
- ✅ Links to X, GitHub, Source Code

### What's Working
1. **Deposit/refund narrative** is unique — most event tools don't tackle no-shows
2. **Dark minimal design** fits the Solana/web3 aesthetic
3. **Waitlist is functional** — can start collecting leads immediately

### What Needs Improvement
1. **No demo experience** — visitors can't TRY the product without signing up
2. **No video/GIF** — people won't read text, they need to SEE it
3. **SEO is empty** — no og:image, no meta description, no structured data
4. **No social proof beyond Solana Thailand** — need testimonials, event count, attendee count
5. **No analytics** — flying blind on conversion rates
6. **No viral loop** — claimed NFTs don't link back to BeThere

---

## Target Customers

### ICP (Ideal Customer Profile)

**Primary:** Web3/crypto event organizers in Thailand & Southeast Asia
- Running meetups, hackathons, conferences with 50-500 attendees
- Already familiar with Solana/wallets
- Pain: no-shows, manual check-in, no attendance proof
- Examples: Solana Thailand, Superteam Thailand, ETH Bangkok, web3 community meetups

**Secondary:** Global Solana ecosystem events
- Breakpoint side events, Solana hacker houses, regional meetups
- Found via Discord, Twitter, Superteam DAO

**Future (NOT NOW):** Web2 event organizers, universities, corporate events

### Buyer vs User

| Persona | Role | Motivation |
|---------|------|------------|
| Event Organizer | **Buyer** | Reduce no-shows, automate check-in, on-chain attendance proof |
| Staff/Volunteer | **User** | Easy QR scanning, no training needed |
| Attendee | **User + Viral Channel** | Get deposit back, collect NFT badges, share on social |

---

## Growth Plan

### Phase 1: Foundation (Week 1-2) — "Make the landing page convert"

**Goal:** Turn the landing page from a brochure into a lead generation machine.

| # | Action | Impact | Effort | Deliverable |
|---|--------|--------|--------|-------------|
| 1.1 | Add SEO meta tags (og:image, description, keywords) | Medium | Low | Code change |
| 1.2 | Add Cloudflare Web Analytics snippet | Medium | Low | Code change |
| 1.3 | Create a 30-60 second demo GIF/video of the full flow | High | Medium | Asset file |
| 1.4 | Add demo GIF above the fold in hero section | High | Low | Code change |
| 1.5 | Build `/demo` self-serve demo route (devnet) | **Highest** | Medium | New page |
| 1.6 | Add "Try Demo" CTA button to hero | High | Low | Code change |
| 1.7 | Add FAQ section to landing page | Medium | Low | Code change |

**Success metrics:**
- Landing page bounce rate < 60%
- Waitlist signup rate > 5% of visitors
- Demo trial rate > 15% of visitors

### Phase 2: First 3 Customers (Week 3-6) — "Do things that don't scale"

**Goal:** Get 3 event organizers to commit to using BeThere for their next event.

| # | Action | Impact | Effort | Notes |
|---|--------|--------|--------|-------|
| 2.1 | **Solana Thailand case study** — run their next event on BeThere | **Highest** | Medium | Already have the relationship. This is #1 priority. |
| 2.2 | Collect testimonial + quote from Solana Thailand CTO | High | Low | Add to landing page |
| 2.3 | Write case study blog post: "How X Event Reduced No-Shows by Y%" | High | Medium | Publish on Mirror.xyz or dev.to |
| 2.4 | Post in 10+ Solana ecosystem Discords | High | Low | See script below |
| 2.5 | Twitter/X thread: "We built BeThere to solve event no-shows" | Medium | Low | Include demo GIF |
| 2.6 | Add Solana Thailand logo + testimonial to landing page | High | Low | Code change |
| 2.7 | Outreach to Superteam Thailand, ETH Bangkok organizers | High | Low | Direct DM |
| 2.8 | Apply to demo at Solana Breakpoint 2025 | Medium | Low | Long lead time |

**Discord outreach template:**
```
Hey! We built BeThere — a deposit-backed event check-in tool on Solana:
• Attendees put down a deposit → show up → get it back
• QR scan check-in + compressed NFT badges as proof of attendance
• Quiz-gated: attendees prove they actually paid attention
• Automated SOL + USDC refund on claim

Free for your first event. We'll help set it up.
Demo: bethere.solana-thailand.workers.dev
DM me if interested! 🎫
```

**Success metrics:**
- 3 event organizers committed
- 1 case study published
- 50+ waitlist signups
- Waitlist → committed conversion > 10%

### Phase 3: Viral Loop (Week 7-10) — "Make users sell for you"

**Goal:** Every attendee who claims an NFT becomes a distribution channel.

| # | Action | Impact | Effort | Deliverable |
|---|--------|--------|--------|-------------|
| 3.1 | Add social sharing to claim success page (Twitter/X pre-filled) | High | Low | Code change |
| 3.2 | NFT metadata includes `external_url` to BeThere landing page | High | Low | Config change |
| 3.3 | NFT metadata includes `properties.category = "BeThere Badge"` | Medium | Low | Config change |
| 3.4 | Add "Share your NFT" button on claim success | Medium | Low | Code change |
| 3.5 | Add claim counter: "42 of 50 attendees claimed" | Low | Low | Code change |
| 3.6 | Confetti animation on successful claim | Low | Low | Code change |

**Twitter share template:**
```
Just claimed my BeThere NFT at #{event_name}! 🎫✨
Showed up, proved I was there, got my deposit back. On-chain.
#Solana #BeThere
```

**Success metrics:**
- NFT claim rate > 40% of checked-in attendees
- Social share rate > 20% of claimers
- Inbound organizer inquiries from social shares > 2/week

### Phase 4: Scale (Month 3+) — "Build the machine"

| # | Action | Impact | Effort |
|---|--------|--------|--------|
| 4.1 | Submit to Solana ecosystem directory (solana.com/ecosystem) | Medium | Low |
| 4.2 | List on awesome-solana GitHub repo | Low | Low |
| 4.3 | Submit to DappRadar, SolanaFM | Low | Low |
| 4.4 | Publish recurring blog posts (Mirror.xyz) | Medium | Medium |
| 4.5 | Launch referral program for organizers | High | Medium |
| 4.6 | Create "BeThere Verified" trust badge | Medium | Medium |
| 4.7 | Paid promotion (sponsored Solana newsletter/_podcast) | Medium | Medium |
| 4.8 | Integrate with Luma/Eventbrite (CSV import) | High | High |
| 4.9 | Multi-language support (Thai, Vietnamese) | Medium | Medium |

---

## Pricing Model (Launch When 5+ Events Completed)

| Tier | Price | Includes |
|------|-------|----------|
| **Free Beta** | $0 | Everything, unlimited — collecting feedback |
| **Event** | $49/event | Up to 500 attendees, mainnet cNFTs, refund automation, support |
| **Organizer** | $199/mo | Unlimited events, 2000 attendees/mo, priority support, custom NFT art |
| **Enterprise** | Custom | White-label, integrations, SLA, multi-chain |

**Unit economics:** cNFT = ~$0.001, SOL transfer = ~$0.00025. Margin at $49/event ≈ 98%.

---

## Landing Page Copy Improvements

### Current Hero
> "Commit. Show up. / Get your money back."

### Proposed Hero (A/B test options)

**Option A (Organizer-focused):**
> "Stop losing money to no-shows."
> "Deposit-backed event check-in on Solana. Attendees commit money, show up, get refunded. Simple."

**Option B (Benefit-focused):**
> "Check in with QR. Mint proof on-chain. Refund automatically."
> "The event platform where showing up literally pays off."

**Option C (Keep current, add subtitle):**
> "Commit. Show up. Get your money back."
> "Deposit-backed event check-in with NFT badges. Built on Solana."

### Recommended CTA Changes
| Current | Proposed | Reason |
|---------|----------|--------|
| "Get Started" → /login | "Try Free Demo" → /demo | Lower friction |
| "How It Works" → #how-it-works | Keep as-is | Good anchor |
| — | Add "Watch 60s Demo" → video | Visual learners |

---

## Key Metrics Dashboard

### Acquisition
| Metric | Tool | Target (M3) |
|--------|------|-------------|
| Landing page visitors | Cloudflare Analytics | 1,000+/mo |
| Demo page visits | Cloudflare Analytics | 200+/mo |
| Waitlist signups | Google Sheets count | 50+ total |
| Demo → Event creation | Custom event tracking | 10%+ |

### Activation
| Metric | Tool | Target |
|--------|------|---------|
| Events created | KV store count | 5+ |
| Attendees checked in | Google Sheets count | 500+ |
| NFT claim rate | claim API logs | >40% |

### Revenue (Future)
| Metric | Tool | Target |
|--------|------|--------|
| Paying organizers | Manual tracking | 3+ |
| MRR | Manual tracking | $150+ |
| Per-event revenue | Calculation | $49 avg |

### Referral
| Metric | Tool | Target |
|--------|------|--------|
| Social shares from claims | Custom tracking | 20%+ of claims |
| Organizer referrals | UTM tracking | 0.3+ viral coefficient |
| Inbound from social | UTM tracking | 5+/mo |

---

## Implementation Priority Order

Code changes ranked by growth impact:

| Priority | Item | Issue Ref |
|----------|------|-----------|
| 🔴 P0 | `/demo` self-serve demo route | New page |
| 🔴 P0 | SEO meta tags (og:image, description) | Landing page edit |
| 🔴 P0 | Demo GIF/video embedded in hero | Landing page edit |
| 🔴 P0 | "Try Demo" CTA button in hero | Landing page edit |
| 🟡 P1 | Cloudflare Web Analytics | Config change |
| 🟡 P1 | Social sharing on claim success page | Claim page edit |
| 🟡 P1 | NFT metadata with BeThere external_url | Config change |
| 🟡 P1 | FAQ section on landing page | Landing page edit |
| 🟡 P1 | Testimonial/social proof from case study | Landing page edit |
| 🟢 P2 | Confetti animation on claim | Claim page edit |
| 🟢 P2 | Claim counter display | Claim page edit |
| 🟢 P2 | "BeThere Verified" badge system | New feature |
| 🟢 P2 | Referral program (UTM + reward) | New feature |

---

## Open Questions

| # | Question | Options | Default |
|---|----------|---------|---------|
| Q1 | NFT artwork for first real event? | AI-generated / Designer / Template | Designer |
| Q2 | Mainnet or devnet for first external customer? | Mainnet (real) / Devnet (safe) | Mainnet after case study |
| Q3 | Analytics: Cloudflare Web Analytics or custom? | CF Analytics / PostHog / Plausible | CF Analytics (free) |
| Q4 | Blog platform? | Mirror.xyz / dev.to / Custom / Hashnode | Mirror.xyz (web3 native) |
| Q5 | Domain: keep workers.dev or get bethere.app? | workers.dev / Custom domain | Custom domain (later) |

---

## References

- Landing page code: `frontend-leptos/src/pages/landing.rs`
- Waitlist handler: `worker/src/handlers/waitlist.rs`
- Design brief: `.design/brief.md`
- Architecture discussion: `DISCUSSION.md`
- Handover 019 (landing page): `.handovers/019_landing_page.md`
