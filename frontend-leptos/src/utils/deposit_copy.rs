//! The deposit promises, written once.
//!
//! They follow the owner's decisions of 2026-09-28
//! (`docs/deposit-commitment-model.md` §5): D1, nothing is forfeited; D3, THB
//! refunds within 7 days. When a decision changes, change it here; the event
//! page and the landing FAQ read these. `tests/deposit_promise_has_one_home.rs`
//! fails if a page writes the promise itself.
//!
//! PromptPay (THB) only: the USDC escrow program can still forfeit after its
//! refund deadline, so no USDC surface may use `NEVER_FORFEITED`.
//!
//! Both languages live here, side by side, so a changed decision is changed
//! in EN and TH in one edit (.plans/037 §2). Pages call [`thb_refund_window`]
//! and [`never_forfeited`] with the reader's locale; the i18n catalog carries
//! only the words around them.

use crate::i18n::Locale;

/// D3: when a THB deposit comes back.
pub const THB_REFUND_WINDOW: &str = "within 7 days after the event";

/// D1: what a no-show loses (nothing).
pub const NEVER_FORFEITED: &str = "Your deposit is never forfeited.";

/// D3 in Thai.
pub const THB_REFUND_WINDOW_TH: &str = "ภายใน 7 วันหลังจบงาน";

/// D1 in Thai.
pub const NEVER_FORFEITED_TH: &str = "เงินมัดจำของคุณจะไม่ถูกริบ";

/// D3 in the reader's language.
pub fn thb_refund_window(locale: Locale) -> &'static str {
    match locale {
        Locale::en => THB_REFUND_WINDOW,
        Locale::th => THB_REFUND_WINDOW_TH,
    }
}

/// D1 in the reader's language. PromptPay (THB) surfaces only.
pub fn never_forfeited(locale: Locale) -> &'static str {
    match locale {
        Locale::en => NEVER_FORFEITED,
        Locale::th => NEVER_FORFEITED_TH,
    }
}
