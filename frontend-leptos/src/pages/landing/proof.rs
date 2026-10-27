//! The operational proof under the hero (.plans/049, owner pick B): how long a
//! deposit slip takes to check and how long after an event the refund comes,
//! both medians the system measured (`GET /api/public/stats`). They are drawn
//! only when both are published (the worker withholds a timing under
//! `MIN_TIMING_SAMPLES`), with when they were measured and how many cases they
//! cover: a median over past events, never a promise.

use leptos::prelude::*;

use crate::i18n::{td_string, use_i18n};

use super::stats::{duration_label, measured_at_label, use_landing_stats};

#[component]
pub fn Proof() -> impl IntoView {
    let i18n = use_i18n();
    let stats = use_landing_stats();
    move || {
        let s = stats.get()?;
        let (slip, refund) = (s.slip_check?, s.refund_after_end?);
        let locale = i18n.get_locale();
        let fine = crate::locale::fill(
            td_string!(locale, landing.proof.fine),
            &[
                ("at", &measured_at_label(&s.measured_at)),
                ("slips", &slip.samples.to_string()),
                ("refunds", &refund.samples.to_string()),
            ],
        );
        Some(view! {
            <section class="lp-proof" aria-label=td_string!(locale, landing.proof.slip)>
                <div class="lp-wrap">
                    <div class="lp-proof-grid">
                        <div class="lp-proof-card">
                            <b>{duration_label(slip.median_minutes, locale)}</b>
                            <p>{td_string!(locale, landing.proof.slip)}</p>
                        </div>
                        <div class="lp-proof-card">
                            <b>{duration_label(refund.median_minutes, locale)}</b>
                            <p>{td_string!(locale, landing.proof.refund)}</p>
                        </div>
                    </div>
                    <p class="lp-fineprint">{fine}</p>
                </div>
            </section>
        })
    }
}
