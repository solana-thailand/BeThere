# 169: The frontend size baseline is past the warn line, and `size_budget_guards` is red on develop

**Status:** open (found 2026-09-29, session `event-checkin-ba`). `develop` fails `cargo test --workspace` until this is resolved. It is not pushed yet.

## What fails

```
worker/tests/size_budget_guards.rs:247  frontend_budget_thresholds_are_coherent
BASELINE_BYTES (1965134) is already past the warn line — a gate that is amber
on a clean tree is a gate nobody reads
```

The warn line is `CEILING_BYTES × WARN_PCT` = 2,097,152 × 90% = **1,887,436**
bytes br4. `e1161fdd` (the EN+TH switch, Wave 1 Task 2) moved the baseline to
1,965,134 to account for +102 KB of i18n. It did not run the workspace suite
that holds this guard. The frontend gate itself still passes: it prints
"past the warn line … still shippable".

## Measured first load (2026-09-29, `frontend_size_budget.sh`, br4)

- Total: 1,970,397. That is 83 KB over the warn line and 127 KB under the
  ceiling.
- The wasm alone is 1,843,269 (94%). Everything else (JS glue, snippets and 23
  stylesheets) is about 127 KB.

## Options

1. **Shrink the wasm by ≥ 83 KB br.** This is the recommendation.
   Candidates, none of them measured yet:
   - the remaining `t!` sites (see the `leptos-i18n-traps` memory: `t!` per
     site bloats and `locale::tr` cut it);
   - route-level code splitting for admin, scanner and dashboard, which an
     attendee never runs.
2. **Raise `CEILING_BYTES`.** `.size-budget` says this is allowed but must not
   be quiet. It is a product call: 2 MiB ≈ 5.6 s at 3 Mbps.
   **Owner decision.**
3. Raise `WARN_PCT` or edit the guard. **Rejected:** that silences the gate
   instead of answering it.

## Why it matters now

The GOAT-hardening package adds attendee UI (P2-a inline QR, P2-b meta rows,
P2-d cookie banner, P3-a FAQ). Each addition moves the first load toward the
2 MiB fail line, so the budget is now the binding constraint for that work.
P1-1 (landing cuts) and P3-c (jsQR lazy-load) help only a little: jsQR is not
in the first-load set that the gate measures.
