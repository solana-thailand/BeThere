//! Counting global allocator for the zero-alloc audits (`alloc_count`,
//! `alloc_count_checkin`). Declaring `mod common;` installs it for that test
//! binary. The counter is process-global, so run with `--test-threads=1`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Incremented on every `alloc` / `alloc_zeroed` and on each growing `realloc`.
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Forwards every call to `System` and counts the allocating ones. `dealloc`
/// is not counted: the audit question is "does this path grow the heap?".
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

pub fn reset_counter() {
    ALLOC_COUNT.store(0, Ordering::Relaxed);
}

pub fn counter_snapshot() -> usize {
    ALLOC_COUNT.load(Ordering::Relaxed)
}

/// Allocations made by `f`, measured after one warmup call. A result of 0 is
/// only meaningful once [`assert_installed`] has passed in the same binary.
pub fn allocs_after_warmup<R>(mut f: impl FnMut() -> R) -> usize {
    std::hint::black_box(f());
    reset_counter();
    let out = f();
    let count = counter_snapshot();
    std::hint::black_box(out);
    count
}

/// Canary before zero: a known allocation must be counted, or every
/// "0 allocs" result from this binary is vacuous.
pub fn assert_installed() {
    let count = allocs_after_warmup(|| Box::new(std::hint::black_box(7u64)));
    assert!(
        count >= 1,
        "counting allocator is not installed: a Box::new counted {count}"
    );
}
