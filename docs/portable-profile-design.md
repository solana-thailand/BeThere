# Portable Developer Profile — Design Sketch (v1)

**Goal:** turn a BeThere attendee/developer profile into a **portable identity +
reputation** that other Solana dApps can read — so a wallet's proven attendance,
verified socials, and badges travel across the ecosystem instead of being locked
in one app. This is a *strategy* deliverable to decide on, not a committed build.

> One-line thesis: BeThere becomes the **identity/reputation layer** for Solana
> event-goers/builders. Every event a wallet attends compounds its portable rep,
> which makes the rep more valuable, which pulls in more apps. Network effect.

## The key insight: you're already halfway there

The attendance **badges are compressed NFTs on Solana**. That means the hardest
part — *portable, verifiable proof that a wallet attended event X* — **already
works today**, with zero extra work: any dApp can call the DAS API
(`getAssetsByOwner`, filtered to the BeThere collection) and see a wallet's
badges. On-chain = composable = done.

What is **not** portable yet is the **profile** (verified GitHub/Telegram, events
count, socials, wallet linkage) — it lives in D1 (`developer_profiles`). Making
*that* portable is the work.

## Two levels of portability (increasing GOAT-ness)

### Level 1 — Public read API + profile page (fast, "trust BeThere")
- `GET /api/dev/{wallet_or_handle}` → the **public** profile: verified socials
  (which platforms, not tokens), events attended (count + optionally names),
  badge asset ids, join date. JSON, cacheable.
- A shareable page: `bethere.../dev/{handle}` (human-facing, OG tags).
- **Trust model:** other apps trust BeThere's API word. Fine for low-stakes
  display ("this dev attended 12 events, GitHub verified"). Not sufficient where
  the consuming app needs to *cryptographically* trust the claim.

### Level 2 — Verifiable Credentials (trustless, the moat)
- Issue the profile claims as **Verifiable Credentials**. **Crossmint (already in
  the stack) supports credentials** — so BeThere can issue "wallet X's GitHub is
  verified" as a VC the *holder* controls and any app can verify **without
  trusting BeThere's API**.
- This is the standards-aligned, trustless version. It's what makes the identity
  a real primitive rather than a BeThere-only lookup.
- Aligns with emerging Solana identity (Solana Attestation Service, VCs) — build
  toward interop, not a silo.

## Privacy & consent (non-negotiable)

This handles verified socials + wallet↔email linkage — the exact data this
codebase's security work guards. Rules:
- **Default private.** Nothing is exposed until the developer **opts in per
  field** (share GitHub? yes/no; share email? almost never; share events? yes/no).
- **Wallet is the public key** — the profile is keyed by wallet; email is never
  in the public payload.
- **Consent is revocable** — un-share pulls it from the public API + revokes the
  VC.
- The public API must return **only** what the owner explicitly marked public.

## Proposed surface (v1, Level 1)

| Endpoint | Auth | Returns |
|---|---|---|
| `GET /api/dev/{wallet}` | public | public profile (opt-in fields only) + badge asset ids |
| `GET /dev/{handle}` (page) | public | human profile page, OG tags |
| `PATCH /api/profile/visibility` | attendee | set per-field public/private flags |

The badge list comes from DAS (already available); the profile fields come from
`developer_profiles`, filtered by the new visibility flags. New storage: a small
per-field visibility set on the profile (default all-private).

## Phasing & honest take

- **Phase 1 (spike):** the public read API + page + visibility flags (leaning on
  the already-on-chain badges). Small — days.
- **Phase 2:** Crossmint verifiable credentials for the trustless version.
- **Phase 3:** align with Solana Attestation Service / publish an interop schema.

**Honest caveat:** the *value* depends on **other apps adopting it** — that's BD /
partnerships, not code. So treat this as a **Phase-1 spike + a partnership
conversation**, not a big up-front build. The technical cost is low precisely
because the badges are already on-chain and Crossmint (credentials) is already
wired.

**Ties into the other threads:** agent-RSVP, season-pass/delegation, and portable
identity are all facets of "BeThere as Solana attendee infrastructure." A shared
identity is the connective tissue — an agent or a partner app that knows a
wallet's BeThere rep can gate perks, pre-fill RSVPs, or extend trust.

## Open questions for you
1. What's the first *consuming* app/partner? (Determines whether Level 1 is enough
   or you need Level 2 VCs on day one.)
2. What's public by default vs opt-in? (Badges are already on-chain/public;
   socials + events should be opt-in.)
3. Is this a product line, or a demo to attract partners? (Changes the polish bar.)
