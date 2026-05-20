pub mod claim_forfeited;
pub mod close_deposit;
pub mod close_event;
pub mod create_event;
pub mod deactivate_event;
pub mod deposit;
pub mod mark_checked_in;
pub mod refund;

pub use claim_forfeited::*;
pub use close_deposit::*;
pub use close_event::*;
pub use create_event::*;
pub use deactivate_event::*;
pub use deposit::*;
pub use mark_checked_in::*;
pub use refund::*;

use quasar_lang::prelude::*;

/// Defense-in-depth: verify no two accounts share the same address.
///
/// Follows Solana Foundation Security Checklist #7.
/// PDA seeds prevent collisions, but explicit checks provide defense-in-depth
/// against duplicate mutable account attacks.
#[inline(always)]
pub(crate) fn require_distinct<T: PartialEq>(items: &[T]) -> Result<(), ProgramError> {
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            if items[i] == items[j] {
                return Err(ProgramError::InvalidArgument);
            }
        }
    }
    Ok(())
}
