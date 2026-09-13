# 085 — `on_chain_event_id` is a u64 in a signed-64-bit column, and half of them are silently corrupted

**Status:** open — **production data affected**, worse than first written (see
"Escalation"), recovery demonstrated, fix not applied
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


---

## Escalation — 2026-09-13, later

The first write-up said this affects "about half" of events. **It affects
effectively all of them**, and there is a second, independent bug.

### Bug 2: the read path loses precision on *every* id

D1 rows reach Rust through `JSON.stringify`, so every integer becomes a
JavaScript number — float64. Anything above **2⁵³** (9 007 199 254 740 992)
is rounded. FNV-1a output is spread across the whole u64 range, so 2⁵³ is a
negligible fraction of it: **essentially every id is corrupted on read**, even
when D1 holds it perfectly.

Demonstrated on a row whose stored value is *intact*:

```
flow-test-event   raw=5055890856068877000   CAST(... AS TEXT)=5055890856068877793
```

The database is right; the Worker is wrong. So Bug 1 (write) and Bug 2 (read)
are independent, and Bug 2 has the wider blast radius.

`db/events.rs:64-67` carries a comment about u64 exceeding `i64::MAX` — someone
hit the *deserialization* symptom and re-typed the field as `u64`, but never
noticed the rounding. The example value in that comment,
`15159210065911600000`, is itself the corrupted form of `islanddao-v4-demo`'s
real id.

### Why anything worked at all: KV was holding the truth

`EventConfig` in KV is serialized by **Rust**, so KV held the exact u64. The
KV-first read path therefore returned correct ids, and D1 was only consulted on
a KV miss. That is why escrow operations mostly worked.

### `POST /api/events/reseed-kv` destroys that

It rewrites KV from D1 — copying the corrupted value over the good one. It is a
**live production endpoint**. Running it replaces every exact KV id with a
rounded one and makes the corruption total and permanent.

Observed: after a reseed on staging, the Worker reports
`flow-test-event → 5055890856068877000`, though D1 still holds
`…877793` and the true value is `…877793`.

**Do not call `reseed-kv` on production until this is fixed.** It is not a
recovery tool for this bug; it is an amplifier.

### The true id is always recomputable

`derive_on_chain_event_id` (`handlers/deposit/mod.rs:48`) is FNV-1a over the
event's string id — a pure function. Verified two independent ways:

| event | FNV-1a of the id | recovered by PDA search |
|---|---|---|
| `islanddao-v4-demo` | `15159210065911598203` | `15159210065911598203` ✓ |
| `flow-test-event` | `5055890856068877793` | matches the intact D1 value ✓ |

So repair does not need the brute-force search at all — recompute and verify
against `escrow_address`. The search remains useful only for an event whose id
was explicitly overridden rather than derived.

### Revised fix

1. **Column to TEXT** (migration) and write a quoted literal — fixes Bug 1.
2. **Deserialize from string** — fixes Bug 2. A tolerant deserializer
   (number *or* string) is needed during rollout, but a numeric value above
   2⁵³ must then be treated as **untrusted**, not merely parsed.
3. **Repair existing rows** by recomputing FNV-1a and asserting the result
   derives to the stored `escrow_address`.
4. **Guard `reseed-kv`** so it cannot write an id that fails that assertion.
5. Read-back assertion after escrow init, as above.
