//! Decimal USDC ↔ atomic-unit conversion (1 USDC = 1_000_000 atomic units).
//!
//! These are the only conversions from a decimal USDC amount to atomic units.
//! `(x * 1_000_000.0) as u64` truncates, and most decimal fractions are not
//! exact in f64, so `2.01` became `2_009_999` (issue 146).

/// Atomic units per whole USDC (6 decimals).
pub const USDC_ATOMIC_PER_UNIT: u64 = 1_000_000;

/// Fraction digits USDC carries.
pub const USDC_DECIMALS: usize = 6;

/// Parse a decimal USDC string (`"2"`, `"2.01"`, `"0.000001"`, `".5"`) into
/// atomic units exactly, with integer arithmetic only.
///
/// Returns `None` for an empty string, a sign, an exponent, more than
/// [`USDC_DECIMALS`] fraction digits, or a value above `u64::MAX`.
pub fn parse_usdc_atomic(input: &str) -> Option<u64> {
    let text = input.trim();
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (text, ""),
    };
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if fraction.len() > USDC_DECIMALS {
        return None;
    }
    if !whole
        .bytes()
        .chain(fraction.bytes())
        .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole_units = match whole {
        "" => 0,
        digits => digits.parse::<u64>().ok()?,
    };
    let fraction_units = fraction
        .bytes()
        .chain(std::iter::repeat(b'0'))
        .take(USDC_DECIMALS)
        .fold(0u64, |acc, digit| acc * 10 + u64::from(digit - b'0'));
    whole_units
        .checked_mul(USDC_ATOMIC_PER_UNIT)?
        .checked_add(fraction_units)
}

/// Convert a float UI amount (for example Helius `tokenAmount`) to atomic
/// units, rounding to nearest.
///
/// Rounding is exact for every atomic value up to far beyond the 1,000 USDC
/// deposit cap: the f64 error there is ~1e-7 atomic units, well under 0.5.
/// Returns `None` for NaN, infinities, negatives and values past `u64::MAX`.
pub fn usdc_ui_to_atomic(ui_amount: f64) -> Option<u64> {
    let atomic = (ui_amount * USDC_ATOMIC_PER_UNIT as f64).round();
    match atomic.is_finite() && atomic >= 0.0 && atomic < u64::MAX as f64 {
        true => Some(atomic as u64),
        false => None,
    }
}

/// Smallest non-zero event deposit the form accepts: 0.01 USDC.
pub const USDC_MIN_DEPOSIT_ATOMIC: u64 = USDC_ATOMIC_PER_UNIT / 100;

/// Largest event deposit (SEC-003): 1,000 USDC. The worker enforces it too.
pub const USDC_MAX_DEPOSIT_ATOMIC: u64 = 1_000 * USDC_ATOMIC_PER_UNIT;

/// Verdict on a USDC deposit field, shared by the form's live hints and its
/// submit path so the two cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsdcDepositCheck {
    /// Blank or zero: no USDC deposit.
    Empty,
    /// Not a plain decimal with at most [`USDC_DECIMALS`] fraction digits.
    Invalid,
    /// Positive but below [`USDC_MIN_DEPOSIT_ATOMIC`].
    BelowMin,
    /// Above [`USDC_MAX_DEPOSIT_ATOMIC`].
    AboveMax,
    /// A usable amount in atomic units.
    Valid(u64),
}

impl UsdcDepositCheck {
    /// Atomic units to submit, or `None` when the field is unusable.
    pub fn atomic(self) -> Option<u64> {
        match self {
            UsdcDepositCheck::Empty => Some(0),
            UsdcDepositCheck::Valid(atomic) => Some(atomic),
            UsdcDepositCheck::Invalid | UsdcDepositCheck::BelowMin | UsdcDepositCheck::AboveMax => {
                None
            }
        }
    }

    /// User-facing reason the field is unusable, if it is.
    pub fn error_message(self) -> Option<&'static str> {
        match self {
            UsdcDepositCheck::Empty | UsdcDepositCheck::Valid(_) => None,
            UsdcDepositCheck::Invalid => {
                Some("USDC amount must be a plain number with at most 6 decimals")
            }
            UsdcDepositCheck::BelowMin => Some("Minimum deposit is 0.01 USDC"),
            UsdcDepositCheck::AboveMax => Some("Maximum deposit is 1,000 USDC"),
        }
    }
}

/// Classify a USDC deposit field with exact integer parsing
/// ([`parse_usdc_atomic`]); no f64 anywhere, so `"1e3"` is `Invalid`.
pub fn check_usdc_deposit(input: &str) -> UsdcDepositCheck {
    let text = input.trim();
    if text.is_empty() {
        return UsdcDepositCheck::Empty;
    }
    match parse_usdc_atomic(text) {
        None => UsdcDepositCheck::Invalid,
        Some(0) => UsdcDepositCheck::Empty,
        Some(atomic) if atomic < USDC_MIN_DEPOSIT_ATOMIC => UsdcDepositCheck::BelowMin,
        Some(atomic) if atomic > USDC_MAX_DEPOSIT_ATOMIC => UsdcDepositCheck::AboveMax,
        Some(atomic) => UsdcDepositCheck::Valid(atomic),
    }
}
