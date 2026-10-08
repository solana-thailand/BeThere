//! The deposit promises, written once.
//!
//! They follow the owner's decisions of 2026-09-28
//! (`docs/deposit-commitment-model.md` §5): D1, a no-show still gets the
//! deposit back; D3, THB refunds within 7 days. Since 2026-10-08 D1 is worded
//! as what the attendee gets, not as a "never forfeited" phrase (owner).
//! When a decision changes, change it here; the event page and the landing
//! FAQ read these. `tests/deposit_promise_has_one_home.rs` fails if a page
//! writes the promise itself, or if the retired phrase comes back.
//!
//! PromptPay (THB) only: the USDC escrow program can still forfeit after its
//! refund deadline, so no USDC surface may use `DEPOSIT_COMES_BACK`.
//!
//! Both languages live here, side by side, so a changed decision is changed
//! in EN and TH in one edit (.plans/037 §2). Pages call [`thb_refund_window`]
//! and [`deposit_comes_back`] with the reader's locale; the i18n catalog carries
//! only the words around them.

use crate::i18n::Locale;

/// D3: when a THB deposit comes back.
pub const THB_REFUND_WINDOW: &str = "within 7 days after the event";

/// D1: what a no-show gets (the deposit back).
pub const DEPOSIT_COMES_BACK: &str = "You still get your deposit back.";

/// D3 in Thai.
pub const THB_REFUND_WINDOW_TH: &str = "ภายใน 7 วันหลังจบงาน";

/// D1 in Thai.
pub const DEPOSIT_COMES_BACK_TH: &str = "คุณยังได้เงินมัดจำคืน";

/// D3 in the reader's language.
pub fn thb_refund_window(locale: Locale) -> &'static str {
    match locale {
        Locale::en => THB_REFUND_WINDOW,
        Locale::th => THB_REFUND_WINDOW_TH,
    }
}

/// D1 in the reader's language. PromptPay (THB) surfaces only.
pub fn deposit_comes_back(locale: Locale) -> &'static str {
    match locale {
        Locale::en => DEPOSIT_COMES_BACK,
        Locale::th => DEPOSIT_COMES_BACK_TH,
    }
}
