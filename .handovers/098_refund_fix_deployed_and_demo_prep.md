# Handover 098: Refund Fix Deployed & Demo Prep

## What Happened

Session delivered two milestones back-to-back:

1. **Refund fix E2E-verified on devnet** — 30/30 tests passed against the real on-chain program (`C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`). The critical Step 9 (refund TX build + submit) succeeded, proving the `instruction_sysvar` account-count fix works end-to-end.
2. **Refund fix deployed to production** — `worker/deploy.sh` pushed commit `55db938` to `https://bethere.solana-thailand.workers.dev`. Production health confirmed (`status: ok`, `dev_mode: false`, 69 attendees / 5 events intact).
3. **NFT dashboard verified live** — `/api/wallet/{addr}/nfts` and `/api/wallet/leaderboard` both responding on production. Organizer wallet has 8 cNFTs ready to demo.

---

## Where is the Plan / Code / Test

| Artifact | Location |
|----------|----------|
| Fix commit | `55db938` on `develop` + `main` (and `feature/001_business_flow_audit`) |
| Fix code | `worker/src/solana_escrow/tx_builders/refund.rs`, `worker/src/solana_escrow/tx_builders/mod.rs` |
| Regression tests | `worker/src/solana_escrow/tx_builders/mod.rs` — `refund_instruction_accounts()` helper + 3 guards (count==10, instruction_sysvar@idx6, attendee signer+writable@idx0) |
| E2E script | `scripts/e2e/test_escrow_devnet.sh` (14 steps, uses `scripts/e2e/sign_and_submit.py`) |
| Deploy script | `worker/deploy.sh` (auto-handles `~/.pnp.cjs` via trap) |
| Issue tracking | `.issues/047_instruction_introspection.md` (P0 done, P3 stretch), `.issues/051_campaign_nft_rewards.md` (all phases done) |

---

## Reflection — Struggles & Solved

### Struggles
1. **`~/.pnp.cjs` stray file** broke `wrangler dev`'s esbuild bundling (Yarn PnP auto-detection walks up from cwd, finds the home-dir manifest, enforces PnP on a pnpm project). Non-invasive fixes failed (`.yarnrc.yml` with `nodeLinker: node-modules` ignored by esbuild; wrangler `alias` config can't target virtual modules).
2. **Local D1 missing migrations** — fresh `wrangler dev` SQLite had zero migrations applied; `deposit_statuses` table absent, causing deposit/refund builders to fail at the D1 query (not the TX build). Looked like a code bug, was actually local-dev state.
3. **PyNaCl missing** — `sign_and_submit.py` imports `nacl.signing`; Homebrew Python is externally-managed (PEP 668), blocking `pip install`.

### Solved
1. **PnP** — Option A (user-approved): temporarily rename `~/.pnp.cjs` → `.disabled`, run, restore. For production deploys, `deploy.sh` already does this automatically (`.bak` + trap on EXIT/INT/TERM).
2. **D1** — `npx wrangler d1 migrations apply bethere-db --local --env dev` applies all 17 migrations to the local SQLite.
3. **PyNaCl** — isolated venv at `/tmp/bethere-e2e/venv` with `pynacl` installed; run e2e with `PATH="/tmp/bethere-e2e/venv/bin:$PATH"`.

---

## Verification Evidence

### E2E (devnet, 30/30 pass)
| Step | Signature |
|------|-----------|
| Init escrow | `5PK9qx6MtueJScna6HshyypyrwuMxz257Bi3X6yLJoMUt5ry87WjjvzmZFUSmcVvMyswosJTNo8qPBzwHuX4i6EC` |
| Deposit 1 USDC | `5MbrGe5q5mQkEtbfN35BPZJKYA7xbo3vcAfSffLuU5epHePK8hqmPdtjUiAER4mcm49cYBRbkEiPyUAsVguUdLu3` |
| **Refund** | `2gL4tcBJTVXAuBVrkzqt5mZ9Ap9KrZTN3krkJQbAgjzVuCqkJUWPZqVR6eejT84uL2huDKA8ixgFsN9m1oLGs8z5` |
| Deactivate | `5uETLtmTQDS5kF7nU4U93UqJLBYumkbHwSTUUB6Jd72xMhbaC9TgyY1KCg1Gi4KwdYikKejfthYGY3DnNi6oi2tf` |
| Claim forfeited | `5npvyovS8NvhoH53nY9FnAvJRmrsVwPF1u1c981531prBaYbink5Smh3s8tE6Eci2kNVfbCxXLfcRUZ2JQTgbSdc` |
| Close + reclaim rent | `rFrkJcs5tk1YJsgaiuW5ukxGWhH1YnoYMP2wPn4z72HaVaD8xGAjuRH5AfHY441WSU22q2t7cxnKfy2bapV2mL9` |

Solscan base: `https://solscan.io/tx/<SIG>?cluster=devnet`

### Production (post-deploy)
- Health: `{"status":"ok","dev_mode":false,"cluster":"devnet","d1":{"connected":true,"counts":{"attendees":69,"events":5,...}}}`
- Frontend: HTTP 200, 71963 bytes JS served
- `/api/wallet/9Bz7.../nfts` → 8 cNFTs returned
- `/api/wallet/leaderboard` → `{"entries":[]}` (empty — see Remaining Work)

---

## Demo Wallets (Ready)

| Role | Address | State | Use in demo |
|------|---------|-------|-------------|
| Organizer | `9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN` | 1.635 SOL, **8 NFTs** | Show populated NFT dashboard + leaderboard |
| Attendee | `FViyYcq7RL4wYkUD6tGZTPj3WKCcNdzqZbVXcDKtFtWy` | 0.098 SOL, **19 USDC**, 0 NFTs | Clean wallet for deposit→check-in→refund→badge flow |
| Attendee keypair | `/tmp/bethere-escrow-e2e-attendee.json` | Persists across reboots | Import to Phantom/Backpack for demo |

**Note**: The attendee keypair is in `/tmp` — may not survive a reboot. Back up to a durable location before the pitch if relying on it.

---

## Demo Flow (Browser-Based — Requires User)

The pitch demo needs a browser with Phantom/Backpack connected to devnet. The assistant cannot perform browser interaction. Recommended sequence:

### Setup (before pitch)
1. Open Phantom/Backpack → switch network to **Devnet**
2. Import `/tmp/bethere-escrow-e2e-attendee.json` (attendee wallet, has 19 USDC)
3. Also have organizer wallet `9Bz7...` available (it has the 8 NFTs)

### Demo Sequence
1. **Landing** — open `https://bethere.solana-thailand.workers.dev` — show the deposit-backedin concept
2. **NFT Dashboard** — navigate to dev dashboard, connect organizer wallet `9Bz7...` → shows 8 cNFTs (proof of attendance history)
3. **Deposit flow** — register for an event with attendee wallet → build deposit TX → sign in Phantom → 1 USDC moves to escrow vault
4. **Check-in** — organizer marks attendee checked-in (or auto via adventure mode)
5. **Refund** — attendee claims refund → sign TX → USDC returns (this is the fix being demoed)
6. **cNFT badge** — attendee receives compressed NFT badge (gas-free via Helius)
7. **Leaderboard** — show ranking if any developers registered; otherwise explain the scoring (event NFT = 1pt, campaign NFT = 3pt)

### Pitch Talking Points
- **Problem**: Event no-shows cost organizers money; traditional deposits are clunky
- **Solution**: SOL/USDC escrow with auto-refund on check-in, forfeit on no-show
- **Differentiator**: Compressed NFTs as verifiable attendance history (no gas, on-chain proof)
- **Technical depth**: Instruction introspection hardening (Issue 047 P0), 10-account refund TX with `instruction_sysvar` at index 6 — exactly what the on-chain program requires

---

## Remaining Work

### Blocking for demo perfection
- **`mark_checked_in` builder** returned "unknown" in e2e Step 8. Did NOT block the refund (Step 9 passed regardless) and did not affect escrow economics (claim_forfeited in Step 13 also passed). But the check-in flow itself may not be perfect. Investigate `worker/src/handlers/` for the check-in TX builder if a flawless check-in demo is needed.

### Non-blocking but visible
- **Leaderboard empty** — `campaign_mints: 0` configured, no developers registered with wallets for ranking. For a populated leaderboard demo, create a campaign with `reward_type: "nft_certificate"` and a `collection_mint`, then register 2-3 developer wallets.
- **Campaign NFT classification** — without `campaign_mints` set, all NFTs score as event NFTs (1pt each). Campaign NFTs (3pt) require campaign config.

### Stretch (Issue 047 P3)
- **Atomic deposit + check-in** — single transaction instead of two. High "wow" factor for pitch if time permits.

---

## Issues Ref
- **047** `instruction_introspection.md` — P0 (instruction_sysvar hardening) DONE + deployed. P3 (atomic deposit+check-in) stretch.
- **051** `campaign_nft_rewards.md` — All 4 phases DONE + deployed. Dashboard live.

---

## How to Dev / Test

### Run local worker (for iterative dev)
```bash
# ~/.pnp.cjs must be moved aside (deploy.sh does this automatically; for manual wrangler dev, rename it)
mv ~/.pnp.cjs ~/.pnp.cjs.disabled
cd worker && npx wrangler dev --port 8787 --env dev
# Apply migrations to local D1 (first time only):
npx wrangler d1 migrations apply bethere-db --local --env dev
# When done:
mv ~/.pnp.cjs.disabled ~/.pnp.cjs
```

### Run E2E against local worker
```bash
# Start wrangler dev (above), then in another terminal:
# Ensure PyNaCl available (one-time):
python3 -m venv /tmp/bethere-e2e/venv && /tmp/bethere-e2e/venv/bin/pip install pynacl

# Run e2e with venv python on PATH:
PATH="/tmp/bethere-e2e/venv/bin:$PATH" bash scripts/e2e/test_escrow_devnet.sh
```

### Deploy to production
```bash
cd frontend-leptos && bash build.sh        # ~30s, builds Leptos WASM
cd ../worker && bash deploy.sh             # ~1-2 min, auto-handles PnP + PUT API fallback
# Verify:
curl -s https://bethere.solana-thailand.workers.dev/api/health | python3 -m json.tool
```

### Unit tests (regression guards)
```bash
cargo test -p event-checkin-worker --quiet
# Expect: 84 + 15 + 21 = 120 tests pass (incl. 3 refund regression guards)
```

---

## Environment Notes

- **`~/.pnp.cjs`**: Stray Yarn PnP manifest at home dir (426KB, March 2025, nothing references it). Breaks wrangler's esbuild. `deploy.sh` handles via move/restore trap. For manual `wrangler dev`, rename temporarily.
- **macOS M5 Pro tooling**: `rg`, `eza`, `bat`, `fd`, `procs` (Rust-powered CLI replacements — see user rules).
- **Wrangler 4.99.0**: Hits `/versions` API bug (code 10013) on deploy; `deploy.sh` falls back to manual PUT API + asset JWT automatically.
- **Devnet faucet rate limits**: SOL airdrop often rate-limited. Workaround: transfer SOL from organizer (default keypair `~/.config/solana/id.json`) to attendee.
- **USDC devnet**: Circle faucet only (manual, captcha) — `https://faucet.circle.com/`.