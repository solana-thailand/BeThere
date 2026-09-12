# 072 — Dead variables in the e2e/integration scripts hide unfinished checks

Status: **resolved** — all 6 items closed, plus 2 out-of-scope entries and 1
item this issue originally missed. The ShellCheck deferral list in
`scripts/verify/shellcheck_gate.sh` is now empty: every `.sh` in the repo is
gated (24 scripts, clean).

Found while clearing ShellCheck across every shell script for #065. These were
**not** style warnings: each unused variable marked a check that was written
and then never wired up, so the script passed without testing what it claimed
to.

## Two claims in the original write-up were wrong

Both were corrected by checking the actual contract rather than reading the
script, and both would have caused a bad "fix":

1. **Item 3 was misdiagnosed.** The original text said the refund deadline was
   never sent and the on-chain event got "whatever default the program
   applies". It *was* being sent — as `refund_deadline_hours: 168` on the line
   right below. The events API has **no** absolute-timestamp field for it
   (`worker/src/event_store/write/create.rs:146`,
   `worker/src/db/events.rs:68`); `refund_deadline_ms` is a derived *output*
   only (`worker/src/handlers/deposit/usdc/handlers/status.rs:114`). Wiring the
   dead variable in as the issue suggested would have been impossible, and the
   dead value said **30 days** against the 7 days actually sent — so
   "resolving" it the documented way would have silently changed the deadline.

2. **The item 2 stub never printed `checking...`.** Its first line is
   `import hashlib, struct, base58`, and `base58` is not installed; the
   `ImportError` went to the suppressed stderr, so `VAULT_ATA` was the empty
   string, not `checking...`. Immaterial to the fix, but the stub was even
   deader than described.

## Resolutions

### 1. `scripts/e2e/test_lifecycle.sh` — `--reuse` — **implemented**

Implemented rather than deleted: the sibling `test_escrow_devnet.sh` already
has this exact capability (`--skip-setup`, "reuse existing event"), so the flag
was consistent with the codebase, not speculative. Under `--reuse` Step 1 now
GETs the event instead of POSTing, and additionally asserts that
`organizer_wallet` matches the local keypair — a mismatch means the escrow PDA
belongs to someone else and every later step would fail `Unauthorized`
(escrow code 9), which is exactly the confusing failure `--reuse` invited.

The root cause of the flag going unnoticed is also fixed: the arg loop had no
`*)` branch, so it silently swallowed anything it did not recognise. It now
exits 2 on an unknown argument.

Verified against a stub worker: unknown flag → exit 2 before any network;
`--reuse` on an existing event → reuses with no POST to `/api/events`;
`--reuse` on a missing event → clean failure, exit 1; wallet mismatch → fails
with both addresses named.

### 2. `scripts/e2e/test_escrow_devnet.sh` — vault ATA — **real check**

The abandoned scaffolding (`find_pda` is `pass`, output `print('checking...')`)
is replaced by an independent cross-check. The Worker derives the vault as
`get_associated_token_address(event_escrow, usdc_mint)` — seeds
`[escrow, TOKEN_PROGRAM, mint]` under the ATA program
(`worker/src/solana_escrow/crypto.rs:342`) — and returns it as `vault_address`.
`spl-token address --token … --owner … --verbose` derives the same address
independently, so the script now fails on a mismatch, which would mean the
Worker funds the wrong account.

Checked that this compares like with like: the devnet USDC mint
`4zMMC9…` is owned by the **legacy** `TokenkegQfe…` program, the same
`TOKEN_PROGRAM_ID` the Worker seeds with, so `spl-token` does not pick a
Token-2022 seed. `VAULT_ADDR` is now initialised to `""` so `--skip-setup`
cannot trip `set -u`, and that case degrades to an informational line rather
than a silent pass.

`--with-vault-ata` is removed from the usage block: it documents a two-step
workflow that no longer exists, since ATA creation was folded into
`/api/escrow/init` (`worker/src/solana_escrow/tx_builders/init.rs:14`).

### 3. `scripts/e2e_devnet_test.sh` — `refund_deadline_ms` — **removed, value single-sourced**

The misleading 30-day computation is gone. The deadline the script really sends
is now a named `refund_deadline_hours=168` local interpolated into the body, so
the value is stated once instead of twice, with a comment recording that the
API takes hours relative to `event_end_ms` and 168 matches the Worker's own
default (`worker/src/db/events.rs:242`). Verified the rendered body is still
valid JSON with `refund_deadline_hours` as an unquoted integer `168`.

### 4. `scripts/e2e_devnet_test.sh` — `ESCROW_ADDRESS` — **removed**

Deleted. The file `escrow_address.txt` is the deliberate channel between steps —
steps 5, 7 and the summary each re-read it, so a single step can be re-run in a
fresh shell. A comment now records that, so nobody "helpfully" reintroduces the
variable.

### 5. `tests/integration/run.sh` — malformed constants — **corrected and now asserted**

`TOKEN_PROGRAM` gained its missing `g` and `SYSTEM_PROGRAM` dropped from 41
ones to 32. Decoded rather than eyeballed: the old values were **31 bytes** and
**41 bytes** respectively, so neither was a valid 32-byte pubkey.

Rather than just correcting them, "Test 7: program constant addresses" now
decodes all six constants and fails if any is not a canonical 32-byte base58
key. Pure base58, no dependency, so it runs where `solders` is absent. Proven
both ways: it passes on the corrected values and, re-run against the pre-fix
values, fails naming both (`TOKEN_PROGRAM decodes to 31 bytes, not 32;
SYSTEM_PROGRAM decodes to 41 bytes, not 32`).

### 6. `tests/integration/run.sh` — `EVENT_ID=42` — **threaded in**

The derivation used a `42` hardcoded inside the Python program while
`EVENT_ID=42` sat unused beside it. `EVENT_ID` is now passed through, and the
program id and organizer with it — **as argv, not interpolated into the program
text**, per the #065 lesson. Seeds are commented against their source of truth
(`bethere-escrow/src/state.rs:17`). Proven with a stubbed `solders` that echoes
its seeds: the event id arrives as `b'*\x00\x00\x00\x00\x00\x00\x00'` (0x2a = 42,
u64 little-endian). Without `solders` the block still takes the failure branch
cleanly.

### Previously "not in scope" — **closed**

`scripts/e2e/test_rollover_devnet.sh` and `test_rollover_full_lifecycle.sh`
each declared an unused `ESCROW_PROGRAM`. Deleted: neither script inspects the
on-chain account owner, so the constant documented nothing. That clears the
last two entries from the gate's deferral list.

**Follow-up worth having (not a defect):** `test_escrow_devnet.sh` asserts the
escrow PDA is owned by the escrow program; the two rollover scripts never do.
Adding that assertion there needs a devnet run to validate, so it is not done
here.

### 7. Missed by this issue — `HELIUS_API_KEY` — **wired up, via a detour through #073**

ShellCheck also flagged `HELIUS_API_KEY` in `test_escrow_devnet.sh`: read from
`worker/.dev.vars` and never used, while the script talks to the public devnet
RPC, which is rate-limited and shows up here as flaky confirmations. The fix is
to point `RPC_URL` at the authenticated Helius devnet endpoint the Worker
itself uses (`worker/wrangler.toml:263`, `worker/src/state.rs:172` — Helius is
still the live RPC provider; only *NFT minting* moved to Crossmint).

That fix was written, **reverted**, and then re-applied. Reverted because
`RPC_URL` was forwarded to `sign_and_submit.py` as argv, so a keyed URL would
have been readable by any local user via `ps` — the exact exposure #065 closed
for the deploy fallback. Investigating that turned up something worse in the
same call: the **signer's private key** was passed as argv too
(`"$(cat ~/.config/solana/id.json)"`), across four scripts. That is **#073**,
now implemented: both secrets moved to an environment channel.

With argv clean, the Helius upgrade is in. An explicit `RPC_URL` still wins,
the public endpoint is still used when no key is present, and the banner prints
`?api-key=<redacted>` — verified by grepping the script's own output for the
real key and finding no match. The `SC2034` suppression is gone, so the
variable is genuinely live.

## Gates

- `bash scripts/verify/shellcheck_gate.sh` — 24 scripts, **0 deferred**, clean;
  still fails closed (verified by dropping a script carrying the #065 backtick
  defect into the tree: 25 scripts, exit 1).
- Emptying `DEFERRED=()` required an `${arr[@]+"${arr[@]}"}` guard — the plain
  `"${DEFERRED[@]}"` form is an `unbound variable` error on the bash 3.2 that
  ships with macOS, which is the default `bash` on this machine. Verified both
  forms under 3.2.
- `bash -n` passes on every changed script.
- `tests/deploy_fallback` — 40/40 pass (untouched by this work).
- No Rust files touched.

## Related

**#073** — the signer's private key was reaching `ps` via argv. Found while
chasing item 7 above; implemented, pending a devnet run. Its own deferred
item — the `python3 -c` interpolation in `scripts/e2e_devnet_test.sh` — has
since been closed too.

**#074** — the same script decoded `AttendeeDeposit` at pre-`version` byte
offsets, so "Step 9: Verify refund on-chain" asserted on the wrong bytes and
passed whether or not the refund landed. This is the dead-assertion shape this
issue is about, but on-chain rather than in shell, and it was invisible to
every check here: the variable *was* used, the guard *did* run, and the
assertion *did* evaluate — against the wrong byte. Found only by decoding a
synthetic account and comparing against the program source.

## Follow-up — the escrow-ownership assertion (item recorded above), closed

The open item here read: `test_escrow_devnet.sh` asserts the escrow PDA is owned
by the escrow program; the two rollover scripts never do. Both halves turned out
to be wrong in the same direction — **neither** script asserted it.

### 1. The rollover scripts never checked (as recorded)

`test_rollover_devnet.sh` and `test_rollover_full_lifecycle.sh` take
`SOURCE_ESCROW_ADDR` / `TARGET_ESCROW_ADDR` straight out of the Worker's
`/api/escrow/init` response and never verify them. Nothing downstream catches a
bad address either: every later reference goes through
`spl-token balance --owner "$ADDR"`, which reports `0` for an address that owns
no token account rather than erroring — a wrong address reads as a plausible
balance, not a failure.

Both scripts now share an `assert_escrow_owned_by_program()` helper, called for
each escrow once both are initialized. Empty address is `skip` under
`--skip-setup` (where it legitimately is not captured) and `fail` otherwise.

### 2. `test_escrow_devnet.sh` did check — with a pattern that never matched

The existing check parsed the CLI output with **lowercase, case-sensitive**
patterns:

```bash
ESCROW_OWNER=$(echo "$ESCROW_INFO" | grep "owner:" | awk '{print $2}' || echo "?")
if [ "$ESCROW_OWNER" = "$ESCROW_PROGRAM" ]; then pass ...
```

`solana account` prints `Owner:` capitalized. Verified against the live devnet
account for the escrow program itself (solana-cli 3.1.10):

```
Owner: BPFLoaderUpgradeab1e11111111111111111111111
Length: 36 (0x24) bytes
```

`grep "owner:"` returns nothing, so under the script's `set -o pipefail` the
pipeline fails and `ESCROW_OWNER` becomes `"?"`. **The `pass` branch was
unreachable**; every run took the `warn` branch, which does not count as a
failure. The neighbouring existence check used `grep -qi` and did work, so the
step reported "Escrow PDA exists on-chain" and then quietly said nothing useful
about ownership.

The same line also grepped for `lamports:`, which solana-cli 3.1.10 does not
print at all (it prints `Balance: <n> SOL`), so the reported lamport figure was
always the `|| echo "0"` fallback.

Fixed: patterns are now case-insensitive and line-anchored (`grep -i "^owner:"`
— unanchored would also match the hexdump rows), the balance is read from
`Balance:` and named/labelled as SOL, and an ownership mismatch is now `fail`
rather than `warn`, since the assertion can finally reach a verdict. The new
rollover helper carries the same anchoring and a comment recording why.

### Verification

Against a stub reproducing the real CLI output byte-for-byte, and then against
**live read-only devnet reads** (`solana account`, no transactions), under the
scripts' own `set -euo pipefail`:

| case | result |
|---|---|
| real escrow-program account, expecting the escrow program | **FAIL** (owner is the BPF loader) |
| same account, expectation set to its true owner | PASS |
| nonexistent devnet account | FAIL ("not found on-chain") |
| wrong owner (stub) | FAIL |
| empty address, `--skip-setup` | SKIP |
| empty address, normal run | FAIL |
| `ESCROW_PROGRAM` env override | honoured |

The first row is the important one: it proves the assertion can reach a `fail`
verdict on real CLI output — which is exactly what the old pattern could not do.
`bash -n` clean on all three scripts; ShellCheck gate 24 scripts / 0 deferred /
clean.

### Remaining

A devnet *run* of the two rollover scripts, to confirm the assertion passes on a
freshly initialized escrow. The checks above used read-only account reads, not a
full flow.
