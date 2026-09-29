//! Where a registered attendee goes next: the rule behind `next_step` in
//! `/api/my-registration(s)` and the register response (`.issues/174`).

/// The attendee's next page, before it is turned into a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextStepKind {
    /// The ticket (final state, or ready for the door).
    Ticket,
    /// The claim page: checked in, badge not claimed yet.
    Claim,
    /// Pay the deposit first.
    Deposit,
    /// Online track: the ticket page waits for the quest.
    Waiting,
}

/// What decides the next step. `deposit_required` is the event's own
/// `deposit_enabled`: a free event never sends anyone to pay.
#[derive(Debug, Clone, Copy)]
pub struct NextStepFacts {
    pub is_claimed: bool,
    pub is_checked_in: bool,
    pub has_claim_token: bool,
    pub is_online_participant: bool,
    pub format_has_in_person: bool,
    pub deposit_required: bool,
    pub real_deposit: bool,
}

pub fn next_step_kind(facts: NextStepFacts) -> NextStepKind {
    match facts {
        NextStepFacts {
            is_claimed: true, ..
        } => NextStepKind::Ticket,
        NextStepFacts {
            is_checked_in: true,
            has_claim_token: true,
            ..
        } => NextStepKind::Claim,
        NextStepFacts {
            is_online_participant: true,
            ..
        } => NextStepKind::Waiting,
        NextStepFacts {
            format_has_in_person: false,
            ..
        } => NextStepKind::Waiting,
        NextStepFacts {
            deposit_required: true,
            real_deposit: false,
            ..
        } => NextStepKind::Deposit,
        _ => NextStepKind::Ticket,
    }
}
