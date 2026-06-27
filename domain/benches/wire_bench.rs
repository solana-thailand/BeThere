//! Plan 014 — Phase 1.7: GOAT-gate microbench.
//!
//! Measures JSON vs. zero-copy binary (BLAKE3-committed) encode **and** decode
//! for a batch payload. The single binary the audit designated as the
//! GOAT-gate arbiter: clear both thresholds → promote `wire` to default-on; miss
//! either → kill the feature and log a negative result.
//!
//! ## Thresholds (set in `.plans/014_wire_audit.md`, Task 1.7)
//!
//! - **Decode speed:** binary must be ≥ **3×** faster than JSON decode.
//! - **Payload size:** binary must be ≥ **40%** smaller than JSON.
//!
//! ## Important caveat — host vs. wasm
//!
//! `criterion` is x86_64-only (`instant`/`std::time` are not on `wasm32`).
//! The numbers below are a **host** measurement. They answer the GOAT question
//! on the *worker* side (where JSON encode runs) and serve as a *proxy* for the
//! *frontend* decode side. The real frontend win is likely **larger** than
//! reported here, because:
//!
//! - wasm JSON parsers are typically 1.3–2× slower than native ones (the JS
//!   bridge, the lack of vectorized string scanning),
//! - `bytemuck::cast_slice` on wasm32 is a literal pointer reinterpret — cost
//!   is one bounds check, regardless of payload size.
//!
//! So a host-side "3× decode win" is a **conservative** lower bound for wasm.
//! If the host bench clears the gate, the wasm bench would clear it by more.
//!
//! ## Run
//!
//! ```sh
//! cargo bench -p event-checkin-domain --features wire --bench wire_bench
//! ```
//!
//! Output is written under `domain/target/criterion/`. The `decode/json` and
//! `decode/wire` median timings are the GOAT-gate inputs. See
//! `.plans/014_wire_audit.md` for the decision matrix.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use event_checkin_domain::models::adventure::LevelScore;
use event_checkin_domain::wire;

/// Row counts to benchmark. 50/200/1000 span the audit's TIER 1 (`GET /api/events`
/// ~50, `GET /api/attendees` ~200) and TIER 1B (`GET /api/contacts/audience`
/// ~1000) ranges. 500 is the headline number the audit called out.
const ROW_COUNTS: &[usize] = &[50, 200, 500, 1000];

/// Build a deterministic fixture so bench-to-bench numbers are comparable.
///
/// `i` is mixed into every field so the JSON encoder can't take a fast path on
/// repeated values. Field magnitudes mirror real adventure scores (moves < 200,
/// time < 600s, stars 1-3).
fn fixture(count: usize) -> Vec<LevelScore> {
    (0..count)
        .map(|i| {
            let i = i as u32;
            LevelScore {
                moves: 5 + (i % 50),
                puzzles_solved: 1 + (i % 5),
                time_seconds: 10 + (i % 600),
                stars: 1 + (i % 3) as u8,
                _pad: [0; 3],
            }
        })
        .collect()
}

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");
    group.plot_config(criterion::PlotConfiguration::default());

    for &count in ROW_COUNTS {
        let data = fixture(count);
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::new("json", count), &data, |b, data| {
            b.iter(|| {
                let bytes = serde_json::to_vec(black_box(data)).expect("json encode");
                black_box(bytes);
            });
        });

        group.bench_with_input(BenchmarkId::new("wire", count), &data, |b, data| {
            b.iter(|| {
                let bytes = wire::pack_slice(black_box(data));
                black_box(bytes);
            });
        });
    }
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");
    group.plot_config(criterion::PlotConfiguration::default());

    for &count in ROW_COUNTS {
        let data = fixture(count);
        let json_bytes = serde_json::to_vec(&data).expect("json encode");
        let wire_bytes = wire::pack_slice(&data);
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::new("json", count), &json_bytes, |b, bytes| {
            b.iter(|| {
                let v: Vec<LevelScore> =
                    serde_json::from_slice(black_box(bytes)).expect("json decode");
                black_box(v);
            });
        });

        group.bench_with_input(BenchmarkId::new("wire", count), &wire_bytes, |b, bytes| {
            b.iter(|| {
                let v: &[LevelScore] = wire::unpack_slice(black_box(bytes)).expect("wire decode");
                black_box(v);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode);
criterion_main!(benches);
