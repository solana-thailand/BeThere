//! Header chips above the admin deposit tabs: cross-event credit liability,
//! this event's Cash/Credit/Comp split, and who got in on credit. Each renders
//! only when it has something to say.

use leptos::prelude::*;

use crate::api::{CreditLiability, DepositSourceSummary, ThbDepositInfo};
use crate::icons::{Icon, IconName};

#[component]
pub fn DepositSummaryChips(
    liability: ReadSignal<CreditLiability>,
    source_summary: ReadSignal<DepositSourceSummary>,
    credit_used_list: ReadSignal<Vec<ThbDepositInfo>>,
) -> impl IntoView {
    view! {
        // Credit liability header chip — organizer's total cash held as
        // rolling deposit credit across all contacts (Issue #061 Phase 2
        // option a2). Cross-event (global); only renders when there's
        // actual liability to surface (no clutter when balance is zero).
        <Show when=move || { let l = liability.get(); l.total_thb > 0 || l.total_usdc > 0 } fallback=|| view! { <div></div> }>
            <div
                class="admin-dep-liability-chip"
                title="Your total cash liability from rolling deposit credit — attendees who chose credit over refund. Auto-applies to their next event registration."
            >
                <Icon icon=IconName::MoneyWings class="icon-sm"/>
                <span>
                    {move || {
                        let l = liability.get();
                        let mut parts: Vec<String> = Vec::new();
                        if l.total_thb > 0 {
                            parts.push(format!("{} THB", l.total_thb));
                        }
                        if l.total_usdc > 0 {
                            parts.push(format!("{} USDC", l.total_usdc));
                        }
                        format!(
                            "Total credit held: {} across {} contacts",
                            parts.join(" + "),
                            l.contact_count
                        )
                    }}
                </span>
            </div>
        </Show>
        // Per-event Cash/Credit/Comp summary chip (GOAT reconciliation): how
        // attendees got in for THIS event — paid cash, spent rolling credit,
        // or staff comp. Complements the per-attendee "Credit ✓" roster badge.
        <Show when=move || { let s = source_summary.get(); s.cash_count + s.credit_count + s.comp_count > 0 } fallback=|| view! { <div></div> }>
            <div
                class="admin-dep-liability-chip"
                title="How attendees got in for this event: paid cash vs spent rolling credit vs free (staff/comp)."
            >
                <Icon icon=IconName::MoneyWings class="icon-sm"/>
                <span>
                    {move || {
                        let s = source_summary.get();
                        format!(
                            "This event \u{2014} Cash: {} (\u{0e3f}{}) \u{00b7} Credit: {} (\u{0e3f}{}) \u{00b7} Free/staff: {}",
                            s.cash_count, s.cash_thb, s.credit_count, s.credit_thb, s.comp_count
                        )
                    }}
                </span>
            </div>
        </Show>
        // Who got in via credit (names) — the GOAT credit-used list on the money page.
        <Show when=move || !credit_used_list.get().is_empty() fallback=|| view! { <div></div> }>
            <div class="admin-dep-credit-used" style="margin:4px 0 8px; font-size:0.85em; opacity:0.85;">
                <strong>"Used credit: "</strong>
                {move || {
                    let names: Vec<String> = credit_used_list.get().iter().map(|d| {
                        let name = d.attendee_name.clone().unwrap_or_else(|| d.attendee_id.clone());
                        format!("{name} (\u{0e3f}{})", d.amount_thb)
                    }).collect();
                    names.join(" \u{00b7} ")
                }}
            </div>
        </Show>
    }
}
