# PDA Schema Migration Strategy — BeThere Escrow Program

> Strategy document for safely evolving on-chain account structures after mainnet deployment.
> Status: **Pre-mainnet** — this is the migration blueprint before any accounts exist in production.

---

## 1. Executive Summary

Solana accounts are **binary blobs**. Once a program is deployed and PDAs are created on mainnet, the bytes written to those accounts are laid out in a specific order dictated by the Rust struct. If you change the struct — reorder fields, change types, add fields in the middle — the program will reinterpret existing bytes incorrectly. **Data corruption, not a compiler error.**

This is fundamentally different from off-chain databases where you can `ALTER TABLE ADD COLUMN` and move on. On Solana:

- **No schema registry.** The struct layout IS the schema.
- **No runtime migration.** You can't run a background job to reformat 10,000 accounts.
- **PDA addresses are deterministic.** Seeds produce a fixed address. Changing seeds = different address = orphaned accounts.

BeThere is in a **privileged position**: the program is on devnet, not mainnet. We can design the migration strategy *before* any production accounts exist. This document exists so we never face the "how do I add a field without breaking everything" question under pressure.

### Why This Matters Now

- The escrow program has **zero** version fields, **zero** migration logic, **zero** upgrade path.
- Once mainnet PDAs are created, changing the struct layout without a plan = corrupted state.
- BeThere events are **ephemeral** (accounts get closed after settlement), which is a major advantage — but we still need a strategy for in-flight events during an upgrade.

---

## 2. Current State

### Program Identity

| Property | Value |
|----------|-------|
| Program ID | `2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo` |
| Framework | Quasar (`quasar-lang`) |
| Cluster | Devnet |
| Discriminator type | Single byte (`#[account(discriminator = N)]`) |
| Version field | **None** |
| Migration logic | **None** |
| Upgrade authority | Program deployer (set at deployment) |

### PDA Type 1: EventEscrow

| Property | Detail |
|----------|--------|
| Discriminator | `1` |
| Seeds | `["escrow", organizer: Address, event_id: u64]` |
| Account size | v0: 149 bytes (1 disc + 148 fields), v1: 192 bytes (1 disc + 1 ver + 148 fields + 36 pad) |

```
Byte Layout:
┌──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
│ disc (1) │ org (32) │ eid (8)  │ mint(32) │ vault(32)│ dep (8)  │ end (8)  │ refdl(8) │ tdep (8) │ tref (8) │ tfor (8) │ active(1)│ bump (1) │
└──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘
  0         1         33        41        73        105       113       121       129       137       145       153       155       156

Note: Quasar discriminator is 1 byte, not the 8-byte Anchor discriminator.
```

Field inventory:

| # | Field | Type | Size | Mutable | Notes |
|---|-------|------|------|---------|-------|
| 1 | `organizer` | Address | 32B | No | PDA seed, set at creation |
| 2 | `event_id` | u64 | 8B | No | PDA seed, set at creation |
| 3 | `usdc_mint` | Address | 32B | No | Cluster-specific |
| 4 | `vault` | Address | 32B | No | Token account for USDC |
| 5 | `deposit_amount` | u64 | 8B | No | Immutable after first deposit |
| 6 | `event_end` | i64 | 8B | No | Set at creation |
| 7 | `refund_deadline` | i64 | 8B | No | Set at creation |
| 8 | `total_deposited` | u64 | 8B | Yes | Running counter |
| 9 | `total_refunded` | u64 | 8B | Yes | Running counter |
| 10 | `total_forfeited` | u64 | 8B | Yes | Running counter |
| 11 | `is_active` | bool | 1B | Yes | `deactivate_event` sets false |
| 12 | `bump` | u8 | 1B | No | PDA nonce |

### PDA Type 2: AttendeeDeposit

| Property | Detail |
|----------|--------|
| Discriminator | `2` |
| Seeds | `["deposit", event: Address, attendee: Address]` |
| Account size | v0: 84 bytes (1 discriminator + 83 data), v1: 96 bytes (1 disc + 1 ver + 82 fields + 1 bump + 11 pad) |

```
Byte Layout:
┌──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
│ disc (1) │ att (32) │ evt (32) │ amt (8)  │ depat(8) │ chk (1)  │ ref (1)  │ bump(1)  │          │
└──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘
  0         1         33        65        73        81        89        90        91        83

Note: Discriminator is 1 byte. Total data = 75 bytes + 8 byte discriminator prefix = 83 bytes.
```

Field inventory:

| # | Field | Type | Size | Mutable | Notes |
|---|-------|------|------|---------|-------|
| 1 | `attendee` | Address | 32B | No | PDA seed |
| 2 | `event` | Address | 32B | No | PDA seed, links to EventEscrow |
| 3 | `amount` | u64 | 8B | No | Set at deposit |
| 4 | `deposited_at` | i64 | 8B | No | Timestamp |
| 5 | `checked_in` | bool | 1B | Yes | `mark_checked_in` |
| 6 | `refunded` | bool | 1B | Yes | `refund` |
| 7 | `bump` | u8 | 1B | No | PDA nonce |

### Instruction Set

| Discriminator | Instruction | Signer | Creates/Modifies |
|---------------|------------|--------|-----------------|
| 0 | `create_event` | Organizer | EventEscrow |
| 1 | `deposit` | Attendee | AttendeeDeposit |
| 2 | `mark_checked_in` | Organizer | AttendeeDeposit |
| 3 | `refund` | Attendee | AttendeeDeposit |
| 4 | `claim_forfeited` | Organizer | EventEscrow |
| 5 | `close_event` | Organizer | EventEscrow (closed) |
| 6 | `deactivate_event` | Organizer | EventEscrow |
| 7 | `close_deposit` | Attendee | AttendeeDeposit (closed) |

### What's Missing

- **No `version` field** on either account type.
- **No `padding` or reserved space** for future fields.
- **No migration instructions** in the instruction set.
- **No account size overhead** — accounts are exactly the struct size.

---

## 3. Migration Scenarios

### Risk Classification

| Risk | Description | Can we recover? | Likelihood |
|------|------------|----------------|------------|
| 🟢 **Low** | Append-only field additions | Yes, natural lifecycle | High |
| 🟡 **Medium** | Semantic changes to existing fields | Yes, with coordination | Medium |
| 🟠 **High** | Removing/reordering fields | Difficult, requires migration | Low |
| 🔴 **Critical** | Changing PDA seeds or program ID | New accounts required | Very low |

### Scenario 1: Adding New Fields (Non-Breaking)

**Example:** Add `platform_fee_bps: u16` to `EventEscrow`.

```
Before:  [disc][org][eid][mint][vault][dep][end][refdl][tdep][tref][tfor][active][bump]
After:   [disc][org][eid][mint][vault][dep][end][refdl][tdep][tref][tfor][active][bump][fee_bps]
```

**Impact:**
- Existing accounts are **too small** to hold the new field.
- Old accounts can still be read if the program handles `version == 0` (missing field = default).
- New accounts get the full size.
- **Must use `Resize`** (Solana's `SystemProgram::transfer` for additional rent) to grow existing accounts, or close old accounts and let new ones be created with the correct size.

**BeThere advantage:** Events are ephemeral. If a new field is added, existing in-flight events can be **grandfathered** — they operate under old rules, new events get the new field. Once old events close naturally, no migration needed.

### Scenario 2: Adding New Fields (Breaking — Middle Insertion)

**Example:** Insert `max_attendees: u32` after `deposit_amount`.

```
Before:  [disc][org][eid][mint][vault][dep][end][refdl][tdep][tref][tfor][active][bump]
After:   [disc][org][eid][mint][vault][dep][max][end][refdl][tdep][tref][tfor][active][bump]
```

**Impact:**
- Every byte after the insertion point is **shifted**. Existing accounts are **corrupted**.
- This is a **breaking change** — cannot deploy without migrating all existing accounts.
- Requires a migration instruction that reads old format, reallocates with new size, writes new format.

**Avoid if at all possible.** Append-only is always safer.

### Scenario 3: Changing Field Semantics

**Example:** Repurpose `is_active: bool` as a status enum `status: u8`.

**Impact:**
- If the binary representation maps cleanly (bool → u8, same byte), it's a **type widening** — potentially non-breaking.
- If semantics change (e.g., `bool` was `true/false`, now `u8` is `0/1/2/3`), the program must handle both old and new interpretations.
- Requires a `version` field to distinguish old vs new behavior.

### Scenario 4: Changing PDA Seeds

**Example:** Change EventEscrow seeds from `["escrow", organizer, event_id]` to `["escrow", organizer, event_id, format]`.

**Impact:**
- **Different seeds → different address.** Existing PDAs are orphaned — the new program can't find them.
- This is effectively the same as deploying a new program.
- Must either: (a) keep old seeds as a fallback, or (b) migrate all data to new PDAs.
- **Last resort.** Avoid unless architecturally unavoidable.

### Scenario 5: Changing Program ID (Nuclear Option)

**Impact:**
- Deploy an entirely new program with a new ID.
- All existing PDAs are under the old program's authority — **cannot be transferred**.
- Must either: (a) close all accounts on old program, recreate on new program, or (b) have old program CPI to new program.
- Viable if **zero mainnet accounts exist** (just redeploy). Catastrophic if active events are running.

---

## 4. Strategy A: Versioned Accounts

### Design

Add a `version: u8` field as the **first data field** (immediately after discriminator) on both account types. The version byte tells the program which struct layout to expect.

```rust
#[account(discriminator = 1, set_inner)]
#[seeds(b"escrow", organizer: Address, event_id: u64)]
pub struct EventEscrow {
    pub version: u8,          // NEW: schema version, 0 = original, 1 = first migration
    pub organizer: Address,
    pub event_id: u64,
    // ... rest of fields unchanged
    // ... future fields appended here
}

#[account(discriminator = 2, set_inner)]
#[seeds(b"deposit", event: Address, attendee: Address)]
pub struct AttendeeDeposit {
    pub version: u8,          // NEW: schema version
    pub attendee: Address,
    pub event: Address,
    // ... rest of fields unchanged
}
```

**Initialization:** All `create_event` and `deposit` instructions set `version = CURRENT_VERSION` (a constant).

**Reading:** Every instruction that deserializes an account checks `version` and reads fields accordingly:

```rust
const CURRENT_VERSION: u8 = 1;

impl EventEscrow {
    fn load(data: &[u8]) -> Result<Self> {
        let version = data[1]; // byte after discriminator
        match version {
            0 => Self::load_v0(data),
            1 => Self::load_v1(data),
            _ => return Err(ProgramError::InvalidAccountData),
        }
    }
}
```

### Cost

| Item | Value |
|------|-------|
| Added bytes per EventEscrow | +1 byte (version field) |
| Added bytes per AttendeeDeposit | +1 byte (version field) |
| Additional rent (EventEscrow) | ~0.00069 SOL (~$0.10 at $150/SOL) |
| Additional rent (AttendeeDeposit) | ~0.00069 SOL (~$0.10 at $150/SOL) |
| Code complexity | Medium (version dispatch in every read path) |

### Migration Process

When adding a new field:

1. Increment `CURRENT_VERSION` constant.
2. Add new field at the **end** of the struct (append-only).
3. Add a `load_vN()` method for the new version.
4. Add a `migrate_vN()` instruction (optional) to upgrade existing accounts.
5. Deploy upgraded program.

### Pros

- **In-place upgrades.** Existing accounts can be migrated without new PDAs.
- **PDA seeds unchanged.** Same addresses, same authority.
- **Gradual migration.** Old-version accounts continue working until explicitly migrated.
- **Audit trail.** Each account's version is visible on-chain.

### Cons

- **Code complexity.** Every deserialization path branches on version.
- **Discriminator conflict.** If version-0 accounts exist without the version field, reading `data[1]` returns the first byte of `organizer` instead of a version number. This is the **bootstrapping problem** — see below.
- **Size constraints.** Growing an account requires `Resize` + additional rent transfer. Not all instructions can do this (compute budget, signer requirements).
- **Testing burden.** Must test N version combinations.

### The Bootstrapping Problem

If the program is already deployed on mainnet *without* a version field, adding one is itself a breaking change. Byte offset 1 of existing accounts is the first byte of `organizer` (a random pubkey byte), not a version number.

**Solutions:**
1. **Add version before mainnet.** Best option — add it now while only devnet accounts exist.
2. **Use discriminator as version proxy.** Quasar's single-byte discriminator could encode version: `discriminator = 1` = v0, `discriminator = 11` = v1. But this changes the account identifier, which may break client-side code.
3. **Append version, don't insert.** Add version at the **end** of the struct. Old accounts simply don't have it — treat missing bytes as version 0. This avoids the bootstrapping problem but requires careful offset calculation.

### Verdict for BeThere

**Recommended before mainnet.** Adding `version: u8` now (pre-mainnet) costs almost nothing and provides maximum flexibility. The bootstrapping problem is moot since no production accounts exist. The 1-byte overhead is negligible.

---

## 5. Strategy B: Program ID Upgrade

### Design

Deploy a **new program** with a new program ID. The new program has the updated struct layout. Old accounts remain under the old program.

```
Old Program (2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo)
├── EventEscrow PDA v0 (old struct)
└── AttendeeDeposit PDA v0 (old struct)

New Program (NEW_PROGRAM_ID)
├── EventEscrow PDA v1 (new struct, same seeds = different address because program ID differs)
└── AttendeeDeposit PDA v1 (new struct)
```

**Critical detail:** PDA derivation includes the program ID. Same seeds + different program ID = different PDA address. This means the new program cannot access old PDAs.

### Migration Process

1. Deploy new program.
2. **Drain old program:** Execute `refund`, `claim_forfeited`, `close_deposit`, `close_event` on all active accounts under the old program.
3. **Recreate on new program:** New events are created on the new program.
4. **Update client code** to point to the new program ID.
5. **Optionally revoke upgrade authority** on old program.

### Timeline Considerations

| Scenario | Downtime | Data Loss |
|----------|----------|-----------|
| No active events | Zero | None (clean switch) |
| Active events in progress | Until all close naturally | None if events complete |
| Active events that need new fields | Immediate (must force-close) | Possible (unclaimed deposits) |

### Pros

- **Clean break.** No version dispatch complexity.
- **Simple code.** New program only handles one struct version.
- **Old program untouched.** No risk of corrupting existing accounts during upgrade.
- **Test in isolation.** New program can be fully tested on devnet before mainnet.

### Cons

- **Cannot migrate in-flight events.** Active events must complete on the old program before switching.
- **Client update required.** All frontends, backends, and integrations must update the program ID.
- **Address change.** All stored escrow addresses in the database become invalid.
- **Communication overhead.** Organizers and attendees may see confusing UI during transition.
- **No backward compatibility.** Cannot serve old and new events simultaneously without maintaining two code paths.

### Verdict for BeThere

**Reserve for breaking changes only.** This is the correct strategy if PDA seeds must change or if the struct layout is fundamentally different. For field additions, it's overkill. The fact that events are ephemeral makes this viable — wait for all events to close, then switch.

---

## 6. Strategy C: Sidecar Accounts

### Design

Create **companion PDAs** that hold extended data. The original account struct remains unchanged. Sidecar accounts are linked by reference.

```
EventEscrow (unchanged, v0 struct)
├── organizer, event_id, deposit_amount, ...

EventEscrowMeta (NEW sidecar)
├── seeds: ["escrow-meta", event_escrow_pubkey]
├── version: u8
├── max_attendees: u32
├── platform_fee_bps: u16
├── cancel_policy: u8
├── reserved: [u8; 64]  // future-proofing
```

The main account stays at its original size. The sidecar holds all "extra" data. Instructions that need the extended data simply load both accounts.

### Example Usage

```rust
#[account(discriminator = 3)]
#[seeds(b"escrow-meta", event_escrow: Address)]
pub struct EventEscrowMeta {
    pub event_escrow: Address,   // reference to main account
    pub max_attendees: u32,      // new field
    pub platform_fee_bps: u16,   // new field
    pub cancel_policy: u8,       // new field
    pub bump: u8,
}

// In instruction handler:
fn deposit(ctx: Context<Deposit>) -> Result<()> {
    let escrow = &ctx.accounts.event_escrow;
    let meta = ctx.accounts.event_escrow_meta.as_ref(); // Option<>

    let max = meta.map(|m| m.max_attendees).unwrap_or(u32::MAX);
    // ... rest of logic
}
```

### Pros

- **Zero risk to existing accounts.** Original struct is untouched.
- **Opt-in.** Sidecar accounts only exist for events that need the new fields.
- **Flexible.** Different sidecar types for different feature sets.
- **No migration.** Old events work without sidecars. New events create sidecars as needed.
- **No version dispatch.** Main account deserialization is always the same.

### Cons

- **More accounts per transaction.** Loading 2-3 accounts instead of 1 increases TX size and compute cost.
- **Account rent multiplier.** Each sidecar requires its own rent-exempt deposit (~0.002 SOL each).
- **Client complexity.** Off-chain code must know to look for sidecar accounts.
- **Orphan risk.** If a sidecar is created but the main account is closed without closing the sidecar, rent is locked.
- **Atomicity.** Operations spanning main + sidecar are atomic (same TX), but it's more accounts to manage.

### Verdict for BeThere

**Best for incremental feature additions.** When adding optional features like max capacity, platform fees, or cancellation policies, a sidecar is the lowest-risk approach. It doesn't touch existing accounts, doesn't require version dispatch, and naturally handles the "old events don't have this feature" case.

---

## 7. Recommended Approach

### The Pragmatic Minimalist Strategy

BeThere has a unique advantage: **events are ephemeral.** The account lifecycle is well-defined:

```
create → deposit (N) → check_in (N) → refund/forfeit → close
```

Most EventEscrow accounts live for days or weeks, not years. This means we can often **let old events close naturally** and only apply new struct layouts to new events. This dramatically simplifies migration.

### Recommended: Layered Approach

```
┌──────────────────────────────────────────────────────────┐
│  Layer 1: Add version field NOW (pre-mainnet)            │
│  - Append version: u8 to both structs                    │
│  - Set CURRENT_VERSION = 1                               │
│  - Zero production accounts exist — no migration needed  │
├──────────────────────────────────────────────────────────┤
│  Layer 2: Append-only field additions (ongoing)          │
│  - New fields always appended to end of struct           │
│  - Version incremented per change                        │
│  - Old-version accounts work until closed                │
│  - No middle insertions, no removals, no reordering     │
├──────────────────────────────────────────────────────────┤
│  Layer 3: Sidecar accounts for optional features         │
│  - Platform fees, cancellation policy, max capacity      │
│  - EventEscrowMeta sidecar for new optional data         │
│  - AttendeeDepositMeta sidecar if needed                 │
│  - Instructions handle None (sidecar not present)        │
├──────────────────────────────────────────────────────────┤
│  Layer 4: Program ID upgrade (break-glass only)          │
│  - Reserved for seed changes or fundamental restructure  │
│  - Only when all active events are closed                │
│  - Requires coordinated deployment + client update       │
└──────────────────────────────────────────────────────────┘
```

### When to Use Each Strategy

| Scenario | Strategy | Rationale |
|----------|----------|-----------|
| Add optional field (platform fee, max capacity) | Sidecar (Layer 3) | Zero risk, opt-in |
| Add mandatory field (new counter, flag) | Versioned append (Layer 2) | Version dispatch, grandfather old |
| Change field semantics (bool → enum) | Versioned append (Layer 2) | New version, old behavior for v0 |
| Remove a field | Don't. Deprecate and ignore | Removing = reordering = corruption |
| Change PDA seeds | Program ID upgrade (Layer 4) | Different seeds = different address |
| Add 3rd account type | New discriminator | No impact on existing accounts |
| Emergency fix (logic bug) | Program upgrade only | Buffer-safe upgrade (no struct change) |

### The "Let It Close" Rule

**For ephemeral events, the simplest migration is often no migration.**

When a new field is added:
1. Deploy the upgraded program.
2. New events (created after upgrade) get the new struct with `version = N`.
3. In-flight events (created before upgrade) remain at their current version.
4. Instructions handle both versions until all old events close.
5. Once all old-version accounts are closed, remove the old version handling code.

Expected lifecycle: most events close within 2-4 weeks of creation. The window where dual-version handling is needed is **finite and short**.

### What NOT To Do

| Anti-pattern | Why it's bad |
|-------------|-------------|
| Insert fields in the middle of a struct | Shifts all subsequent bytes = corruption |
| Change field types in place (u64 → u128) | Different byte width = misaligned reads |
| Reuse a field for a different purpose | Client code, indexer, off-chain state all assume original semantics |
| Remove a field to "save space" | All offsets after the removal shift = corruption |
| Reserve large padding blocks | Wastes rent. Sidecars are cheaper for rare extensions. |
| Change PDA seeds for cosmetic reasons | Creates entirely new accounts, orphans old ones |

---

## 8. Pre-Mainnet Checklist

### Must-Do Before First Mainnet Deployment

- [x] **Add `version: u8` field to both account structs** — first data field after discriminator (v1 implemented)
- [x] **Set `CURRENT_VERSION = 1`** — `ESCROW_VERSION` / `DEPOSIT_VERSION` constants, written in `set_inner()` calls
- [ ] **Add version-aware deserialization** — at minimum, log a warning if version < current
- [x] **Allocate padding bytes** — EventEscrow: 36 bytes, AttendeeDeposit: 11 bytes (total sizes: 192B / 96B)
- [ ] **Write a `migrate_event_escrow` instruction** (even if unused initially) that can upgrade v0 → vN
- [ ] **Write a `migrate_attendee_deposit` instruction** (even if unused initially)
- [ ] **Test migration path** on devnet: create v0 accounts, deploy upgraded program, migrate, verify
- [x] **Document the version history** in code comments and this document
- [ ] **Verify upgrade authority** is set correctly (multi-sig recommended for mainnet)
- [ ] **Consider upgrade authority revocation** after initial deployment if no changes are planned

### Account Size Planning

| Account Type | v0 Size | v1 Size (Current) | Padding | Total |
|-------------|---------|-------------------|---------|-------|
| EventEscrow | 149 bytes (1 disc + 148 fields) | 192 bytes | +36 bytes | 1(disc) + 1(ver) + 148(fields) + 36(pad) = 192 |
| AttendeeDeposit | 84 bytes (1 disc + 83 fields) | 96 bytes | +11 bytes | 1(disc) + 1(ver) + 83(fields) + 11(pad) = 96 |

The padding allows 3-4 field additions without requiring reallocation. If we hit the ceiling, we use the Resize instruction or sidecars.

### Upgrade Authority Governance

| Environment | Authority | Recommendation |
|------------|-----------|---------------|
| Devnet | Deployer keypair | Single key, fine for testing |
| Mainnet | Deployer keypair → Multi-sig | Use Squads or similar 2-of-3 multi-sig |

---

## 9. Future Schema Changes Register

Predicted schema changes, their expected timeline, and the planned migration path.

| # | Change | Type | Target Account | Strategy | Priority |
|---|--------|------|---------------|----------|----------|
| F-01 | Add `platform_fee_bps: u16` (platform fee basis points) | New field | EventEscrow | Sidecar or Versioned append | P4 (future) |
| F-02 | Add `max_attendees: u32` (capacity cap) | New field | EventEscrow | Sidecar (optional feature) | P3 (near-term) |
| F-03 | Add `cancelled: bool` (event cancellation) | New field | EventEscrow | Versioned append | P1 (security audit) |
| F-04 | Add `cancel_event` instruction | New instruction | N/A | Program upgrade only | P1 (security audit) |
| F-05 | Add `refund_and_close` combined instruction | New instruction | N/A | Program upgrade only | In progress |
| F-06 | Add `deposited_count: u32` (number of deposits) | New field | EventEscrow | Versioned append | Nice-to-have |
| F-07 | Add `checked_in_at: i64` (check-in timestamp) | New field | AttendeeDeposit | Versioned append | Nice-to-have |
| F-08 | Add `event_format: u8` (in-person/online/hybrid) | New field | EventEscrow | Sidecar or Versioned append | P2 (near-term) |
| F-09 | Change `is_active: bool` → `status: u8` (active/deactivated/cancelled/settled) | Semantic change | EventEscrow | Version bump (v2), handle v1 as `is_active` | Requires F-03 |
| F-10 | Add multi-organizer support (delegate authority) | New account type | New PDA | New discriminator, no migration | Future |
| F-11 | Support Token-2022 USDC (different mint) | Mint change | EventEscrow | No struct change, different `usdc_mint` value | Future |
| F-12 | Add `EventEscrowMeta` sidecar account | New account type | N/A | New discriminator = 3, linked by reference | F-01/F-02/F-08 |

### Version History (to be maintained as changes occur)

| Version | Date | Changes | Migration Required |
|---------|------|---------|-------------------|
| 0 | Devnet (pre-v1) | Initial struct layout, no version field | N/A |
| 1 | Pre-mainnet (current) | Add `version: u8` (first field), add `_padding: [u8; N]` (last field), EventEscrow → 192B, AttendeeDeposit → 96B | Fresh deploy, no migration needed |
| 2 | TBD | (reserved for first field addition) | Append-only, "let it close" |

---

## Appendix: Technical Reference

### Solana Account Reallocation (Resize)

Accounts can be grown in-place using the `Resize` instruction:

```rust
use solana_program::system_instruction;

// In the instruction handler:
let new_size = current_size + additional_bytes;
let rent = Rent::get()?.minimum_balance(new_size);
let additional_rent = rent.saturating_sub(Rent::get()?.minimum_balance(current_size));

// Transfer additional rent from payer
system_instruction::transfer(&payer, &account.key, additional_rent);

// Realloc the account
account.to_account_info().realloc(new_size, false)?;
```

**Constraints:**
- Can only grow, not shrink (unless zeroing and closing).
- Payer must sign and have sufficient SOL.
- Account owner must be the program (i.e., PDAs owned by this program).
- Compute budget applies.

### Quasar Discriminator Notes

Quasar uses a **single-byte discriminator** (`#[account(discriminator = N)]`), unlike Anchor's 8-byte SHA256 prefix. This means:
- Discriminator range: 0-255 (theoretically, but N < 16 is practical)
- 8 bytes saved per account vs Anchor
- Discriminator 0 is used for the program entrypoint (first instruction)
- Available discriminators for accounts: 1-15 (ample room for new account types)

### PDA Seed Constraints

- Seeds are `&[u8]` slices, max 32 bytes each.
- Max total seed length: unclear in Quasar docs, but Solana limit is ~64 bytes total.
- Seeds cannot be changed for existing accounts — the derivation is deterministic.
- BeThere seeds use `Address` (32 bytes) and `u64` (8 bytes), which is well within limits.

---

*This document should be updated whenever a schema change is proposed or deployed. Last updated: pre-mainnet planning phase.*
