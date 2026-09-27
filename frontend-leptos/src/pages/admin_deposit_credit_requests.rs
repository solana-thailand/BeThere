//! Held-as-Credit tab: the "Credit Refund Requested" sub-list (Issue #061
//! Phase 3 — exit path). Cross-event: contacts who clicked "Request Return" on
//! their ticket page. The organizer pays out through the existing refund
//! tooling, then clicks "✓ Clear" to dismiss the request.

use leptos::prelude::*;

use crate::api::{self, ClearCreditRefundRequest, CreditRefundRequest};
use crate::components::{self, ToastMessage, ToastType};
use crate::icons::{Icon, IconName};
use crate::utils;

#[component]
pub fn CreditRefundRequests(
    requests: ReadSignal<Vec<CreditRefundRequest>>,
    set_toast: WriteSignal<Option<ToastMessage>>,
    set_refresh_counter: WriteSignal<u32>,
) -> impl IntoView {
    // Which email is being cleared — disables that row's button while the POST
    // is in flight. The clear is idempotent, so a re-click is a safe retry.
    let (clear_pending_email, set_clear_pending_email) = signal(None::<String>);

    let handle_clear = move |email: String| {
        set_clear_pending_email.set(Some(email.clone()));

        leptos::task::spawn_local(async move {
            let body = ClearCreditRefundRequest {
                email: email.clone(),
            };
            match api::clear_credit_refund_request(&body).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        "Cleared credit refund request.",
                        ToastType::Success,
                    );
                    set_refresh_counter.update(|c| *c += 1);
                }
                Err(e) => {
                    log::warn!("[admin-deposit] failed to clear credit refund request: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to clear request: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_clear_pending_email.set(None);
        });
    };

    view! {
        <div class="admin-dep-credit-refund-requests">
            <div class="admin-dep-flow-hint">
                <Icon icon=IconName::Warning class="icon-sm"/>
                {move || {
                    let count = requests.get().len();
                    format!(
                        "{} contact{} requested return of held credit. Process the payout through your refund channel, then clear the request.",
                        count,
                        if count != 1 { "s" } else { "" }
                    )
                }}
            </div>
            {move || {
                let pending = clear_pending_email.get();
                requests.get().into_iter().map(|req| {
                    let CreditRefundRequest { email, name, credit_thb, credit_usdc, locked_thb, locked_until, requested_at, .. } = req;
                    let is_pending = pending.as_deref() == Some(email.as_str());
                    // Issue #120 §3. Nothing payable and credit still covering an
                    // event that has not ended: clearing would reverse nothing and
                    // drop the request. The server refuses this with a 409 — this
                    // only saves the organizer the round trip, so a locked bucket
                    // the display does not cover (USDC) is still caught there.
                    let locked_only = credit_thb == 0 && credit_usdc == 0 && locked_thb > 0;
                    let display_name = if name.is_empty() { email.clone() } else { name };
                    let credit_str = if credit_thb > 0 && credit_usdc > 0 {
                        format!("{credit_thb} THB + {credit_usdc} USDC")
                    } else if credit_thb > 0 {
                        format!("{credit_thb} THB")
                    } else if credit_usdc > 0 {
                        format!("{credit_usdc} USDC")
                    } else {
                        "0".to_string()
                    };
                    let requested_display = if requested_at.is_empty() {
                        "N/A".to_string()
                    } else {
                        utils::format_timestamp(&requested_at)
                    };
                    view! {
                        <div class="card admin-dep-credit-refund-row">
                            <div class="flex-row-wrap">
                                <div>
                                    <div class="admin-attendee-name">
                                        {utils::escape_html(&display_name)}
                                    </div>
                                    <div class="admin-amount-line">
                                        {format!("Held credit: {credit_str}")}
                                    </div>
                                    {(locked_thb > 0).then(|| {
                                        let event = match locked_until.is_empty() {
                                            true => "an event that has not ended".to_string(),
                                            false => locked_until.clone(),
                                        };
                                        view! {
                                            <div class="panel-hint">
                                                {format!(
                                                    "{locked_thb} THB is covering {event} — it returns when that event ends"
                                                )}
                                            </div>
                                        }
                                    })}
                                    <div class="panel-hint">
                                        {format!("Requested: {requested_display}")}
                                    </div>
                                </div>
                                <div>
                                    <span class="badge badge-warning">"Refund Requested"</span>
                                    <button
                                        class="btn btn-success btn-xs admin-dep-clear-btn"
                                        disabled=move || is_pending || locked_only
                                        title=match locked_only {
                                            true => "Nothing to pay out yet — the credit is covering an event that has not ended. The request stays open until then.",
                                            false => "",
                                        }
                                        on:click=move |_| handle_clear(email.clone())
                                    >
                                        {move || if is_pending {
                                            view! { <span>"Clearing..."</span> }.into_any()
                                        } else if locked_only {
                                            view! { <span>"Waiting on event"</span> }.into_any()
                                        } else {
                                            view! {
                                                <Icon icon=IconName::Check class="icon-sm"/>
                                                " ✓ Clear"
                                            }.into_any()
                                        }}
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                }).collect_view()
            }}
        </div>
    }
}
