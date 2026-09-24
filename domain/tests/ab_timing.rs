//! `domain::ab_timing` (.plans/030 §3): interleaving, fresh input, the 0 ns
//! failure, median of ratios, and the p99-vs-support tail rule.

use std::cell::RefCell;
use std::hint::black_box;

use event_checkin_domain::ab_timing::{
    AbError, Lane, P99_MIN_ROUNDS, Tail, interleaved, summarize,
};

#[test]
fn lanes_alternate_and_the_lead_flips_each_round() {
    let order = RefCell::new(Vec::new());
    interleaved(
        4,
        |round| round,
        |round| {
            order.borrow_mut().push((Lane::A, round));
            spin(round)
        },
        |round| {
            order.borrow_mut().push((Lane::B, round));
            spin(round)
        },
    )
    .expect("spin work is measurable");
    assert_eq!(
        order.into_inner(),
        vec![
            (Lane::A, 0),
            (Lane::B, 0),
            (Lane::B, 1),
            (Lane::A, 1),
            (Lane::A, 2),
            (Lane::B, 2),
            (Lane::B, 3),
            (Lane::A, 3),
        ]
    );
}

#[test]
fn each_lane_gets_its_own_input_every_round() {
    let built = RefCell::new(Vec::new());
    interleaved(
        3,
        |round| {
            built.borrow_mut().push(round);
            vec![round as u64; 256]
        },
        |v: Vec<u64>| v.iter().copied().map(spin_n).sum::<u64>(),
        |v: Vec<u64>| v.iter().copied().map(spin_n).sum::<u64>(),
    )
    .expect("measurable");
    assert_eq!(built.into_inner(), vec![0, 0, 1, 1, 2, 2]);
}

#[test]
fn slower_lane_b_reads_as_ratio_above_one() {
    let report = interleaved(31, |_| 2_000u64, spin_n, |n| spin_n(n * 8)).expect("measurable");
    assert_eq!(report.rounds, 31);
    assert!(
        report.median_ratio > 1.5,
        "8x the work read as {:.2}x",
        report.median_ratio
    );
    assert!(report.b_median_ns > report.a_median_ns);
}

#[test]
fn zero_reading_fails_with_lane_and_round() {
    assert_eq!(
        summarize(&[10, 10, 10], &[10, 0, 10]),
        Err(AbError::ZeroDuration {
            lane: Lane::B,
            round: 1
        })
    );
    assert_eq!(
        summarize(&[0], &[5]),
        Err(AbError::ZeroDuration {
            lane: Lane::A,
            round: 0
        })
    );
}

#[test]
fn empty_and_mismatched_lanes_fail() {
    assert_eq!(summarize(&[], &[]), Err(AbError::NoRounds));
    assert_eq!(
        summarize(&[1, 2], &[1]),
        Err(AbError::LaneLengthMismatch { a: 2, b: 1 })
    );
    assert_eq!(
        interleaved(0, |_| (), |()| (), |()| ()),
        Err(AbError::NoRounds)
    );
}

#[test]
fn median_of_ratios_pairs_each_round() {
    // The load moves both lanes 100x across rounds; B stays 2x A in each.
    let report = summarize(&[100, 1_000, 10_000], &[200, 2_000, 20_000]).expect("valid");
    assert_eq!(report.median_ratio, 2.0);
    // One B round hit by a load spike moves neither the medians nor the ratio.
    let report = summarize(&[100, 100, 100], &[100, 100, 10_000]).expect("valid");
    assert_eq!(report.median_ratio, 1.0);
    assert_eq!((report.a_median_ns, report.b_median_ns), (100, 100));
    // Where they disagree: ratios 4, 0.667, 0.9 → median 0.9 (B faster in the
    // paired rounds), while the unpaired medians 300 vs 400 read B as slower.
    let report = summarize(&[100, 300, 500], &[400, 200, 450]).expect("valid");
    assert_eq!(report.median_ratio, 0.9);
    assert_eq!((report.a_median_ns, report.b_median_ns), (300, 400));
}

#[test]
fn even_count_medians_average_the_middle_pair() {
    let report = summarize(&[1, 3, 5, 7], &[2, 6, 10, 15]).expect("valid");
    assert_eq!(report.a_median_ns, 4);
    assert_eq!(report.b_median_ns, 8);
    // ratios 2, 2, 2, 15/7 → sorted middle pair (2, 2)
    assert_eq!(report.median_ratio, 2.0);
    // u64 halves do not overflow near the top of the range
    let big = summarize(&[u64::MAX, u64::MAX], &[1, 1]).expect("valid");
    assert_eq!(big.a_median_ns, u64::MAX);
}

#[test]
fn below_the_floor_the_tail_is_support_not_p99() {
    let n = P99_MIN_ROUNDS - 1;
    let a: Vec<u64> = (1..=n as u64).collect();
    let b: Vec<u64> = a.iter().map(|x| x * 2).collect();
    let report = summarize(&a, &b).expect("valid");
    assert_eq!(
        report.tail,
        Tail::Support {
            n,
            a_max_ns: n as u64,
            b_max_ns: 2 * n as u64
        }
    );
}

#[test]
fn at_the_floor_the_tail_is_nearest_rank_p99() {
    let a: Vec<u64> = (1..=200).collect();
    let b: Vec<u64> = a.iter().map(|x| x + 1_000).collect();
    let report = summarize(&a, &b).expect("valid");
    // rank ⌈0.99·200⌉ = 198
    assert_eq!(
        report.tail,
        Tail::P99 {
            a_ns: 198,
            b_ns: 1_198
        }
    );
    let exactly: Vec<u64> = (1..=P99_MIN_ROUNDS as u64).collect();
    let report = summarize(&exactly, &exactly).expect("valid");
    assert_eq!(report.tail, Tail::P99 { a_ns: 99, b_ns: 99 });
}

#[test]
fn error_text_names_the_fix() {
    let text = AbError::ZeroDuration {
        lane: Lane::A,
        round: 3,
    }
    .to_string();
    assert!(text.contains("round 3") && text.contains("batch"), "{text}");
}

fn spin(seed: usize) -> u64 {
    spin_n(64 + seed as u64)
}

fn spin_n(n: u64) -> u64 {
    (0..black_box(n)).fold(0u64, |acc, i| {
        black_box(acc.wrapping_mul(31).wrapping_add(i))
    })
}
