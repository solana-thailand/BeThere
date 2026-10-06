//! What an upcoming-event card says about money, and which cards come first
//! (.plans/043 L3). Pure, so the rules are tested without a browser.
//!
//! Build plan rules 1 and 3: the amount is the event's own configured THB
//! deposit, never a literal; and the card says the money comes back when you
//! show up. It never offers the "can't come → credit" exit (ASKS-4 §19).

/// The one-line deposit rule under a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepositRule {
    /// "฿{n}, all back when you show up".
    BackWhenYouShowUp(u64),
    /// An online-only event: free, no deposit.
    OnlineFree,
    /// In person, no deposit configured.
    Free,
}

impl DepositRule {
    /// `format` is the API's `event_format` string. A deposit switched on
    /// with no THB amount configured says nothing about money rather than
    /// guess one (USDC-only events state theirs on the event page).
    pub fn for_event(deposit_enabled: bool, amount_thb: u64, format: &str) -> Option<DepositRule> {
        match (format, deposit_enabled, amount_thb) {
            ("online", _, _) => Some(DepositRule::OnlineFree),
            (_, true, 0) => None,
            (_, true, n) => Some(DepositRule::BackWhenYouShowUp(n)),
            (_, false, _) => Some(DepositRule::Free),
        }
    }
}

/// Nearest start first; a date still to be announced (`0`) goes last.
pub fn nearest_first(start_ms: &mut [(i64, usize)]) {
    start_ms.sort_by_key(|&(ms, i)| (ms <= 0, ms, i));
}
