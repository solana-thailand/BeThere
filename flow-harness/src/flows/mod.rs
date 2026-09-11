//! Flow modules — one per harness scenario in plan 005 §3.4.
//!
//! Each submodule implements [`crate::runner::Flow`] for a single end-to-end
//! scenario. The runner drives them in [`default_flows`] order, which mirrors
//! the dependency order in the plan:
//!
//!  1. `deposit`                       — register → POST deposit → poll verified
//!  2. `refund_pre_event_end`          — negative: revert `RefundNotYetAllowed`
//!  3. `refund_post_event_end_checked_in` — positive: refund succeeds post-end
//!  4. `refund_no_show_deadline`        — both halves of the no-show window
//!  5. `claim`                          — NFT claim reachability + state
//!  6. `auth`                           — plan 006 SIWS regression baseline
//!
//! ## Staging-independence
//!
//! Every flow struct's `new()` and config is offline-testable. Only the `run`
//! body issues HTTP calls, and those are gated behind `// TODO(staging-live):`
//! markers. Until staging is live, the runner can still register the default
//! flow set (proving wiring); invoking `run_all` against an unreachable worker
//! produces `Transport` errors that the runner records cleanly.

pub mod auth;
pub mod claim;
pub mod deposit;
pub mod refund_no_show_deadline;
pub mod refund_post_event_end_checked_in;
pub mod refund_pre_event_end;

pub use auth::AuthFlow;
pub use claim::ClaimFlow;
pub use deposit::DepositFlow;
pub use refund_no_show_deadline::RefundNoShowDeadlineFlow;
pub use refund_post_event_end_checked_in::RefundPostEventEndCheckedInFlow;
pub use refund_pre_event_end::RefundPreEventEndFlow;

use crate::runner::Runner;

/// Register the canonical plan 005 §3.4 flow set, in dependency order.
///
/// The runner executes flows sequentially in registration order, so the order
/// here is the order in `summary.json`. A failure in an earlier flow does not
/// skip later ones (see [`Runner::run_all`]); the summary is always complete.
///
/// Returns `&mut Runner` so the call site can chain or append extra flows:
///
/// ```ignore
/// let mut runner = Runner::new(ctx, client, results_root);
/// flows::register_default(&mut runner);
/// ```
pub fn register_default(runner: &mut Runner) {
    runner.register(DepositFlow::from_env());
    runner.register(RefundPreEventEndFlow::new());
    runner.register(RefundPostEventEndCheckedInFlow::new());
    runner.register(RefundNoShowDeadlineFlow::new());
    runner.register(ClaimFlow::new());
    // AuthFlow::from_env picks up the optional session cookie from
    // FLOW_HARNESS_ATTENDEE_SESSION at registration time. CLI callers should
    // call ONLY register_default — do not register AuthFlow a second time
    // (doing so yields a duplicate auth row in summary.json).
    runner.register(AuthFlow::from_env());
}

/// Register one named flow for focused staging diagnosis. This is deliberately
/// separate from [`register_default`]: focused runs never qualify a production
/// preflight gate and therefore must not replace the full-suite green signal.
pub fn register_named(runner: &mut Runner, name: &str) -> Result<(), String> {
    match name {
        "deposit" => runner.register(DepositFlow::from_env()),
        "refund-pre-event-end" => runner.register(RefundPreEventEndFlow::new()),
        "refund-post-event-end-checked-in" => {
            runner.register(RefundPostEventEndCheckedInFlow::new())
        }
        "refund-no-show-deadline" => runner.register(RefundNoShowDeadlineFlow::new()),
        "claim" => runner.register(ClaimFlow::new()),
        "auth" => runner.register(AuthFlow::from_env()),
        other => {
            return Err(format!(
                "unknown flow '{other}'; expected deposit, refund-pre-event-end, \
                 refund-post-event-end-checked-in, refund-no-show-deadline, claim, or auth"
            ));
        }
    };
    Ok(())
}
