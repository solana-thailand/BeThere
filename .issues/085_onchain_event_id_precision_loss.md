# 085 — `on_chain_event_id` is a u64 in a signed-64-bit column, and half of them are silently corrupted

**Status:** open — **production data affected**, recovery demonstrated, fix not applied
**Found:** 2026-09-13, diagnosing why the flow-harness deposit flow failed
**Severity:** high — every escrow operation for an affected event is broken, and
the failure is silent at write time

## What

`events.on_chain_event_id` holds a **u64** — that is what the on-chain program
seeds the `EventEscrow` PDA with. SQLite integers are **signed** 64-bit.

`db/events.rs:577` interpolates the value straight into the SQL literal. When it
exceeds `i64::MAX` (9 223 372 036 854 775 807), SQLite cannot store it as an
integer and silently falls back to **REAL**. The low digits are gone
permanently — the write does not fail, nothing is logged, and the row looks
fine.

The ids appear to be randomly generated across the whole u64 range, so **about
half of every escrow ever created is affected**. Observed rate: 6 of 8 on
staging, 1 of 1 in production.

```sql
SELECT id, typeof(on_chain_event_id), CAST(on_chain_event_id AS TEXT)
FROM events WHERE on_chain_event_id NOT IN (0);
-- typeof = 'real'  →  corrupted
```

## Why it breaks everything downstream

`EscrowCtx::resolve` (`worker/src/solana_escrow/tx_builders/mod.rs:88-99`)
derives the escrow PDA **from the id**, never from the stored address:

```rust
find_program_address(&[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()], &program_id)
```

Every builder shares it — `deposit`, `refund`, `close`, `mark`, `rollover`. So a
corrupted id makes all of them address a PDA that does not exist, while
`escrow_address` sits correct-but-unused in the same row.

The observable symptom is a simulation failure that names nothing useful:

```
Program C6HDeZES… consumed 195 of 200000 compute units
Program C6HDeZES… failed: Provided owner is not allowed     # IllegalOwner
```

195 compute units — it dies at account validation, because the accounts belong
to a PDA that was never created.

## Production impact

`islanddao-v4-demo`:

| | |
|---|---|
| `escrow_status` | `initialized` |
| `escrow_address` | `H8iHzXcz9Sq3sr5Ny5B5VEvCv6bN77NZ6EhmJc163uQ` (correct) |
| stored id | `1.5159210065911599e+19` (REAL — corrupted) |
| deposits | **1, `verified=1`, `refundable=1`, 15 USDC** |

**That deposit cannot be refunded through the Worker today.** The funds are in
the real vault; the Worker cannot build a transaction that addresses it.

Mitigating: production runs `SOLANA_CLUSTER = "devnet"`, so this is devnet USDC,
not mainnet money. **That is the only reason this is not an incident.** The same
code path on mainnet would strand real funds, and would do so silently.

## The corruption is recoverable

`escrow_address` is TEXT and correct, so it can be used as an oracle: the true
id is within a few float64 ULPs of the stored value, and only one candidate in
that range derives to the known address.

`scripts/verify/onchain_event_id_audit.py` does this. Against production:

```
event          islanddao-v4-demo
stored value   1.5159210065911599e+19  (REAL — low digits lost)
recovery       ✅ 15159210065911598203  (derives to the stored address)
```

901 below the stored float. Verified by re-deriving the PDA.

Recovery only works while `escrow_address` is intact. An event with a corrupted
id and **no** stored address is unrecoverable, which is why this should be fixed
before the next escrow is created.

## Fix

1. **Stop the bleeding.** Store `on_chain_event_id` as `TEXT` (SQLite has no
   unsigned 64-bit integer) and parse to `u64` in Rust, or constrain generation
   to `< i64::MAX` so the value always fits. The first is correct; the second is
   cheaper and keeps the column numeric.
2. **Fail loudly meanwhile.** `upsert_event` should reject — not silently
   round — an id above `i64::MAX`. A guard test over the DDL plus a unit test on
   the boundary value.
3. **Migrate the existing rows** using the recovery script, for every event
   where `escrow_address` is present.
4. **Add a read-back assertion** after escrow initialization: re-read the row
   and confirm the stored id still derives to the address that was just
   confirmed on-chain. That converts a silent corruption into an immediate,
   loud failure — this bug survived because nothing ever checked.

Do not "fix" this by having the builders read `escrow_address` instead. That
would paper over a corrupt id and leave rollover/close deriving other PDAs from
the same bad value.

## Verification

```sh
python3 scripts/verify/onchain_event_id_audit.py --db bethere-db --recover
python3 scripts/verify/onchain_event_id_audit.py --db bethere-db-staging --recover
```

Exits non-zero while any row is stored as REAL.

## Related

- `.issues/084_preflight_gate_has_never_been_satisfiable.md` — the harness
  deposit failure that led here. The remaining "SPL ownership" blocker in that
  issue **is this bug**, not a vault problem.
- `.issues/047_instruction_introspection.md`, `.issues/013` — escrow safety.
