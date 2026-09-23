//! Interleaved A/B timing for CPU-budget claims (`.plans/030` §3, feeds
//! `.plans/028` and the `.benchmarks/` rules).
//!
//! Native wall time is a proxy for Workers `cpuTime`, never the claim itself.
//! What this helper guarantees is that the proxy is fair:
//!
//! - **Interleaved:** A and B alternate every round, and the lead lane flips
//!   (A,B then B,A), so thermal and load drift hit both lanes alike.
//! - **Fresh input per iteration:** `input(round)` is built outside the timed
//!   region, once per lane, so neither lane gets a warm copy of the other's data.
//! - **`black_box`** on the input and the output, so the work cannot be
//!   hoisted or deleted.
//! - **Median of ratios,** not the ratio of medians. Each round's B/A pair
//!   shares the same moment's load.
//! - **0 ns fails.** A zero reading means the timer is coarser than the work, or
//!   the optimizer removed it. Batch the work inside the closure instead.
//! - **No "p99" below [`P99_MIN_ROUNDS`] rounds.** A p99 over fewer samples is
//!   the max under another name, so the tail is reported as its support.
//!
//! Native only: `std::time::Instant` panics on `wasm32-unknown-unknown`.

use std::fmt;
use std::hint::black_box;
use std::time::Instant;

/// Fewest rounds for which a nearest-rank p99 is reported.
pub const P99_MIN_ROUNDS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    A,
    B,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbError {
    NoRounds,
    /// The two lanes hold different sample counts (only reachable via [`summarize`]).
    LaneLengthMismatch {
        a: usize,
        b: usize,
    },
    /// A lane measured 0 ns in this round.
    ZeroDuration {
        lane: Lane,
        round: usize,
    },
}

impl fmt::Display for AbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRounds => write!(f, "no rounds measured"),
            Self::LaneLengthMismatch { a, b } => {
                write!(f, "lane A has {a} samples, lane B has {b}")
            }
            Self::ZeroDuration { lane, round } => write!(
                f,
                "lane {lane:?} measured 0 ns in round {round}: the timer is coarser than the work \
                 or the work was optimized away; batch it inside the closure"
            ),
        }
    }
}

impl std::error::Error for AbError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    /// n ≥ [`P99_MIN_ROUNDS`]: nearest-rank p99 per lane.
    P99 { a_ns: u64, b_ns: u64 },
    /// n < [`P99_MIN_ROUNDS`]: the sample count and the worst reading per lane.
    Support {
        n: usize,
        a_max_ns: u64,
        b_max_ns: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbReport {
    pub rounds: usize,
    pub a_median_ns: u64,
    pub b_median_ns: u64,
    /// Median over rounds of `b_ns / a_ns`. Below 1.0 means B is faster.
    pub median_ratio: f64,
    pub tail: Tail,
}

/// Run `rounds` interleaved rounds of `a` against `b`, each fed a fresh
/// `input(round)`, and summarize them.
pub fn interleaved<I, Ra, Rb>(
    rounds: usize,
    mut input: impl FnMut(usize) -> I,
    mut a: impl FnMut(I) -> Ra,
    mut b: impl FnMut(I) -> Rb,
) -> Result<AbReport, AbError> {
    let mut a_ns = Vec::with_capacity(rounds);
    let mut b_ns = Vec::with_capacity(rounds);
    for round in 0..rounds {
        let (a_input, b_input) = (input(round), input(round));
        match round % 2 {
            0 => {
                a_ns.push(time_one(&mut a, a_input));
                b_ns.push(time_one(&mut b, b_input));
            }
            _ => {
                b_ns.push(time_one(&mut b, b_input));
                a_ns.push(time_one(&mut a, a_input));
            }
        }
    }
    summarize(&a_ns, &b_ns)
}

fn time_one<I, R>(f: &mut impl FnMut(I) -> R, input: I) -> u64 {
    let input = black_box(input);
    let start = Instant::now();
    let out = f(input);
    let elapsed = start.elapsed();
    black_box(out);
    u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
}

/// Summarize per-round samples; `a_ns[i]` and `b_ns[i]` must share round `i`.
pub fn summarize(a_ns: &[u64], b_ns: &[u64]) -> Result<AbReport, AbError> {
    if a_ns.len() != b_ns.len() {
        return Err(AbError::LaneLengthMismatch {
            a: a_ns.len(),
            b: b_ns.len(),
        });
    }
    let rounds = a_ns.len();
    if rounds == 0 {
        return Err(AbError::NoRounds);
    }
    for (round, (&a, &b)) in a_ns.iter().zip(b_ns).enumerate() {
        match (a, b) {
            (0, _) => {
                return Err(AbError::ZeroDuration {
                    lane: Lane::A,
                    round,
                });
            }
            (_, 0) => {
                return Err(AbError::ZeroDuration {
                    lane: Lane::B,
                    round,
                });
            }
            _ => {}
        }
    }
    let mut ratios: Vec<f64> = a_ns
        .iter()
        .zip(b_ns)
        .map(|(&a, &b)| b as f64 / a as f64)
        .collect();
    ratios.sort_by(f64::total_cmp);
    let mut a_sorted = a_ns.to_vec();
    let mut b_sorted = b_ns.to_vec();
    a_sorted.sort_unstable();
    b_sorted.sort_unstable();
    let tail = match rounds {
        n if n >= P99_MIN_ROUNDS => Tail::P99 {
            a_ns: nearest_rank_p99(&a_sorted),
            b_ns: nearest_rank_p99(&b_sorted),
        },
        n => Tail::Support {
            n,
            a_max_ns: a_sorted[n - 1],
            b_max_ns: b_sorted[n - 1],
        },
    };
    Ok(AbReport {
        rounds,
        a_median_ns: median_u64(&a_sorted),
        b_median_ns: median_u64(&b_sorted),
        median_ratio: median_f64(&ratios),
        tail,
    })
}

/// Sorted, non-empty input. Even n: the mean of the two middles, rounded down.
fn median_u64(sorted: &[u64]) -> u64 {
    let mid = sorted.len() / 2;
    match sorted.len() % 2 {
        1 => sorted[mid],
        _ => sorted[mid - 1] / 2 + sorted[mid] / 2 + (sorted[mid - 1] % 2 + sorted[mid] % 2) / 2,
    }
}

fn median_f64(sorted: &[f64]) -> f64 {
    let mid = sorted.len() / 2;
    match sorted.len() % 2 {
        1 => sorted[mid],
        _ => (sorted[mid - 1] + sorted[mid]) / 2.0,
    }
}

/// Sorted input, n ≥ 1: the value at rank ⌈0.99·n⌉.
fn nearest_rank_p99(sorted: &[u64]) -> u64 {
    let rank = (sorted.len() * 99).div_ceil(100);
    sorted[rank.max(1) - 1]
}
