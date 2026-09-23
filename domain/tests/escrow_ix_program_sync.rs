//! Drift guard between the escrow program and the pinned instruction data
//! (`.plans/030` §3, memory `onchain-struct-offset-drift`).
//!
//! `bethere-escrow` is outside the workspace, so it cannot be linked here.
//! This reads its `lib.rs` instead and checks, for every
//! `#[instruction(discriminator = N)] pub fn name(ctx, args…)`:
//!   * the fixture's `name` cases start with byte `N`;
//!   * their length is `1 + 8 × args` (every argument is a `u64`/`i64`).
//!
//! `golden_vectors.rs` ties the same fixture to `EscrowIxData::encode`, so a
//! program change that the worker's encoder misses fails one of the two.

use serde::Deserialize;

const PROGRAM_LIB: &str = include_str!("../../bethere-escrow/src/lib.rs");
const FIXTURE: &str = include_str!("fixtures/golden_vectors.json");

#[derive(Debug, PartialEq, Eq)]
struct DeclaredIx {
    name: String,
    discriminator: u8,
    word_args: usize,
}

#[derive(Deserialize)]
struct Fixture {
    escrow_ix_data: Section,
}

#[derive(Deserialize)]
struct Section {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    ix: String,
    hex: String,
}

const MARKER: &str = "#[instruction(discriminator = ";

/// Pull `(name, discriminator, non-ctx arg count)` out of the `#[program]`
/// module. Every argument must be `u64` or `i64`; anything else panics, since
/// the length rule below would then be wrong.
fn declared(src: &str) -> Vec<DeclaredIx> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(at) = rest.find(MARKER) {
        rest = &rest[at + MARKER.len()..];
        let close = rest.find(')').expect("unterminated discriminator attr");
        let discriminator: u8 = rest[..close].trim().parse().expect("u8 discriminator");
        let fn_at = rest.find("pub fn ").expect("attr without a pub fn");
        let sig = &rest[fn_at + "pub fn ".len()..];
        let open = sig.find('(').expect("fn without params");
        let name = sig[..open].trim().to_owned();
        let params_end = sig.find(") ->").expect("fn without a return type");
        let word_args = sig[open + 1..params_end]
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty() && !p.contains("Ctx<"))
            .inspect(|p| {
                let ty = p.rsplit(':').next().unwrap_or_default().trim();
                assert!(
                    matches!(ty, "u64" | "i64"),
                    "{name}: arg {p:?} is not u64/i64; teach this guard its width"
                );
            })
            .count();
        out.push(DeclaredIx {
            name,
            discriminator,
            word_args,
        });
    }
    out
}

#[test]
fn parser_reads_the_program() {
    let ixs = declared(PROGRAM_LIB);
    assert_eq!(ixs.len(), 9, "expected 9 escrow instructions: {ixs:?}");
    assert_eq!(
        ixs[0],
        DeclaredIx {
            name: "create_event".into(),
            discriminator: 0,
            word_args: 4
        }
    );
}

/// Proves the guard can fail: a planted drift is reported.
#[test]
fn parser_sees_a_planted_drift() {
    let drifted = PROGRAM_LIB.replacen(
        "#[instruction(discriminator = 1)]",
        "#[instruction(discriminator = 9)]",
        1,
    );
    let deposit = declared(&drifted)
        .into_iter()
        .find(|ix| ix.name == "deposit")
        .expect("deposit is declared");
    assert_eq!(deposit.discriminator, 9);
}

#[test]
fn fixture_matches_program_declarations() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    for ix in declared(PROGRAM_LIB) {
        let cases: Vec<&Case> = fixture
            .escrow_ix_data
            .cases
            .iter()
            .filter(|c| c.ix == ix.name)
            .collect();
        assert!(!cases.is_empty(), "{}: no pinned case", ix.name);
        for case in cases {
            assert_eq!(
                &case.hex[..2],
                format!("{:02x}", ix.discriminator),
                "{}: fixture discriminator differs from the program",
                ix.name
            );
            assert_eq!(
                case.hex.len() / 2,
                1 + 8 * ix.word_args,
                "{}: fixture length differs from the program's {} args",
                ix.name,
                ix.word_args
            );
        }
    }
}
