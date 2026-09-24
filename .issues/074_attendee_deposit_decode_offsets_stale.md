# 074 — The devnet e2e decodes `AttendeeDeposit` at pre-`version` offsets, so step 9 verifies nothing

Status: **closed 2026-09-23 — full script run green against staging.** The
offsets were verified against the deployed program on 2026-09-17 (38/38 real
devnet `AttendeeDeposit` accounts decode and re-derive; see below). The full
`e2e_devnet_test.sh` run that stayed outstanding is now done, after the script
was repaired for four contract drifts it had accumulated (see "Full run —
2026-09-23"). Found while converting the `python3 -c` interpolation sites listed
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

**Nothing — the full run is done; see "Full run — 2026-09-23" below.** The
history is kept as it was written.

~~A real devnet run.~~ The open question was whether the corrected offsets
match what the deployed program writes. That is now answered from real chain
data (below) without needing a fresh deposit. A full end-to-end run of
`e2e_devnet_test.sh` is still outstanding, but it is a script-level check, not
an offset check.

~~and it is blocked on devnet USDC with the #084 gate.~~ **That blocker is
stale, corrected 2026-09-22.** Re-measured directly against devnet rather than
re-read from the issue that asserted it:

```
$ solana balance 7ABX2ZyPogms6dvb3f8mTACy3SZUui25DhqYmSY9LSNC --url devnet
4.99657124 SOL
$ spl-token accounts --owner 7ABX2… --url devnet
4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU   9.99998
```

The harness attendee wallet is funded in both SOL and devnet USDC
(`4zMMC9sr…` is the devnet USDC-Dev mint), so nothing here waits on a faucet
captcha. `.issues/084` carries the same correction for its own USDC blocker.
What actually remains for a green run is the four contract drifts listed in
`.issues/084`, not funding.

## Full run — 2026-09-23

`PATH=<venv with solders>:$PATH ./scripts/e2e_devnet_test.sh --skip-setup --non-interactive`
against `bethere-staging` (DEV_MODE on), organizer = default CLI wallet
`9Bz7p4RW…`, attendee = the funded harness wallet `7ABX2ZyP…`. **Exit 0, ALL
PASSED**, event `e2e-test-event-1790173781`, escrow
`95Z2hU57wKpphvhLUUXKBMb17hwrLV5s4G33vV3Q8f8a`:

```
[PASS] Escrow initialized and confirmed: 95Z2hU57… (initialized)
[PASS] Deposit verified via webhook
[PASS] Deposit verified on-chain (version=1)        amount 1000000
[PASS] checked_in=true on-chain
[PASS] AttendeeDeposit closed
[E2E]   Attendee USDC: 8999980 -> 9999980 (raw)
[E2E]   Vault USDC:    0 (raw)
[PASS] Full escrow cycle verified!
```

The first two runs failed. Four drifts between the script and the Worker had
never surfaced because the script had not been run since they landed:

1. **`/api/escrow/create-vault-ata` + `/api/escrow/create-event` no longer
   exist.** Both were folded into `POST /api/escrow/init` (one transaction:
   idempotent vault ATA + `create_event`). The script got an empty body and
   died at step 3. Now: `init` → sign → send → `POST /api/escrow/confirm-init`,
   which re-derives the PDA and checks it on-chain before writing
   `escrow_address` back. That replaces a raw `PUT /api/events/{id}`, which
   would have recorded an address nobody had checked.
2. **The webhook's `confirmed:false` was treated as a warning.** Since H8 the
   verification is detached, so the first reply is *always* `false`; re-sending
   the same signature is an idempotent no-op that answers `true` once the
   background check lands. The script now polls it and fails if it never
   verifies. It verified on the second poll.
3. **Step 9 decoded an account the refund had closed.** `/api/escrow/refund`
   builds `refund` + `close_deposit` in one transaction, so a successful refund
   leaves no `AttendeeDeposit` to read. The old step 9 would therefore fail on
   every *successful* run ("account not found after 10 retries"). It now asserts
   the PDA is gone, that the attendee's USDC rose by exactly `DEPOSIT_AMOUNT` (raw
   base units from a pre-refund snapshot), and that the single-depositor vault is 0.
4. **`checked_in` was only asserted in step 9**, i.e. on the closed account. It is
   now asserted in step 6, right after `mark_checked_in` confirms, while the
   account still exists.

Also: the fixed `sleep 130` before the refund became a wait until
`event_end + 15 s` (one `EVENT_END_S` constant feeds both the event body and the
wait), and the steps are renumbered 0–8.

**The new assertions were checked in both directions**, not only on the green
run: `poll_deposit_field` on the now-closed PDA returns failure; a zero USDC delta
is rejected; `getTokenAccountBalance` on a non-token account yields `""`, not
`"0"`, so a wrong vault address cannot pass as an empty vault.

**Relevance to [141](141_preflight_suite_is_self_contradictory_on_one_fixture.md):**
`flow-harness/README.md` says a fixture's escrow must be initialized by hand from
Manage Events. This run shows that it need not be: `init` + `confirm-init`, signed
with the organizer keypair, is an unattended path, and it is the seeding step
141's Phase A and Phase B both need.

## Verification against real devnet accounts — 2026-09-17

`getProgramAccounts` on the deployed program `C6HDeZES…` with a 96-byte +
discriminator-2 filter returned **38** `AttendeeDeposit` accounts, created
between 2026-05-26 and 2026-09-13. The full program has 91 accounts, all version
1: 53 `EventEscrow` and 38 `AttendeeDeposit`. None use the 84-byte layout.

Each account was decoded with the canonical offsets and checked against
independent on-chain facts rather than plausibility:

| check | canonical offsets | old pre-`version` offsets |
|---|---|---|
| `find_program_address(["deposit", event, attendee])` == account address | **38/38** | 0/38 |
| derived bump == byte `[84]` | 38/38 | — |
| `event` field is a live `EventEscrow` of the program | 37/38 ¹ | 0/38 |
| `checked_in`/`refunded` bytes are 0 or 1 | 38/38 | — |
| padding `[85..96]` all zero | 38/38 | — |

¹ The one miss (`GeiMUECT…`) points at `DVpYsycx…`, an escrow with transaction
history that has since been closed. The deposit is refunded and its PDA still
derives from that event key, so the offsets are correct there too.

The PDA match is the decisive check: the address can only re-derive if bytes
`[2..34]` and `[34..66]` really are `attendee` and `event`. Amounts decode to
round values (1, 10 or 15 USDC; two accounts hold 10 base units). States seen:
25 not checked in, 12 checked in and refunded, and **1 checked in but not
refunded** (`HcQtePa1…`). That last one is the exact state this issue's
false-green depended on, and it exists on chain. On that account the old decoder
reads `amount=256000046`, and byte `[82]` = 1 is what it treated as "refunded".

The script's own `decode_deposit` helper, extracted verbatim, was run on real
base64 for all three states (`HcQtePa1…`, `2h8zLaXX…`, `2ngtttvm…`), and every
field matched. Two of those accounts are now pinned as regression fixtures:
`worker/src/solana_escrow/wire.rs` (`decodes_real_devnet_accounts`) and
`flow-harness/src/chain.rs` (`decode_attendee_deposit_real_devnet_account`).

## Follow-up

The audit this issue called for — "does any *Rust* consumer carry the same
drift?" — was done and found one: the Worker's own decoder gated on the
pre-`version` length of 84. Fixed under
[075](075_rust_deposit_decoder_stale_length_gate.md), which also confirms
`EventEscrow` has no raw-byte decoder anywhere outside the program.
