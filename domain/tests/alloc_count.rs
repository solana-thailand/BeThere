#![cfg(feature = "alloc_count")]
//! Plan 014 Phase 5.4 — Zero-allocation hot-path audit for the wire decode path.
//!
//! Installs a counting global allocator and asserts that the wire decode hot
//! path (`unpack` / `unpack_slice`) performs **zero heap allocations** after a
//! one-time warmup. katgpt-rs measures allocs/call on every hot kernel; this
//! is the equivalent discipline applied to the Phase 1 zero-copy path.
//!
//! ## Why warmup
//!
//! The first BLAKE3 invocation may lazily initialise SIMD-detection state and
//! the test harness itself touches a few `Once` cells. These are one-time
//! costs. The discipline katgpt-rs applies — and we apply here — is to measure
//! the **steady-state** hot path, not the cold start. Each test calls the
//! decoder once to warm up, resets the counter, then measures.
//!
//! ## Audit outcome (measured 2026-06-27, blake3 1.8.5)
//!
//! Initial hypothesis was that `unpack_slice` with multi-chunk payloads
//! (≥64 B) would force blake3's internal sub-tree stack to allocate. The
//! measurement disproved this: blake3 1.8.5 uses `InlineSubCtxStack` (a
//! fixed-size stack array, not a `Vec`), so steady-state allocation count is
//! **0 for every tested shape** — including a 10000-row / 160 KB payload.
//! `cast_slice` / `from_bytes` are pointer reinterprets and do not allocate
//! either. The decode path is genuinely zero-alloc.
//!
//! The constant `0` is therefore asserted for every shape, not just the
//! single-value case. A regression here would mean either a blake3 upgrade
//! that re-introduces heap growth, or new code on the decode path that
//! allocates.
//!
//! ## Required features
//!
//! This test uses `wire::pack` / `wire::unpack`, so it needs `--features wire`
//! to compile. The counting allocator itself is gated behind the
//! `alloc_count` feature. Run with both:
//!
//! ```sh
//! cargo test -p event-checkin-domain \
//!     --features wire,alloc_count \
//!     --test alloc_count -- --nocapture --test-threads=1
//! ```
//!
//! `--test-threads=1` is required: the counter is process-global, so parallel
//! tests would cross-contaminate each other's measurements.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use event_checkin_domain::models::adventure::LevelScore;
use event_checkin_domain::wire;

/// Allocation counter. Incremented by the global allocator on every
/// `alloc` / `alloc_zeroed` / `realloc` (grown) call. Reset by tests via
/// [`reset_counter`] before each measurement.
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Counting wrapper around `System`. Forwards every call but counts the
/// allocating ones. `dealloc` is not counted — we measure *allocation* sites,
/// not frees (the audit question is "does decode grow the heap?").
struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        // SAFETY: forwarding to System with the caller-provided layout.
        // The unsafe block is required by Rust 2024's unsafe-op-in-unsafe-fn.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        // SAFETY: see `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: see `alloc`.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // Only count growths — in-place shrinks/grows don't allocate.
        if new_size > layout.size() {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: see `alloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOC: CountingAllocator = CountingAllocator;

fn reset_counter() {
    ALLOC_COUNT.store(0, Ordering::Relaxed);
}

fn counter_snapshot() -> usize {
    ALLOC_COUNT.load(Ordering::Relaxed)
}

fn make_level_score(i: u32) -> LevelScore {
    LevelScore {
        moves: 5 + (i % 50),
        puzzles_solved: 1 + (i % 5),
        time_seconds: 10 + (i % 600),
        stars: 1 + (i % 3) as u8,
        _pad: [0; 3],
    }
}

#[test]
fn unpack_single_value_is_zero_alloc_after_warmup() {
    let sample = make_level_score(42);
    let bytes = wire::pack(&sample);

    // Warmup: primes blake3 SIMD detection + any one-time lazy state.
    let _ = wire::unpack::<LevelScore>(&bytes).expect("warmup");
    reset_counter();

    let decoded = wire::unpack::<LevelScore>(&bytes).expect("decode");
    let count = counter_snapshot();

    println!(
        "unpack<LevelScore> ({} B payload): alloc_count = {count}",
        std::mem::size_of::<LevelScore>()
    );
    assert_eq!(decoded, &sample, "decoded value must match");
    assert_eq!(
        count, 0,
        "unpack<T> must be zero-alloc after warmup — single Pod values fit in one BLAKE3 chunk"
    );
}

#[test]
fn unpack_slice_empty_is_zero_alloc_after_warmup() {
    let empty: Vec<LevelScore> = Vec::new();
    let bytes = wire::pack_slice(&empty);

    let _ = wire::unpack_slice::<LevelScore>(&bytes).expect("warmup");
    reset_counter();

    let decoded = wire::unpack_slice::<LevelScore>(&bytes).expect("decode");
    let count = counter_snapshot();

    println!("unpack_slice<LevelScore> (0 rows): alloc_count = {count}");
    assert!(decoded.is_empty(), "empty slice must decode to empty");
    assert_eq!(
        count, 0,
        "empty slice must be zero-alloc — hashed body is just header+count, well under one chunk"
    );
}

#[test]
fn unpack_slice_one_row_is_zero_alloc_after_warmup() {
    let one = vec![make_level_score(0)];
    let bytes = wire::pack_slice(&one);

    let _ = wire::unpack_slice::<LevelScore>(&bytes).expect("warmup");
    reset_counter();

    let decoded = wire::unpack_slice::<LevelScore>(&bytes).expect("decode");
    let count = counter_snapshot();

    println!("unpack_slice<LevelScore> (1 row, 16 B payload): alloc_count = {count}");
    assert_eq!(decoded.len(), 1);
    assert_eq!(
        count, 0,
        "1-row slice (16 B payload) fits in one BLAKE3 chunk — must be zero-alloc"
    );
}

#[test]
fn unpack_slice_three_rows_is_zero_alloc_after_warmup() {
    // 3 rows × 16 B = 48 B payload; hashed input = 8 + 4 + 48 = 60 B < 64 B chunk.
    // Still single-chunk; the audit boundary we care about.
    let rows: Vec<LevelScore> = (0..3).map(make_level_score).collect();
    let bytes = wire::pack_slice(&rows);

    let _ = wire::unpack_slice::<LevelScore>(&bytes).expect("warmup");
    reset_counter();

    let decoded = wire::unpack_slice::<LevelScore>(&bytes).expect("decode");
    let count = counter_snapshot();

    println!("unpack_slice<LevelScore> (3 rows, 48 B payload): alloc_count = {count}");
    assert_eq!(decoded.len(), 3);
    assert_eq!(
        count, 0,
        "3-row slice (48 B payload) fits in one BLAKE3 chunk — must be zero-alloc"
    );
}

#[test]
fn unpack_slice_four_rows_is_zero_alloc_after_warmup() {
    // 4 rows × 16 B = 64 B payload; hashed input = 8 + 4 + 64 = 76 B > 64 B chunk.
    // First multi-chunk boundary. Audit measurement (2026-06-27, blake3 1.8.5):
    // 0 allocs — `InlineSubCtxStack` is a fixed-size stack array, not a Vec.
    let rows: Vec<LevelScore> = (0..4).map(make_level_score).collect();
    let bytes = wire::pack_slice(&rows);

    let _ = wire::unpack_slice::<LevelScore>(&bytes).expect("warmup");
    reset_counter();

    let decoded = wire::unpack_slice::<LevelScore>(&bytes).expect("decode");
    let count = counter_snapshot();

    println!(
        "unpack_slice<LevelScore> (4 rows, 64 B payload — multi-chunk boundary): alloc_count = {count}"
    );
    assert_eq!(decoded.len(), 4);
    assert_eq!(
        count, 0,
        "4-row slice — first multi-chunk boundary, confirmed zero-alloc on blake3 1.8.5"
    );
}

#[test]
fn unpack_slice_50_rows_is_zero_alloc_after_warmup() {
    // 50 rows × 16 B = 800 B payload — the realistic Tier 1 batch size
    // (`GET /api/attendees` ~50–200 rows).
    let rows: Vec<LevelScore> = (0..50).map(make_level_score).collect();
    let bytes = wire::pack_slice(&rows);

    let _ = wire::unpack_slice::<LevelScore>(&bytes).expect("warmup");
    reset_counter();

    let decoded = wire::unpack_slice::<LevelScore>(&bytes).expect("decode");
    let count = counter_snapshot();

    println!("unpack_slice<LevelScore> (50 rows, 800 B payload): alloc_count = {count}");
    assert_eq!(decoded.len(), 50);
    assert_eq!(
        count, 0,
        "50-row slice (Tier 1 realistic) must be zero-alloc — confirmed by audit"
    );
}

#[test]
fn unpack_slice_500_rows_is_zero_alloc_after_warmup() {
    // 500 rows × 16 B = 8 KB payload — the GOAT-gate headline size. Documents
    // the realistic upper bound for `GET /api/contacts/audience` (~1k rows
    // capped to 500 per page).
    let rows: Vec<LevelScore> = (0..500).map(make_level_score).collect();
    let bytes = wire::pack_slice(&rows);

    let _ = wire::unpack_slice::<LevelScore>(&bytes).expect("warmup");
    reset_counter();

    let decoded = wire::unpack_slice::<LevelScore>(&bytes).expect("decode");
    let count = counter_snapshot();

    println!("unpack_slice<LevelScore> (500 rows, 8 KB payload): alloc_count = {count}");
    assert_eq!(decoded.len(), 500);
    assert_eq!(
        count, 0,
        "500-row slice (GOAT-gate headline) must be zero-alloc — confirmed by audit"
    );
}

#[test]
fn unpack_slice_10000_rows_stress_is_zero_alloc_after_warmup() {
    // Stress test: 10000 rows × 16 B = 160 KB payload. ~2500 BLAKE3 chunks.
    // Far past any realistic batch size — included to confirm the
    // InlineSubCtxStack does not silently overflow into heap growth for
    // pathological inputs. If this ever fails, the audit conclusion needs
    // revisiting for any payload size at or above the failure point.
    let rows: Vec<LevelScore> = (0..10000).map(make_level_score).collect();
    let bytes = wire::pack_slice(&rows);

    let _ = wire::unpack_slice::<LevelScore>(&bytes).expect("warmup");
    reset_counter();

    let decoded = wire::unpack_slice::<LevelScore>(&bytes).expect("decode");
    let count = counter_snapshot();

    println!(
        "unpack_slice<LevelScore> (10000 rows, 160 KB payload — stress): alloc_count = {count}"
    );
    assert_eq!(decoded.len(), 10000);
    assert_eq!(
        count, 0,
        "10000-row stress test must be zero-alloc — InlineSubCtxStack has capacity for log2(chunks) levels"
    );
}

#[test]
fn repeated_unpack_single_value_stays_zero_alloc() {
    // Regression guard: a second decode in the same process must not allocate
    // either. Catches the case where warmup is insufficient (e.g. lazy init
    // happens on the Nth call, not the 1st).
    let sample = make_level_score(7);
    let bytes = wire::pack(&sample);

    let _ = wire::unpack::<LevelScore>(&bytes).expect("warmup 1");
    let _ = wire::unpack::<LevelScore>(&bytes).expect("warmup 2");
    reset_counter();

    for _ in 0..100 {
        let _ = wire::unpack::<LevelScore>(&bytes).expect("decode");
    }
    let count = counter_snapshot();

    println!("100× unpack<LevelScore>: alloc_count = {count}");
    assert_eq!(
        count, 0,
        "100 repeated decodes must remain zero-alloc — no per-call lazy init"
    );
}

#[test]
fn repeated_unpack_slice_500_rows_stays_zero_alloc() {
    // Regression guard for the slice path: 100 sequential 500-row decodes
    // must all be zero-alloc. Catches the case where the blake3 Hasher leaves
    // behind per-call state that the next call has to clean up.
    let rows: Vec<LevelScore> = (0..500).map(make_level_score).collect();
    let bytes = wire::pack_slice(&rows);

    let _ = wire::unpack_slice::<LevelScore>(&bytes).expect("warmup");
    reset_counter();

    for _ in 0..100 {
        let _ = wire::unpack_slice::<LevelScore>(&bytes).expect("decode");
    }
    let count = counter_snapshot();

    println!("100× unpack_slice<LevelScore> (500 rows): alloc_count = {count}");
    assert_eq!(
        count, 0,
        "100 repeated slice decodes must remain zero-alloc — no per-call heap churn"
    );
}
