# 064 — Architecture Refactor + Performance Optimization

## What Happened

Continued from conversation thread "Event Checkin Code Architecture Assessment". Three-phase effort:

### Phase 1-2: File Splitting + TX Builder DRY (Already Committed)
- Split 3 monolithic files (6113 lines) into 10 focused submodules
- Introduced `EscrowCtx`, `acct_sw/acct_w/acct_r`, `finalize_tx`
- Committed as `23a84ce` on main
- 47 tests passing, zero diagnostics

### Phase 2.5: Rust-Analyzer False Positives
- RA reported `expected String, found str` at `register.rs:203` and `register.rs:359`
- `cargo check`, `cargo clippy`, `cargo check --message-format=json` all show **zero errors**
- `Option<String>::clone().unwrap_or_default()` returns `String` — provably correct
- Confirmed as stale RA cache bug (known with edition 2024)
- Workaround: close/reopen file in Zed, or restart RA server

### Phase 3: Performance Audit (Findings Documented)
Two sub-agents audited all 30+ source files. Key findings:
- **H1** Walk-in Sheets sync blocks response (~1s) — should use `wait_until`
- **H3** Full KV scan to count deposits (~200ms) — should use counter key
- **H5** `cancel_status` two sequential KV scans — should use `join!`
- **B2** Sheets API reads 26 cols when ~18 needed — 30-40% bandwidth waste
- **B4** No Cache-Control on public endpoints — edge cache misses
- **C1** Claim lock TOCTOU race — double-mint risk
- **C5** Quiz + adventure KV checks sequential — should use `join!`

## Plan/Code/Test

- Issue: `.issues/022_architecture_refactor_perf_optimization.md`
- Implementation: Phase 3 items, priority order in issue doc
- Tests: `cargo test -p event-checkin-worker` (47 existing)

## Reflection

- The RA false positive wasted investigation time. Lesson: always trust `cargo check` over RA diagnostics.
- The perf audit revealed the walk-in handler (H1) as the single biggest win — ~1s saved per registration.
- The `cancel_status` join (H5) and quiz/adventure join (C5) are trivial changes with guaranteed savings.

## Remain Work

- Implement Phase 3 items H1, H2, H3, H4, H6, H7, H8, B1, B2, B3, B5, C1, C2, C3, C4 (see issue 022)
- ✅ H5 — `cancel_status` two KV scans now use `futures::join!`
- ✅ C5 — Quiz + adventure KV checks now use `futures::join!`
- ✅ C6 — `SolanaConfig::full_rpc_url()` replaces 12 identical `format!` blocks
- ✅ B4 — `Cache-Control` middleware on public endpoints (60s/120s), `no-store` on auth
- Update issue 022 status as each item is completed
- Run tests after each batch of changes

## Issues Ref

- `.issues/022_architecture_refactor_perf_optimization.md`

## How to Dev/Test

```bash
# Build check
cargo check --workspace

# Lint
cargo clippy --workspace --all-targets

# Test
cargo test -p event-checkin-worker

# Clean verify
cargo check -p event-checkin-worker --message-format=json | python3 -c "
import sys, json
for line in sys.stdin:
    try:
        msg = json.loads(line)
        if msg.get('reason') == 'compiler-message':
            level = msg.get('message', {}).get('level', '')
            text = msg.get('message', {}).get('message', '')
            if level in ('error', 'warning'):
                print(f'{level}: {text}')
    except: pass
"
```
