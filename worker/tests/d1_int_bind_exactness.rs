//! Money, slot and timestamp binds must reach D1 exactly.
//!
//! `D1Type::Integer` holds an `i32`. Binding `amount as i32` wraps silently
//! above 2^31: 2,500 USDC (2_500_000_000 base units) was stored as
//! -1_794_967_296. `db::d1_int` binds through f64, which is exact to 2^53 - 1,
//! and refuses anything larger instead of rounding it.

use std::fs;
use std::path::Path;

use event_checkin_worker::db::d1_int::{MAX_SAFE_INTEGER, int_bind, uint_bind};
use worker::d1::D1Type;

fn bound_value(bind: D1Type<'_>) -> f64 {
    match bind {
        D1Type::Real(f) => f,
        other => panic!("expected D1Type::Real, got {other:?}"),
    }
}

#[test]
fn values_past_i32_bind_exactly() {
    let usdc_2500 = 2_500_000_000_u64;
    assert_eq!(
        bound_value(uint_bind("amount", usdc_2500).unwrap()),
        2_500_000_000.0
    );
    let devnet_slot = 3_000_000_000_u64;
    assert_eq!(
        bound_value(uint_bind("slot", devnet_slot).unwrap()),
        3_000_000_000.0
    );
    let after_2038 = 2_200_000_000_i64;
    assert_eq!(
        bound_value(int_bind("block_time", after_2038).unwrap()),
        2_200_000_000.0
    );
    assert_eq!(
        bound_value(int_bind("delta", -2_500_000_000).unwrap()),
        -2_500_000_000.0
    );
}

#[test]
fn safe_integer_boundary_is_inclusive() {
    let max = MAX_SAFE_INTEGER;
    assert_eq!(bound_value(int_bind("x", max).unwrap()) as i64, max);
    assert_eq!(bound_value(int_bind("x", -max).unwrap()) as i64, -max);
    assert_eq!(bound_value(uint_bind("x", max as u64).unwrap()) as i64, max);
}

#[test]
fn values_past_2_pow_53_are_refused_not_rounded() {
    let err = int_bind("credit_ledger.delta", MAX_SAFE_INTEGER + 1).unwrap_err();
    assert_eq!(err.field, "credit_ledger.delta");
    assert_eq!(err.value, i128::from(MAX_SAFE_INTEGER + 1));
    assert!(int_bind("x", -MAX_SAFE_INTEGER - 1).is_err());
    assert!(int_bind("x", i64::MIN).is_err());
    assert!(uint_bind("x", u64::MAX).is_err());
    let msg = String::from(uint_bind("onchain_events.amount", u64::MAX).unwrap_err());
    assert!(msg.contains("onchain_events.amount"), "{msg}");
    assert!(
        !msg.contains("://"),
        "the error redactor would eat this: {msg}"
    );
}

/// Source guard: no bind of a money, slot or timestamp column may go back to
/// a wrapping `as i32` cast.
#[test]
fn no_wrapping_i32_casts_on_money_or_chain_columns() {
    const WIDE: [&str; 6] = ["amount", "delta", "slot", "block_time", "usdc", "thb"];
    let db_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/db");
    let mut offenders = Vec::new();
    let mut scanned = 0;
    let mut stack = vec![db_dir];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            match path.extension().and_then(|e| e.to_str()) {
                _ if path.is_dir() => stack.push(path),
                Some("rs") => {
                    scanned += 1;
                    let code = fs::read_to_string(&path).unwrap();
                    for (n, line) in code.lines().enumerate() {
                        let casts = line.contains("D1Type::Integer(") && line.contains("as i32");
                        let wide = WIDE.iter().any(|w| line.contains(w));
                        if casts && wide {
                            offenders.push(format!(
                                "{}:{}: {}",
                                path.display(),
                                n + 1,
                                line.trim()
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    assert!(scanned > 20, "guard is blind: scanned only {scanned} files");
    assert!(
        offenders.is_empty(),
        "use db::d1_int::{{int_bind, uint_bind}}:\n{}",
        offenders.join("\n")
    );
}
