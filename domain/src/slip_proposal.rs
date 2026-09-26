//! The deterministic half of the slip agent (`.plans/033` W1).
//!
//! The rule from `.plans/026` §0: a model may *propose* what a slip says, but
//! nothing a model outputs decides anything. This module is where the deciding
//! happens, and it contains no model and no I/O. It takes the facts read off a
//! slip (from the mini-QR, or from a vision model when there is no readable QR)
//! plus what the event expects, runs a fixed set of checks, and returns a
//! verdict.
//!
//! The verdict is a *proposal* to the organizer. In shadow mode nothing reads
//! it but the admin screen; the organizer's approve/reject is still the only
//! thing that moves a deposit.
//!
//! # Why an unknown is never a pass
//!
//! The QR carries a bank reference and nothing else: no amount, no time. A
//! QR-only slip therefore cannot be `Accepted` — its amount is unknown, and
//! "unknown" routes to a human. A checker that treated a missing fact as fine
//! would accept every slip that simply omitted the fact it could not fake.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Where the facts on a slip came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactSource {
    /// The slip's mini-QR, decoded in the browser and re-parsed by the server.
    Qr,
    /// A vision model reading the slip image.
    Vision,
}

impl FactSource {
    /// The stored form; matches the serde form and the D1 CHECK.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Qr => "qr",
            Self::Vision => "vision",
        }
    }
}

/// One of the fixed checks. The order of [`Check::ALL`] is the display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    /// The transferred amount equals the event's deposit.
    Amount,
    /// No other deposit has claimed this bank reference.
    RefNew,
    /// The transfer happened between registration and the upload.
    InWindow,
    /// The masked receiver account on the slip ends in the event's PromptPay
    /// digits.
    Receiver,
}

impl Check {
    /// Every check, in display order.
    pub const ALL: [Self; 4] = [Self::Amount, Self::RefNew, Self::InWindow, Self::Receiver];
}

/// The result of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The fact is known and agrees.
    Pass,
    /// The fact is known and disagrees.
    Fail,
    /// The fact is not on the slip, or could not be read.
    Unknown,
}

/// What the checker proposes to the organizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Every check passed.
    Accepted,
    /// Nothing failed, but at least one fact is unknown.
    NeedsReview,
    /// At least one check failed.
    Rejected,
}

impl Verdict {
    /// The stored form; matches the serde form and the D1 CHECK.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::NeedsReview => "needs_review",
            Self::Rejected => "rejected",
        }
    }
}

/// What was read off one slip. Every field is optional because no source
/// yields all of them: the QR gives only `bank_ref`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipFacts {
    /// `SlipReference::storage_key()` of the bank reference.
    pub bank_ref: Option<String>,
    /// Amount in satang (1 THB = 100 satang), so `500.50` is exact.
    pub amount_satang: Option<u64>,
    /// When the bank says the transfer happened.
    pub transferred_at: Option<DateTime<Utc>>,
    /// The receiver account as printed on the slip, usually masked
    /// (`xxx-xxx-5678`).
    pub receiver_account: Option<String>,
}

/// What the event expects of a valid slip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expectations {
    /// The event's deposit, in whole baht.
    pub deposit_amount_thb: u64,
    /// Earliest acceptable transfer time (the attendee's registration).
    pub window_start: Option<DateTime<Utc>>,
    /// Latest acceptable transfer time (the upload).
    pub window_end: DateTime<Utc>,
    /// Whether another deposit already claimed `SlipFacts::bank_ref`. Looked
    /// up by the caller; the checker does no I/O.
    pub ref_claimed_elsewhere: bool,
    /// The event's PromptPay id (phone or national id), any formatting.
    pub promptpay_id: String,
}

/// The checker's full answer: every check's outcome and the verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evaluation {
    /// One entry per [`Check::ALL`], in that order.
    pub checks: Vec<(Check, Outcome)>,
    /// Derived from `checks` by [`verdict_of`].
    pub verdict: Verdict,
}

impl Evaluation {
    /// The outcome of one check.
    pub fn outcome(&self, check: Check) -> Outcome {
        self.checks
            .iter()
            .find(|(c, _)| *c == check)
            .map_or(Outcome::Unknown, |(_, o)| *o)
    }
}

/// Run every check and derive the verdict.
pub fn evaluate(facts: &SlipFacts, expect: &Expectations) -> Evaluation {
    let checks: Vec<(Check, Outcome)> = Check::ALL
        .iter()
        .map(|check| (*check, run_check(*check, facts, expect)))
        .collect();
    let verdict = verdict_of(checks.iter().map(|(_, o)| *o));
    Evaluation { checks, verdict }
}

/// Any fail rejects; otherwise any unknown sends it to a human.
pub fn verdict_of(outcomes: impl IntoIterator<Item = Outcome>) -> Verdict {
    let mut verdict = Verdict::Accepted;
    for outcome in outcomes {
        match outcome {
            Outcome::Fail => return Verdict::Rejected,
            Outcome::Unknown => verdict = Verdict::NeedsReview,
            Outcome::Pass => {}
        }
    }
    verdict
}

fn run_check(check: Check, facts: &SlipFacts, expect: &Expectations) -> Outcome {
    match check {
        Check::Amount => match facts.amount_satang {
            None => Outcome::Unknown,
            Some(satang) => pass_if(expect.deposit_amount_thb.checked_mul(100) == Some(satang)),
        },
        Check::RefNew => match &facts.bank_ref {
            None => Outcome::Unknown,
            Some(_) => pass_if(!expect.ref_claimed_elsewhere),
        },
        Check::InWindow => match facts.transferred_at {
            None => Outcome::Unknown,
            Some(at) => pass_if(
                expect.window_start.is_none_or(|start| at >= start) && at <= expect.window_end,
            ),
        },
        Check::Receiver => match &facts.receiver_account {
            None => Outcome::Unknown,
            Some(printed) => receiver_outcome(printed, &expect.promptpay_id),
        },
    }
}

fn pass_if(ok: bool) -> Outcome {
    match ok {
        true => Outcome::Pass,
        false => Outcome::Fail,
    }
}

/// Fewest trailing digits a masked account must show before it is compared.
/// Two visible digits would match one PromptPay id in a hundred by chance.
const MIN_VISIBLE_TAIL: usize = 3;

/// Compare the unmasked tail of the printed receiver with the PromptPay id.
///
/// Slips mask the receiver (`xxx-xxx-5678`, `XXX-X-X1234-X`), so only the
/// trailing run of digits is compared. A tail too short to mean anything is
/// `Unknown`, not a pass.
fn receiver_outcome(printed: &str, promptpay_id: &str) -> Outcome {
    let tail = visible_digit_tail(printed);
    let expected: String = promptpay_id.chars().filter(char::is_ascii_digit).collect();
    match tail.len() >= MIN_VISIBLE_TAIL && !expected.is_empty() {
        false => Outcome::Unknown,
        true => pass_if(expected.ends_with(&tail)),
    }
}

/// The digits after the last mask character, ignoring separators.
///
/// `XXX-X-X1234-X` has a trailing mask after the digits; that layout cannot be
/// aligned with a PromptPay id without knowing the bank's format, so it yields
/// no tail rather than a guess.
fn visible_digit_tail(printed: &str) -> String {
    let significant: Vec<char> = printed
        .chars()
        .filter(|c| !matches!(c, '-' | ' ' | '.'))
        .collect();
    let tail_start = significant
        .iter()
        .rposition(|c| !c.is_ascii_digit())
        .map_or(0, |i| i + 1);
    significant[tail_start..].iter().collect()
}

/// Parse a baht amount as printed on a slip (`"500"`, `"500.00"`,
/// `"1,500.50"`, `"฿500.00"`) into satang, exactly.
///
/// Returns `None` for anything that is not a plain non-negative amount with at
/// most two decimals: a vision model's `"about 500"` must not become a number.
pub fn parse_thb_satang(input: &str) -> Option<u64> {
    let text: String = input
        .trim()
        .trim_start_matches('฿')
        .trim_end_matches("THB")
        .trim_end_matches("บาท")
        .trim()
        .chars()
        .filter(|c| *c != ',')
        .collect();
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    if whole.is_empty() || fraction.len() > 2 {
        return None;
    }
    if !whole
        .bytes()
        .chain(fraction.bytes())
        .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let baht: u64 = whole.parse().ok()?;
    let satang: u64 = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().ok()? * 10,
        _ => fraction.parse().ok()?,
    };
    baht.checked_mul(100)?.checked_add(satang)
}

/// One stored proposal, as the admin screen receives it
/// (`slip_proposals`, migration 0054).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipProposal {
    /// Where the facts came from.
    pub source: FactSource,
    /// The model id when `source` is `Vision`.
    #[serde(default)]
    pub model: Option<String>,
    /// What was read off the slip.
    pub facts: SlipFacts,
    /// Every check's outcome and the verdict.
    pub evaluation: Evaluation,
    /// RFC 3339.
    pub created_at: String,
}
