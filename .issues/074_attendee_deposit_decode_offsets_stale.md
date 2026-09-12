# 074 — The devnet e2e decodes `AttendeeDeposit` at pre-`version` offsets, so step 9 verifies nothing

Status: **fixed locally** — offsets corrected, assertions moved into the shell,
and the false-green reproduced before and after. Closing it fully needs a real
devnet run. Found while converting the `python3 -c` interpolation sites listed
as deferred work in [073](073_signing_keypair_in_process_argv.md).

## What happened

`scripts/e2e_devnet_test.sh` decoded the on-chain `AttendeeDeposit` account at
hand-written byte offsets starting `off = 1`, with a `len(data) >= 84` guard:

```python
off = 1  # quasar-lang: 1-byte discriminator
attendee = data[off:off+32]; off += 32
event    = data[off:off+32]; off += 32
amount   = struct.unpack('<Q', data[off:off+8])[0]; off += 8
...
```

That layout has been wrong since `f246e8e feat(escrow): add version field +
padding`, which inserted a `version: u8` immediately after the discriminator.
The account is now **96 bytes**, not 84, and every field sits one byte later.
The script was never updated — its last two touches (`d2c595b`, `c490036`) were
unrelated.

Authoritative layout, `bethere-escrow/src/state.rs:57` and the tested decoder in
`flow-harness/src/chain.rs:173`:

```
[0] discriminator=2  [1] version  [2..34] attendee  [34..66] event
[66..74] amount  [74..82] deposited_at  [82] checked_in  [83] refunded
[84] bump  [85..96] padding
```

## Why it matters — step 9 passed without a refund

The `len(data) >= 84` guard never trips, because real accounts are 96 bytes. So
the decode did not fail; it silently returned neighbouring bytes.

Step 9's polling helper computed `off = 1+32+32+8+8+1 = 82` and called it
`refunded`. Byte 82 is really **`checked_in`**, which step 7 had just set to
`true`. So the "wait until refunded" loop broke out on its first iteration
whether or not the refund had landed. The two assertions that followed were
equally displaced: `checked_in` read the high byte of `deposited_at` (non-zero
for any real timestamp) and `refunded` read the real `checked_in` (`true`).

**Both assertions in "Step 9: Verify refund on-chain" passed unconditionally.**
The one step that proves attendee money comes back was structurally incapable
of failing.

Reproduced against synthetic account bytes for a checked-in, *not yet refunded*
deposit (`amount=1000000 checked_in=true refunded=false`):

| field | truth | old decoder |
|---|---|---|
| `amount` | `1000000` | `256000187` — byte-shifted garbage |
| `checked_in` | `true` | `false` |
| `refunded` | `false` | **`true`** |

Step 6 was broken too, but loudly: `assert amount == $DEPOSIT_AMOUNT` compares
`256000187` against `1000000`, so a genuine devnet run would hard-fail there.
That is the tell that this script has not completed end-to-end since `f246e8e`.

## Fix

- One `decode_deposit()` helper replaces all three hand-rolled decode sites
  (step 6 verify, step 9 poll, step 9 verify). Offsets are the tested ones, and
  the source is cited in a comment so the next schema change has a breadcrumb.
- It now **validates** rather than assuming: rejects `len < 96` and rejects a
  discriminator other than `2`. A stale-layout account can no longer decode to
  plausible-looking garbage.
- It prints `key=value` lines and the **shell** asserts, so `$DEPOSIT_AMOUNT` is
  no longer interpolated into a Python program.
- The step 9 poll no longer treats an unreadable account as "not refunded yet";
  if nothing ever decodes, the step fails naming the decode error.
- Step 9's PDA derivation gained the `|| fail` its step 6 twin already had — it
  previously swallowed a derivation error and polled a garbage address.
- Amount display uses integer arithmetic. The old `{amount/1_000_000:.2f}`
  rounded to cents and would have hidden sub-cent discrepancies.

## Verification

Synthetic accounts built to the canonical layout, checked in both directions —
a test that only ever passes proves nothing:

| case | expected | result |
|---|---|---|
| checked in, **not** refunded | step 9 **fails** | fails: `Should be refunded, got refunded=false` |
| checked in **and** refunded | step 9 passes | passes |
| discriminator `7` | rejected | `AttendeeDeposit discriminator = 7, expected 2` |
| 84-byte pre-`version` account | rejected | `AttendeeDeposit too short: 84 < 96` |

Row 1 is the case the old code passed.

Also confirmed no sibling instance of the defect: `e2e_devnet_test.sh` is the
only script in the repo that decodes raw escrow account bytes — the rollover and
escrow scripts assert through the API instead. (Checked because guards landing
on one path while a twin keeps the bug is a recurring shape here.)

## Remaining

A real devnet run. All verification above used synthetic account bytes; nothing
proves the corrected offsets match what the deployed program actually writes,
only that they match the program source and the flow-harness decoder.

## Follow-up

The audit this issue called for — "does any *Rust* consumer carry the same
drift?" — was done and found one: the Worker's own decoder gated on the
pre-`version` length of 84. Fixed under
[075](075_rust_deposit_decoder_stale_length_gate.md), which also confirms
`EventEscrow` has no raw-byte decoder anywhere outside the program.
