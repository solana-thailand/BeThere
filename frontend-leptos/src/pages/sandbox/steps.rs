//! The sandbox steps in order, and what each one shows.

use crate::i18n::{Locale, td_string};

/// One step of the loop. The order is the program's order: the event must
/// exist before a deposit, the check-in must land before the event ends, and
/// the refund opens only after it ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Event,
    Faucet,
    Deposit,
    CheckIn,
    Refund,
    Return,
    Done,
}

pub const STEPS: [Step; 6] = [
    Step::Event,
    Step::Faucet,
    Step::Deposit,
    Step::CheckIn,
    Step::Refund,
    Step::Return,
];

type Text = fn(Locale) -> &'static str;

impl Step {
    pub fn next(self) -> Self {
        match self {
            Self::Event => Self::Faucet,
            Self::Faucet => Self::Deposit,
            Self::Deposit => Self::CheckIn,
            Self::CheckIn => Self::Refund,
            Self::Refund => Self::Return,
            Self::Return | Self::Done => Self::Done,
        }
    }

    /// Index into the per-step signature list.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> Text {
        match self {
            Self::Event => |l| td_string!(l, sandbox.step_event),
            Self::Faucet => |l| td_string!(l, sandbox.step_faucet),
            Self::Deposit => |l| td_string!(l, sandbox.step_deposit),
            Self::CheckIn => |l| td_string!(l, sandbox.step_checkin),
            Self::Refund => |l| td_string!(l, sandbox.step_refund),
            Self::Return | Self::Done => |l| td_string!(l, sandbox.step_return),
        }
    }

    pub fn hint(self) -> Text {
        match self {
            Self::Event => |l| td_string!(l, sandbox.step_event_hint),
            Self::Faucet => |l| td_string!(l, sandbox.step_faucet_hint),
            Self::Deposit => |l| td_string!(l, sandbox.step_deposit_hint),
            Self::CheckIn => |l| td_string!(l, sandbox.step_checkin_hint),
            Self::Refund => |l| td_string!(l, sandbox.step_refund_hint),
            Self::Return | Self::Done => |l| td_string!(l, sandbox.step_return_hint),
        }
    }

    pub fn button(self) -> Text {
        match self {
            Self::Event => |l| td_string!(l, sandbox.btn_event),
            Self::Faucet => |l| td_string!(l, sandbox.btn_faucet),
            Self::Deposit => |l| td_string!(l, sandbox.btn_deposit),
            Self::CheckIn => |l| td_string!(l, sandbox.btn_checkin),
            Self::Refund => |l| td_string!(l, sandbox.btn_refund),
            Self::Return | Self::Done => |l| td_string!(l, sandbox.btn_return),
        }
    }
}

/// `m:ss` for a countdown of `seconds` (never negative).
pub fn countdown(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{}:{:02}", s / 60, s % 60)
}
