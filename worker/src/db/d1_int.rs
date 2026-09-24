//! Exact integer binds for D1.
//!
//! `D1Type::Integer` holds an `i32`, so `D1Type::Integer(x as i32)` silently
//! wraps any amount, slot or timestamp above 2^31: 2_500_000_000 USDC base
//! units (2,500 USDC) binds as a negative number. Every value crosses into JS
//! as an f64 either way (`worker` 0.8 maps `Integer(i)` to
//! `JsValue::from_f64(i as f64)`), so `D1Type::Real` is byte-identical for
//! in-range values and exact up to 2^53 - 1. Past that, refuse instead of
//! rounding.

use std::fmt;

use worker::d1::D1Type;

/// Largest integer an f64 (a JS number) represents exactly: 2^53 - 1.
pub const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;

/// A value that cannot cross the JS boundary without losing precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntBindError {
    pub field: &'static str,
    pub value: i128,
}

impl fmt::Display for IntBindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let IntBindError { field, value } = self;
        write!(
            f,
            "D1 bind {field}: {value} is outside the exact f64 range (±2^53-1)"
        )
    }
}

impl std::error::Error for IntBindError {}

impl From<IntBindError> for String {
    fn from(e: IntBindError) -> Self {
        e.to_string()
    }
}

/// Bind a signed integer exactly, or fail when it exceeds ±(2^53 - 1).
pub fn int_bind(field: &'static str, value: i64) -> Result<D1Type<'static>, IntBindError> {
    match value.unsigned_abs() <= MAX_SAFE_INTEGER as u64 {
        true => Ok(D1Type::Real(value as f64)),
        false => Err(IntBindError {
            field,
            value: value.into(),
        }),
    }
}

/// Bind an unsigned integer exactly, or fail when it exceeds 2^53 - 1.
pub fn uint_bind(field: &'static str, value: u64) -> Result<D1Type<'static>, IntBindError> {
    match i64::try_from(value) {
        Ok(v) => int_bind(field, v),
        Err(_) => Err(IntBindError {
            field,
            value: value.into(),
        }),
    }
}
