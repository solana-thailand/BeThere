//! Read-only cards for settled deposits: the Refunded and Held-as-Credit tabs.
//! Both show the same attendee/amount/slip/bank block and differ only in the
//! timestamp, the missing-bank wording, the refund proof link and the badge.

use leptos::prelude::*;

use crate::api::ThbDepositInfo;
use crate::pages::admin_deposit_bank_info::refund_bank_info;
use crate::pages::admin_deposit_slip_link::slip_link;
use crate::utils;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettledKind {
    Refunded,
    Held,
}

impl SettledKind {
    fn timestamp_label(self) -> &'static str {
        match self {
            SettledKind::Refunded => "Refunded at",
            SettledKind::Held => "Held at",
        }
    }

    fn timestamp(self, item: &ThbDepositInfo) -> Option<&str> {
        match self {
            SettledKind::Refunded => item.refunded_at.as_deref(),
            SettledKind::Held => item.held_as_credit_at.as_deref(),
        }
    }

    fn missing_bank_info(self) -> &'static str {
        match self {
            SettledKind::Refunded => "⚠ No bank info was provided",
            // Held credit can still be paid out later (a contact's "Request
            // Return"), so the payout details belong here too.
            SettledKind::Held => "⚠ No bank info — ask attendee before any payout",
        }
    }

    fn badge(self) -> &'static str {
        match self {
            SettledKind::Refunded => "✓ Refunded",
            SettledKind::Held => "✓ Held as Credit",
        }
    }
}

/// The refund proof link, Refunded tab only. Rows stored before `.issues/145`
/// may hold any scheme, so only a safe document link is rendered.
fn refund_proof_link(kind: SettledKind, item: &ThbDepositInfo) -> Option<AnyView> {
    match kind {
        SettledKind::Held => None,
        SettledKind::Refunded => {
            let refund_proof_url = item
                .refund_proof_url
                .as_deref()
                .and_then(event_checkin_domain::validation::safe_document_link)
                .map(str::to_string);
            let has_refund_proof = refund_proof_url.is_some();
            Some(
                view! {
                    <Show when=move || has_refund_proof fallback=|| view! { <span></span> }>
                        <div class="admin-dep-slip-link-row">
                            <a
                                href=refund_proof_url.clone().unwrap_or_default()
                                target="_blank"
                                rel="noopener noreferrer"
                                class="link-accent"
                            >
                                "View Refund Proof"
                            </a>
                        </div>
                    </Show>
                }
                .into_any(),
            )
        }
    }
}

fn settled_card(kind: SettledKind, item: &ThbDepositInfo) -> AnyView {
    let amount = format!("{} THB", item.amount_thb);
    let verified_by = item.verified_by.as_deref().unwrap_or("Unknown").to_string();
    let settled_at = kind
        .timestamp(item)
        .map(utils::format_timestamp)
        .unwrap_or_else(|| "N/A".to_string());
    let display_name = item
        .attendee_name
        .as_deref()
        .unwrap_or(&item.attendee_id)
        .to_string();
    let bank_info = refund_bank_info(
        item.bank_account.clone(),
        item.bank_name.clone(),
        item.account_name.clone(),
        kind.missing_bank_info(),
    );
    let label = kind.timestamp_label();

    view! {
        <div class="card">
            <div class="flex-row-wrap">
                <div>
                    <div class="admin-attendee-name">
                        {format!("Attendee: {display_name}")}
                    </div>
                    <div class="admin-amount-line">
                        {amount}
                    </div>
                    <div class="panel-hint">
                        {format!("Verified by: {}", utils::escape_html(&verified_by))}
                    </div>
                    <div class="panel-hint">
                        {format!("{label}: {settled_at}")}
                    </div>

                    {slip_link(item.slip_url.clone())}

                    {bank_info}

                    {refund_proof_link(kind, item)}
                </div>
                <div>
                    <span class="badge badge-success">{kind.badge()}</span>
                </div>
            </div>
        </div>
    }
    .into_any()
}

/// One card per settled deposit in `items`.
#[component]
pub fn SettledDepositList(
    items: ReadSignal<Vec<ThbDepositInfo>>,
    kind: SettledKind,
) -> impl IntoView {
    move || {
        items
            .get()
            .iter()
            .map(|item| settled_card(kind, item))
            .collect_view()
    }
}
