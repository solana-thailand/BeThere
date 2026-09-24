# Benchmark records

One numbered file per rung: a single measured claim, such as "check-in
`cpuTime` on staging, before and after W12". Rules adopted for `.plans/028` M1
from the riir ladder (`docs/gist_rs_study.md`). Checked by
`scripts/verify/bench_records.py`, which runs in CI.

## Rules

1. **Allocate the number.** Run `python3 scripts/verify/numbering_gate.py --next .benchmarks`,
   then create `.benchmarks/NNN_<slug>.md`. One rung per file. A later rung is a
   new file, never an edit to an old number.
2. **The correctness gate is green first.** `Gate:` names the command that
   proves the measured code is correct at this posture, and it says `exit 0`.
   No green gate means no number.
3. **One session, interleaved.** Put the lanes of a comparison (A vs B) in the
   same session, alternating A, B, A, B, and do not run them on different days.
   `Lanes:` says `<A> vs <B>, interleaved`, or `single`. For native timings,
   `event_checkin_domain::ab_timing::interleaved` does the alternation, the
   median of ratios and the tail rule.
4. **Write down the load.** `Load:` records the box or environment (for
   example "staging, 1 isolate, idle laptop" or "native --release, 3 peers
   building"). The gate for Workers CPU is `wrangler tail` `cpuTime` on
   staging. Native wall time is only a proxy.
5. **Retract in place.** When a number turns out wrong, set
   `Status: retracted: <why>` in the record itself and keep the body. A
   commit message is not a record.
6. **Quote numbers by citation.** A number in a plan, issue or doc cites
   `.benchmarks/NNN`. The checker fails on a citation that doesn't resolve,
   and on a citation of a retracted record unless the line says so.
7. **n < 100 means no "p99".** Report the tail support (the n and the max)
   instead.

## Header

```text
# NNN — <title>
Status: valid | retracted: <why> | superseded by NNN
Rung: <what is measured> (<plan ref, e.g. 028 M1>)
Session: <session-name>, <unix-epoch>
Commit: <sha>
Gate: `<correctness command>` exit 0
Lanes: single | <A> vs <B>, interleaved
Load: <box / environment>

## Method
## Result
## Notes
```
