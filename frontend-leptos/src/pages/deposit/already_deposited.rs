//! AlreadyDeposited + NotEnabled + Error + Loading views.

use leptos::prelude::*;

use crate::icons::{Icon, IconName};

use crate::api::{self, DepositMethod, DepositStatusResponse};
use crate::i18n::{t, t_string, use_i18n};
use crate::utils::format_timestamp;

use super::types::*;

/// Outcome of the "Check confirmation again" button, kept typed so the note
/// renders in the reader's current language.
#[derive(Clone, Copy)]
enum ConfirmCheck {
    Syncing,
    Pending,
    Unavailable,
}

/// Loading view.
pub fn loading_view() -> AnyView {
    view! {
        <div class="dep2-card">
            <div class="dep2-confirming">
                <div class="dep2-confirming-dots">
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                </div>
                <p>{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.loading))}</p>
            </div>
        </div>
    }
    .into_any()
}

/// Error view.
pub fn error_view(error: DepositError) -> AnyView {
    let i18n = use_i18n();
    let message = match error {
        DepositError::InvalidLink => view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.error.invalid_link))} }.into_any(),
        DepositError::EndedNoDeposit => {
            view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.error.ended_no_deposit))} }.into_any()
        }
        DepositError::LoadFailed(error) => {
            view! { {t!(i18n, deposit.error.load_failed, error)} }.into_any()
        }
        DepositError::ReloadFailed => view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.error.reload_failed))} }.into_any(),
    };
    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.error.title))}</span>
            </div>
            <p>{message}</p>
            <a href="/" class="btn btn-primary">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.error.go_home))}</a>
        </div>
    }
    .into_any()
}

/// Not enabled view.
///
/// Surfaces the **resolved** event name + slug so the organizer can immediately
/// tell whether the wrong event was looked up. The most common cause of this
/// view in production is the "no event_id in URL → first active event fallback"
/// path: the URL falls through to whatever happens to be the newest active
/// event, which may have deposits disabled even though the attendee's actual
/// event has them enabled. Showing the resolved event name makes that mismatch
/// self-diagnosable instead of looking like a backend bug.
pub fn not_enabled_view(data: &DepositStatusResponse) -> AnyView {
    let i18n = use_i18n();
    let event_name = data.event_name.clone();
    let event_slug = data.event_slug.clone();
    let has_event_info = !event_name.is_empty();
    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.not_enabled.title))}</span>
            </div>
            <p>{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.not_enabled.body))}</p>

            // Diagnostic block — surface which event the backend actually
            // resolved. Without this, "wrong event fallback" looks identical
            // to "deposits genuinely disabled", forcing a curl to debug.
            {if has_event_info {
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            {t!(i18n, deposit.not_enabled.resolved, name = event_name)}
                        </p>
                        {if !event_slug.is_empty() {
                            view! {
                                <p class="hint-note">
                                    {t!(i18n, deposit.not_enabled.slug, slug = event_slug)}
                                </p>
                            }.into_any()
                        } else {
                            ().into_any()
                        }}
                        <p class="hint-note">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.not_enabled.hint))}
                        </p>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}

            <a href="/" class="btn btn-primary">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.error.go_home))}</a>
        </div>
    }
    .into_any()
}

/// Already deposited view.
pub fn already_deposited_view(
    data: &DepositStatusResponse,
    set_state: &WriteSignal<DepositPageState>,
) -> AnyView {
    let i18n = use_i18n();
    let info = data.status.as_ref().unwrap();
    // Rolling credit is reliably signalled by the method enum (auto-applied credit
    // is stored as credit_thb/credit_usdc). The string checks are a legacy
    // fallback; on their own they missed auto-applied credit (the markers live in
    // slip_url/verified_by, which aren't exposed here).
    let is_credit = matches!(
        info.method,
        DepositMethod::CreditThb | DepositMethod::CreditUsdc
    ) || info
        .tx_signature
        .as_ref()
        .is_some_and(|s| s.contains("CREDIT"))
        || info
            .wallet_address
            .as_ref()
            .is_some_and(|w| w.contains("CREDIT"));
    // The label is display-only; the method stays the typed enum.
    let method = info.method;
    let display_method_label = move || match is_credit {
        true => t_string!(i18n, deposit.method.rolling_credit),
        false => deposit_method_display(i18n.get_locale(), &method).1,
    };
    let verified = info.verified;
    let verified_text = move || match verified {
        true => t_string!(i18n, deposit.already.verified),
        false => t_string!(i18n, deposit.already.pending),
    };
    let verified_class = if info.verified {
        "badge badge-success"
    } else {
        "badge badge-warning"
    };
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);
    // Refund window check: mirrors bethere-escrow refund instruction's
    // two-path model (checked-in → [event_end, ∞); no-show →
    // [event_end, refund_deadline)). The on-chain program is the source of
    // truth; this gate is advisory and hides the CTA to avoid offering a TX
    // that would revert. Fails safe on missing data. See
    // `event_refund_window_open` for the full contract.
    let event_ended =
        event_refund_window_open(data.event_end_ms, data.refund_deadline_ms, data.checked_in);

    let data_clone_for_refund = data.clone();
    let data_clone_for_refund_info = data.clone();
    let data_clone_for_event_link = data.clone();
    let pending_usdc = !info.verified
        && info.method == DepositMethod::Usdc
        && info
            .tx_signature
            .as_deref()
            .is_some_and(|signature| !signature.is_empty());
    let pending_event_id = info.event_id.clone();
    let pending_attendee_id = info.attendee_id.clone();
    let pending_data = data.clone();
    let (checking_confirmation, set_checking_confirmation) = signal(false);
    let (confirmation_message, set_confirmation_message) = signal(None::<ConfirmCheck>);

    // Currency codes are compared, never translated: "THB" is the stored code.
    let amount_display = if info.currency == "THB" {
        format!("฿{}", info.amount)
    } else {
        format!("{} {}", format_usdc(info.amount), info.currency)
    };
    // Non-USDC (THB / rolling credit) refund guidance — computed here (not inside
    // the view) to avoid borrowing `info`. THB refund/credit actions live on the
    // TICKET page, so point there; never mislabel a ฿ deposit as USDC.
    let nonusdc_amount_display = amount_display.clone();
    let nonusdc_ticket_href = if info.event_id.is_empty() {
        format!("/ticket/{}", info.attendee_id)
    } else {
        format!("/ticket/{}?event_id={}", info.attendee_id, info.event_id)
    };
    let deposited_at = format_timestamp(&info.deposited_at);

    let set_state = *set_state;

    view! {
        <div class="dep2-card">
            // Header: title + badge
            <div class="dep2-card-header">
                <span class="dep2-card-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.already.title))}</span>
                <span class=verified_class>
                    {verified_text}
                </span>
            </div>

            // Success icon (only if verified)
            {if info.verified {
                view! {
                    <div class="dep2-success-icon"><Icon icon=IconName::Check class="icon-lg" /></div>
                }.into_any()
            } else {
                ().into_any()
            }}

            // Amount hero
            <div class="dep2-amount-hero">
                {t!(i18n, deposit.already.amount_deposited, amount = amount_display.clone())}
            </div>

            // Receipt block
            <div class="dep2-receipt">
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.already.method))}</span>
                    <span class="dep2-receipt-value">
                        {display_method_label}
                    </span>
                </div>
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.already.amount))}</span>
                    <span class="dep2-receipt-value">
                        {amount_display.clone()}
                    </span>
                </div>
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.already.status))}</span>
                    <span class="dep2-receipt-value">
                        <span class=verified_class>
                            {verified_text}
                        </span>
                    </span>
                </div>
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.already.date))}</span>
                    <span class="dep2-receipt-value">
                        {deposited_at}
                    </span>
                </div>
            </div>

            // Refund info section
            {if info.verified && info.method == DepositMethod::Usdc {
                let data_clone_for_refund = data_clone_for_refund.clone();
                let data_for_info = data_clone_for_refund_info.clone();
                let usdc_secured = usdc_fmt.clone();
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            {t!(i18n, deposit.already.usdc_secured, amount = usdc_secured)}
                        </p>
                    </div>
                    // Refund deadline
                    {move || match compute_refund_info(i18n.get_locale(), &data_for_info) {
                        Some((deadline, duration)) => view! {
                            <div class="dep2-deadline dep2-deadline--warning">
                                <span class="dep2-deadline-text">
                                    {t!(i18n, deposit.already.refund_window, duration, deadline)}
                                </span>
                            </div>
                        }.into_any(),
                        None => ().into_any(),
                    }}
                    // Refund CTA — three states:
                    //   (a) refundable tier AND event ended → claim button
                    //   (b) refundable tier AND event not ended → "available after event" notice
                    //   (c) not refundable tier → muted badge (unchanged)
                    {if info.refundable && event_ended {
                        let data_clone_for_refund = data_clone_for_refund.clone();
                        let claim_amount = usdc_fmt.clone();
                        view! {
                            <div class="dep2-refund-cta">
                                <span class="dep2-refund-cta-text">
                                    {t!(i18n, deposit.already.claim_now, amount = claim_amount)}
                                </span>
                                <button
                                    class="btn btn-success btn-block"
                                    on:click=move |_| {
                                        set_state.set(DepositPageState::RefundChooseWallet(data_clone_for_refund.clone()));
                                    }
                                >
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.claim_refund))}
                                </button>
                            </div>
                        }.into_any()
                    } else if info.refundable {
                        view! {
                            <div class="dep2-info-note">
                                <p class="hint-note">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.already.refund_after_event))}
                                </p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <span class="badge badge-muted">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.already.non_refundable))}</span>
                        }.into_any()
                    }}
                }.into_any()
            } else if !info.verified {
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            {move || match pending_usdc {
                                true => t_string!(i18n, deposit.already.usdc_recorded),
                                false => t_string!(i18n, deposit.already.refund_after_verify),
                            }}
                        </p>
                        {if pending_usdc {
                            let event_id = pending_event_id.clone();
                            let attendee_id = pending_attendee_id.clone();
                            let data = pending_data.clone();
                            view! {
                                <button
                                    class="btn btn-outline btn-block"
                                    disabled=move || checking_confirmation.get()
                                    on:click=move |_| {
                                        let event_id = event_id.clone();
                                        let attendee_id = attendee_id.clone();
                                        let data = data.clone();
                                        set_checking_confirmation.set(true);
                                        set_confirmation_message.set(None);
                                        leptos::task::spawn_local(async move {
                                            match api::confirm_deposit(&event_id, &attendee_id).await {
                                                Ok(result) if result.confirmed => {
                                                    if let Some(signature) = result.tx_signature {
                                                        set_state.set(DepositPageState::DepositConfirmed(data, signature));
                                                    } else {
                                                        set_confirmation_message.set(Some(ConfirmCheck::Syncing));
                                                    }
                                                }
                                                Ok(_) => set_confirmation_message.set(Some(ConfirmCheck::Pending)),
                                                Err(_) => set_confirmation_message.set(Some(ConfirmCheck::Unavailable)),
                                            }
                                            set_checking_confirmation.set(false);
                                        });
                                    }
                                >
                                    {move || if checking_confirmation.get() {
                                        t_string!(i18n, deposit.already.checking)
                                    } else {
                                        t_string!(i18n, deposit.already.check_again)
                                    }}
                                </button>
                                {move || confirmation_message.get().map(|check| {
                                    let message = match check {
                                        ConfirmCheck::Syncing => t_string!(i18n, deposit.already.confirm_syncing),
                                        ConfirmCheck::Pending => t_string!(i18n, deposit.already.confirm_pending),
                                        ConfirmCheck::Unavailable => t_string!(i18n, deposit.already.confirm_unavailable),
                                    };
                                    view! {
                                        <p class="hint-note" role="status">{message}</p>
                                    }
                                })}
                            }.into_any()
                        } else {
                            ().into_any()
                        }}
                    </div>
                }.into_any()
            } else if is_credit {
                let amount = nonusdc_amount_display.clone();
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            {t!(i18n, deposit.already.credit_note, amount)}
                        </p>
                    </div>
                }.into_any()
            } else {
                // Verified THB deposit. Refund and hold-as-credit actions live on
                // the ticket page — surface the choice honestly (no "USDC" label).
                let amount = nonusdc_amount_display.clone();
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            {t!(i18n, deposit.already.thb_secured, amount)}
                        </p>
                        <a href=nonusdc_ticket_href class="btn btn-outline btn-block">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.already.go_ticket))}
                        </a>
                    </div>
                }.into_any()
            }}

            // Back to event link
            <a
                href=if data_clone_for_event_link.event_slug.is_empty() { "/".to_string() } else { format!("/e/{}", data_clone_for_event_link.event_slug) }
                class="dep2-back"
            >
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.back_event))}
            </a>
        </div>
    }
    .into_any()
}
