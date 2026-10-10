//! "How the deposit works" and the proof under it, on `/events` (.plans/045
//! R4.3, prototype events `#deposit` and `.proof`): three steps, the live
//! median time to get it back (`/api/public/stats`, `refund_after_end`), then
//! the payers who came (`domain::models::facts`, the hall's numbers).

use leptos::prelude::*;

use crate::i18n::{t_string, td_string, use_i18n};
use crate::locale::{fill, tr};
use crate::pages::landing::stats::{duration_label, use_landing_stats};
use event_checkin_domain::models::facts::ladder_total;

#[component]
pub fn DepositWalk() -> impl IntoView {
    let i18n = use_i18n();
    let stats = use_landing_stats();
    // No measured refunds yet: say what happens, without a number.
    let median = move || {
        stats
            .get()
            .and_then(|s| s.refund_after_end)
            .map(|t| {
                let d = duration_label(t.median_minutes, i18n.get_locale());
                format!(
                    " · {}",
                    fill(t_string!(i18n, landing.site.dep_median), &[("d", &d)])
                )
            })
            .unwrap_or_default()
    };
    let (paid, came) = ladder_total();
    view! {
        <section id="deposit" class="lp-walk">
            <div class="lp-wrap">
                <div class="lp-walk-head">
                    <h2 class="lp-h2">{tr(|l| td_string!(l, landing.site.dep_title))}</h2>
                    <span class="lp-st lp-st-yes">{tr(|l| td_string!(l, landing.site.dep_live))}</span>
                </div>
                <ol class="lp-walk-steps">
                    <li>
                        <svg viewBox="0 0 64 64" aria-hidden="true">
                            <circle cx="32" cy="32" r="27" fill="none" stroke="currentColor" stroke-width="4" />
                            <text x="32" y="44" text-anchor="middle" font-weight="800" font-size="32" fill="currentColor">"฿"</text>
                        </svg>
                        <b>{tr(|l| td_string!(l, landing.site.dep_1))}</b>
                    </li>
                    <li>
                        <svg viewBox="0 0 64 64" aria-hidden="true">
                            <rect x="8" y="8" width="48" height="48" rx="6" fill="currentColor" />
                            <g class="lp-walk-qr">
                                <rect x="15" y="15" width="12" height="12" />
                                <rect x="37" y="15" width="12" height="12" />
                                <rect x="15" y="37" width="12" height="12" />
                                <rect x="37" y="37" width="5" height="5" />
                                <rect x="44" y="44" width="5" height="5" />
                            </g>
                        </svg>
                        <b>{tr(|l| td_string!(l, landing.site.dep_2))}</b>
                    </li>
                    <li class="lp-walk-ok">
                        <svg viewBox="0 0 64 64" aria-hidden="true">
                            <circle cx="32" cy="32" r="27" fill="currentColor" />
                            <path class="lp-walk-tick" d="M20 33l8 8 16-17" />
                        </svg>
                        <b>{tr(|l| td_string!(l, landing.site.dep_3))}</b>
                    </li>
                </ol>
                <p class="lp-plan-fine">{tr(|l| td_string!(l, landing.site.dep_fine))}{median}</p>
                <p class="lp-proof">
                    <b>{format!("{came}/{paid}")}</b>
                    <span>{tr(|l| td_string!(l, landing.site.proof_line))}</span>
                    <a href="/organizers#story">{tr(|l| td_string!(l, landing.site.proof_why))}</a>
                </p>
                <p class="lp-plan-fine">{tr(|l| td_string!(l, landing.site.proof_fine))}</p>
            </div>
        </section>
    }
}
