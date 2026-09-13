//! Money formatting — one canonical renderer per currency.
//!
//! The frontend previously carried four independent `format_usdc`
//! implementations that disagreed with each other: two took the atomic 6-decimal
//! unit (one truncating, one rounding through `f64`), one took an `f64` and
//! guessed the unit from magnitude (`if val > 1000.0 { val / 1_000_000.0 }`), and
//! one prefixed a `$`. Same name, four semantics — the classic way a display bug
//! reaches production on one page while its sibling page looks fine.
//!
//! This module is the single implementation. Callers append their own suffix
//! (`" USDC"`, `"$"`) so the numeric rendering stays identical everywhere.

/// Format an atomic USDC amount (1 USDC = 1_000_000 units) with 2 decimals:
/// `15_000_000` → `"15.00"`, `1_500_000` → `"1.50"`.
///
/// Sub-cent remainders are **truncated, not rounded** (`1_005_000` → `"1.00"`),
/// so a displayed balance can never overstate what is actually held. Integer
/// math throughout — no `f64` rounding drift, and no decimal crate in the WASM
/// bundle.
pub fn format_usdc(atomic_usdc: u64) -> String {
    let whole = atomic_usdc / 1_000_000;
    let cents = (atomic_usdc % 1_000_000) / 10_000;
    format!("{whole}.{cents:02}")
}

#[cfg(test)]
mod tests {
    use super::format_usdc;

    #[test]
    fn zero_renders_two_decimals() {
        assert_eq!(format_usdc(0), "0.00");
    }

    #[test]
    fn exact_usdc_has_no_remainder() {
        assert_eq!(format_usdc(25_000_000), "25.00");
    }

    #[test]
    fn fractional_usdc_keeps_cents() {
        assert_eq!(format_usdc(1_500_000), "1.50");
    }

    #[test]
    fn sub_cent_is_truncated_not_rounded() {
        assert_eq!(format_usdc(1_005_000), "1.00");
        assert_eq!(format_usdc(99), "0.00");
    }

    #[test]
    fn cents_below_ten_are_zero_padded() {
        assert_eq!(format_usdc(1_010_000), "1.01");
    }
}
