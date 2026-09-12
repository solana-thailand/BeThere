# 073 — The e2e signing helper takes the private key as a command-line argument

Status: **implemented locally** — the secret is out of argv and verified by
probe. One step remains: a real devnet run, since no static check can prove the
four scripts still transact end-to-end. Found while closing #072 item 7.

## What happens

`scripts/e2e/sign_and_submit.py` reads its inputs from `sys.argv`:

```python
tx_b64 = str(sys.argv[1])
keypair_json = str(sys.argv[2])   # <- the raw secret key
rpc_url = str(sys.argv[3])
```

`argv[2]` is not a path — it is the **contents** of a Solana keypair file, i.e.
the 64-byte secret key as a JSON array. Four scripts pass it that way:

| caller | line | what it passes |
|---|---|---|
| `scripts/e2e/test_lifecycle.sh` | 40 | `"$(cat ~/.config/solana/id.json)"` inline |
| `scripts/e2e/test_escrow_devnet.sh` | 88 | `$ORG_KEYPAIR_JSON` / `$ATT_KEYPAIR_JSON` |
| `scripts/e2e/test_rollover_devnet.sh` | 88 | same |
| `scripts/e2e/test_rollover_full_lifecycle.sh` | 79 | same |

A process's argument vector is readable by any other local user. Verified on
this machine rather than assumed: a canary string passed as argv to a child
process shows up in `ps -Ao args` while the process lives. So for the duration
of every signing call, the secret key is world-readable locally, and it lands
in anything that samples the process table — `procs`, `btm`, an activity
monitor, a CI log that dumps `ps`, a crash reporter.

`~/.config/solana/id.json` is the operator's **default** keypair, not a
throwaway. It is the same file the `solana` CLI uses for whatever cluster it is
configured against, so the exposure is not confined to devnet test wallets.

This is the same defect class #065 closed for the production deploy fallback,
where the Cloudflare OAuth token was interpolated into a `python3 -c` program
and thereby into argv. That fix moved the secrets to environment variables;
this call site was not part of that issue and kept the old shape.

## Fix as implemented

`scripts/e2e/sign_and_submit.py` now prefers an environment channel and reads
the key itself:

- `SIGNER_KEYPAIR_PATH` — a **path**, so the secret enters neither argv nor the
  environment. The helper opens the file. An unreadable path fails with a clear
  `ERROR: cannot read SIGNER_KEYPAIR_PATH=…` instead of a traceback.
- `SOLANA_RPC_URL` — the endpoint, which may carry `?api-key=`. Environment
  rather than argv because argv is world-readable locally.
- `argv[1]` is still the unsigned TX; it is not secret.
- The legacy positional form still works but prints a `WARNING:` to stderr
  naming this issue, so nothing breaks mid-migration while new callers are
  discouraged.

All four callers migrated. The three identical `sign_and_submit_tx` wrappers
now take a path and set both env vars; `test_lifecycle.sh`'s inline wrapper no
longer `cat`s the keypair. The `*_KEYPAIR_JSON` variables were renamed to
`*_KEYPAIR_PATH` (51 references across three scripts) because a variable named
`_JSON` holding a path is a lie, and the 19 `$(cat …)` assignments became plain
path assignments.

## Verification

The probe was validated in both directions, which matters — the first attempt
produced a false "clean" result because `ps` is aliased to `procs` and `grep`
to `ugrep` on this machine, and both had failed rather than matched. Re-run with
`/bin/ps -Ao args` and `/usr/bin/grep -F`:

- **New form:** key material absent from `ps` while signing is in flight.
- **Legacy form:** key material present — so the probe genuinely detects argv
  exposure, and the clean result above is meaningful rather than vacuous.
- **Signing still correct:** against a stub RPC, the submitted transaction's
  signature verifies against the message with the keypair's public key, so the
  refactor did not break the signing path.
- **Error paths:** missing TX, missing signer, missing RPC, unreadable keypair
  and a directory-instead-of-file each produce a single clear `ERROR:` line.
  `~` in `SIGNER_KEYPAIR_PATH` expands correctly.
- `bash -n` and the repo ShellCheck gate (24 scripts, 0 deferred) pass.

**Still required:** a devnet run of all four scripts. Everything above used a
stub RPC and a throwaway keypair.

## Knock-on: #072 item 7 is unblocked and done

With the RPC URL no longer in argv, `test_escrow_devnet.sh` now uses the
authenticated Helius devnet endpoint when `HELIUS_API_KEY` is present in
`worker/.dev.vars`, instead of the rate-limited public one. `HELIUS_API_KEY` is
live, so the `# shellcheck disable=SC2034` is gone.

Guarded and verified: an explicit `RPC_URL` still wins; with no key the public
endpoint is used; and the banner prints
`https://devnet.helius-rpc.com/?api-key=<redacted>` — confirmed by grepping the
script's output for the real 36-character key and finding no match.

One bug caught by running it rather than reading it: the first cut printed
`$RPC_LABEL` in the banner 14 lines **before** it was assigned, which `set -u`
turns into `RPC_LABEL: unbound variable`. ShellCheck cannot see ordering like
that. The `.dev.vars` read now sits above the banner.

## Why it was filed rather than fixed inside #072

#072 was about dead assertions. Fixing this properly means changing
`sign_and_submit.py`'s input contract and updating four callers, which is its
own change with its own verification. Doing it inside #072 would have mixed two
unrelated diffs.

It also **blocks a real improvement**: #072 item 7 wanted to point `RPC_URL` at
the authenticated Helius devnet endpoint instead of the rate-limited public
one, since the flaky confirmations in these scripts come from public-RPC
throttling. That URL carries `?api-key=…` and is forwarded as `argv[3]` through
the same call, so the upgrade was written and then reverted rather than
introduce a second secret into argv. `HELIUS_API_KEY` stays read-but-unused in
`test_escrow_devnet.sh` with a `# shellcheck disable=SC2034` naming this issue.

## Proposed fix

Move both secrets to an env channel, which `sign_and_submit.py` already has a
precedent for (`CONFIRM_TIMEOUT` comes from `os.environ`):

1. Read the keypair from `SIGNER_KEYPAIR_PATH` (a **path**, so the secret never
   enters the environment either — an env var is better than argv but is still
   inherited by children and visible in `/proc` on Linux). Have the helper open
   and read the file itself.
2. Read the RPC from `SOLANA_RPC_URL`, falling back to `argv[3]` so existing
   callers keep working during the transition.
3. Keep `argv[1]` (the unsigned TX) as-is — it is not secret.
4. Update the four callers to pass the keypair path they already have
   (`$ATTENDEE_KEYPAIR`, `~/.config/solana/id.json`) instead of `cat`-ing it.
5. Then unblock #072 item 7 and delete the `SC2034` suppression.

## Resolved — `python3 -c` interpolation in `e2e_devnet_test.sh`

This was deferred when the issue was first written, on the grounds that every
interpolated value there is a path, an address or base64, so no secret was
exposed. That still holds — but the conversion has now been done, and it
uncovered a separate defect filed as
[074](074_attendee_deposit_decode_offsets_stale.md).

All eight interpolation sites are gone. The file now contains no `python3 -c`
with an interpolated value; the one remaining `python3 -c` is the constant
`import solders` probe. Values reach Python as `argv`, via a quoted heredoc —
the pattern established in `tests/integration/run.sh`:

| site | change |
|---|---|
| `fnv1a_hash()` | slug passed as argv |
| `sign_tx()` | both paths as argv |
| `send_tx()` | signed blob and RPC URL as argv |
| `derive_pda()` | **deleted** — defined, never called; it interpolated a caller-supplied *Python program* (`$seeds_py`), the worst shape in the file |
| deposit PDA derivation ×2 | folded into one `derive_deposit_pda()` helper |
| deposit decode ×3 | folded into one `decode_deposit()` helper (see #074) |

**A claim in the original version of this section was wrong.** It said a value
containing a quote or backtick "would break or execute". It would break, but it
would **not** execute: the values arrive via parameter expansion (`'$1'`,
`'$data_b64'`), and bash does not re-expand the *result* of a parameter
expansion. Checked directly — a slug of `$(echo INJECTED)` reaches Python as the
literal text, not as a command. So this was robustness only, as the deferral
assumed; there was no injection vector.

What it did do is break: a slug containing an apostrophe produced
`SyntaxError: unterminated string literal`. The converted helpers handle
`it's-a-quote`, `` back`tick` `` and `$(echo pwned)` and match the Rust
`derive_on_chain_event_id` (`worker/src/handlers/deposit/mod.rs:48`) on all of
them.

### Verification

- `fnv1a_hash` — matches an independent FNV-1a implementation on four inputs
  including shell-hostile ones; the old form raises `SyntaxError` on the same
  input.
- `derive_deposit_pda` — matches an independent derivation; seeds cross-checked
  against five Rust sites, all `[b"deposit", escrow, attendee]`; a bad address
  now exits non-zero instead of returning an empty string.
- `sign_tx` — signs a real unsigned transaction; `Transaction.verify()` passes
  against the message. Keypair bytes **absent** from `/bin/ps -Ao args` during
  signing, with a positive control (a key deliberately placed in argv) that the
  same probe **does** detect.
- `send_tx` — against a stub RPC, a blob containing `'`, `"`, `$(…)` round-trips
  verbatim with `encoding=base64` and `skipPreflight=True` preserved.
- ShellCheck gate: 24 scripts, 0 deferred, clean. `bash -n` passes.

The argv probe needed two corrections before it meant anything: the first
version matched its own `grep` process (the pattern sat in grep's argv), so it
reported a leak that was not there. Pattern moved to a file and the probe's own
processes filtered out. Same lesson as the earlier false clean — a probe has to
be checked in both directions before its output counts.
