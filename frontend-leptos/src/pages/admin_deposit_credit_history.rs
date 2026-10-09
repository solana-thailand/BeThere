//! Held-as-Credit tab: "Paid out", the held-credit payouts already recorded
//! (from the global audit log, newest first) — who was paid, how much, by
//! whom, whether they asked, and the transfer slip. Organizers only; the
//! worker scopes it like the payout queue. Renders nothing when empty.

use leptos::prelude::*;

use crate::api::{self, CreditPayoutRecord};
use crate::icons::{Icon, IconName};
use crate::utils;

/// D1 writes `YYYY-MM-DD HH:MM:SS` in UTC; `Date.parse` would read that as
/// local time, so make it ISO UTC first.
fn utc_iso(timestamp: &str) -> String {
    match timestamp.contains('T') {
        true => timestamp.to_string(),
        false => format!("{}Z", timestamp.replacen(' ', "T", 1)),
    }
}

fn amount_line(record: &CreditPayoutRecord) -> String {
    match (record.thb, record.usdc) {
        (thb, 0) => format!("{thb} THB"),
        (0, usdc) => format!("{usdc} USDC"),
        (thb, usdc) => format!("{thb} THB + {usdc} USDC"),
    }
}

#[component]
pub fn CreditPayoutHistory(refresh_counter: ReadSignal<u32>) -> impl IntoView {
    let (payouts, set_payouts) = signal(Vec::<CreditPayoutRecord>::new());
    Effect::new(move |_| {
        let _ = refresh_counter.get();
        leptos::task::spawn_local(async move {
            match api::get_credit_payout_history().await {
                Ok(data) => set_payouts.set(data.payouts),
                // Non-fatal, like the queue: a scanner gets 403 and sees nothing.
                Err(e) => log::warn!("[admin-deposit] failed to load payout history: {e}"),
            }
        });
    });
    view! {
        <Show when=move || !payouts.get().is_empty() fallback=|| ()>
            <div class="admin-dep-credit-refund-requests">
                <div class="admin-dep-flow-hint">
                    <Icon icon=IconName::Check class="icon-sm"/>
                    {move || {
                        let count = payouts.get().len();
                        format!("Paid out: {count} recorded payout{}", if count != 1 { "s" } else { "" })
                    }}
                </div>
                {move || {
                    payouts
                        .get()
                        .into_iter()
                        .map(|record| {
                            let when = utils::format_timestamp(&utc_iso(&record.paid_at));
                            let amount = amount_line(&record);
                            let asked = match record.initiated_by.as_str() {
                                "organizer" => "Not requested",
                                _ => "Requested",
                            };
                            view! {
                                <div class="admin-dep-payout-history-row">
                                    <div class="admin-dep-payout-history-main">
                                        <strong>{record.contact.clone()}</strong>
                                        <span class="admin-amount-line">{amount}</span>
                                    </div>
                                    <div class="panel-hint">
                                        {format!("{when} · by {} · {asked}", record.paid_by)}
                                    </div>
                                    {record.proof_url.clone().map(|url| view! {
                                        <a href=url target="_blank" rel="noopener" class="admin-dep-payout-history-slip">
                                            "View slip ↗"
                                        </a>
                                    })}
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </Show>
    }
}
