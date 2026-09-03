//! State view helper components and state-view rendering.

use leptos::prelude::*;

use crate::icons::{Icon, IconName};
use crate::utils;

use super::interop::*;
use super::logic::*;
use super::state::*;

// ===== State View Helper Components =====

/// QR code card for claim URLs — displayed on Success and AlreadyCheckedIn.
#[component]
pub(super) fn ClaimQrCard(qr_src: String, claim_url: String, label: &'static str) -> impl IntoView {
    view! {
        <div class="scanner-qr-wrapper">
            <div class="scanner-qr-card">
                <p class="scanner-qr-label">{label}</p>
                <img src=qr_src alt="Claim URL QR Code" class="scanner-qr-img" />
                <button class="btn btn-primary btn-sm scanner-qr-copy-btn"
                    on:click=move |_| { let _ = copy_to_clipboard_js(&claim_url); }>
                    <Icon icon=IconName::Copy class="icon-sm" />" Copy Link"
                </button>
            </div>
        </div>
    }
}

/// Attendee name + email info block.
#[component]
pub(super) fn AttendeeInfoCard(name: String, email: String) -> impl IntoView {
    view! {
        <div class="scanner-attendee-info">
            <p class="scanner-attendee-name">{name}</p>
            <p class="scanner-attendee-email">{email}</p>
        </div>
    }
}

// ===== State View Rendering =====

/// Render the current check-in state as a view.
#[allow(clippy::too_many_arguments)] // internal render helper; splitting the signature would obscure the reactive wiring
pub(super) fn render_check_in_state<E1, E2, E3, E4, E5>(
    state: CheckInState,
    on_check_in: impl Fn(web_sys::MouseEvent) + 'static,
    on_reset: impl Fn(web_sys::MouseEvent) + Clone + 'static,
    on_escrow_check_in: E1,
    on_escrow_wallet_connect: E2,
    on_escrow_sign: E3,
    escrow_enabled: bool,
    wallets: Vec<String>,
    on_online_check_in: E4,
    // Undo check-in
    on_undo: E5,
    undo_confirm: ReadSignal<bool>,
    undo_timer_secs: ReadSignal<u32>,
    undo_expired: ReadSignal<bool>,
) -> AnyView
where
    E1: Fn(web_sys::MouseEvent) + Clone + 'static,
    E2: Fn(String) + Clone + 'static,
    E3: Fn(web_sys::MouseEvent) + Clone + 'static,
    E4: Fn(web_sys::MouseEvent) + Clone + 'static,
    E5: Fn(web_sys::MouseEvent) + Clone + Send + 'static,
{
    match state {
        CheckInState::Idle => view! { <div></div> }.into_any(),
        CheckInState::LookingUp => view! {
            <div class="scanner-state-loading">
                <div class="page-loading">
                    <span class="spinner spinner-lg"></span>
                    <span>"Looking up attendee..."</span>
                </div>
            </div>
        }
        .into_any(),
        CheckInState::Found(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let ticket = data.attendee.ticket_name.clone();
            let participation = data.participation_type.clone();
            let badge = utils::get_participation_badge(&participation);
            view! {
                <div>
                    <div class="scanner-state-header">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2>"Ready to Check In"</h2>
                    </div>
                    <AttendeeInfoCard name=name email=email />
                    <div class="scanner-attendee-badges">
                        <span class="badge badge-info badge-pill">{ticket}</span>
                        <span class=format!("badge badge-pill {}", badge.css_class)>{badge.label}</span>
                    </div>
                    <div class="scanner-actions">
                        <button class="btn btn-success btn-block" on:click=on_check_in>
                            <Icon icon=IconName::Check class="icon-sm icon-success" />" Confirm Check-In"
                        </button>
                    </div>
                    <button class="btn btn-outline btn-block scanner-mt-half" on:click=on_reset>
                        "Cancel"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::AlreadyCheckedIn(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let checked_at = data.attendee.checked_in_at.clone().unwrap_or_default();
            let formatted = utils::format_timestamp(&checked_at);
            let by_suffix = data
                .attendee
                .checked_in_by
                .as_ref()
                .map_or(String::new(), |by| {
                    if by.is_empty() {
                        String::new()
                    } else {
                        format!(" by {}", utils::escape_html(by))
                    }
                });
            let claim_url = data.attendee.claim_token.as_ref().map(|t| build_claim_url(t));
            let qr_data_url = claim_url
                .as_ref()
                .and_then(|url| generate_qr_data_url(url, 240));
            let claim_url_for_display = claim_url.clone();
            view! {
                <div>
                    <div class="result-warning">
                        <h2>"Already Checked In"</h2>
                        <AttendeeInfoCard name=name email=email />
                        <p class="scanner-result-detail-line">
                            "Checked in at: "{formatted}{by_suffix}
                        </p>
                    </div>

                    // Claim URL QR code — re-show in case staff needs to display it again
                    {move || {
                        match (&qr_data_url, &claim_url_for_display) {
                            (Some(img_src), Some(url)) => {
                                view! {
                                    <ClaimQrCard
                                        qr_src=img_src.clone()
                                        claim_url=url.clone()
                                        label="Claim QR (show to attendee):"
                                    />
                                }
                                    .into_any()
                            }
                            _ => view! { <div></div> }.into_any(),
                        }
                    }}

                    <button class="btn btn-outline btn-block scanner-mt-1" on:click=on_reset>
                        "Scan Another"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::NotApproved(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let status_label = data.attendee.approval_status.label();
            view! {
                <div>
                    <div class="result-error">
                        <h2>"Not Approved"</h2>
                        <AttendeeInfoCard name=name email=email />
                        <p class="scanner-result-detail-line">
                            "Status: "
                            <span class="scanner-status-warning">{status_label}</span>
                        </p>
                    </div>
                    <button class="btn btn-outline btn-block scanner-mt-1" on:click=on_reset>
                        "Scan Another"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::NotInPerson(data) => {
            let name = data.attendee.name.clone();
            let email = data.attendee.email.clone();
            let badge = utils::get_participation_badge(&data.participation_type);
            let on_online = on_online_check_in.clone();
            view! {
                <div>
                    <div class="result-warning">
                        <h2>"Online Attendee"</h2>
                        <AttendeeInfoCard name=name email=email />
                        <div class="scanner-attendee-badges">
                            <span class=format!("badge badge-pill {}", badge.css_class)>{badge.label}</span>
                        </div>
                        <p class="scanner-hint scanner-mt-075">
                            "This attendee registered for the online track. You can perform a virtual check-in to generate their claim link."
                        </p>
                    </div>
                    <button
                        class="btn btn-primary btn-block scanner-mt-1"
                        on:click=on_online
                    >
                        <Icon icon=IconName::Globe class="icon-sm" />" Virtual Check-In"
                    </button>
                    <button class="btn btn-outline btn-block scanner-mt-half" on:click=on_reset>
                        "Scan Another"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::NotFound => view! {
            <div>
                <div class="result-error">
                    <h2>"Not Found"</h2>
                    <div class="result-details">
                        <p>"No matching attendee found. Please try again."</p>
                    </div>
                </div>
                <button
                    class="btn btn-outline btn-block scanner-mt-1"
                    on:click=on_reset
                >
                    "Try Again"
                </button>
            </div>
        }
        .into_any(),
        CheckInState::CheckingIn { name, .. } => view! {
            <div class="scanner-state-loading">
                <div class="page-loading">
                    <span class="spinner spinner-lg"></span>
                    <span>"Checking in "{name}"..."</span>
                </div>
            </div>
        }
        .into_any(),
        CheckInState::Success(result) => {
            let name = result.name.clone();
            let checked_at = result.checked_in_at.clone();
            let formatted = utils::format_timestamp(&checked_at);
            let by_suffix = {
                let by = result.checked_in_by.clone();
                if by.is_empty() {
                    String::new()
                } else {
                    format!(" by {}", utils::escape_html(&by))
                }
            };
            let claim_url = result.claim_token.as_ref().map(|t| build_claim_url(t));
            let qr_data_url = claim_url
                .as_ref()
                .and_then(|url| generate_qr_data_url(url, 240));
            let claim_url_for_display = claim_url.clone();
            let show_escrow = escrow_enabled;
            view! {
                <div>
                    <div class="result-success">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2 class="claim-success-title">"Checked In!"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p>"Checked in at: "{formatted}{by_suffix}</p>
                        </div>
                    </div>

                    // Claim URL QR code — show to attendee so they can scan it
                    {move || {
                        match (&qr_data_url, &claim_url_for_display) {
                            (Some(img_src), Some(url)) => {
                                view! {
                                    <ClaimQrCard
                                        qr_src=img_src.clone()
                                        claim_url=url.clone()
                                        label="Show this QR to the attendee to claim their NFT:"
                                    />
                                }
                                    .into_any()
                            }
                            _ => view! { <div></div> }.into_any(),
                        }
                    }}

                    // On-chain escrow check-in button (if event has escrow enabled)
                    {if show_escrow {
                        view! {
                            <button
                                class="btn btn-outline btn-block scanner-btn-escrow"
                                on:click=on_escrow_check_in
                            >
                                <Icon icon=IconName::Lock class="icon-sm" />" Mark Checked In On-Chain"
                            </button>
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }}

                    <button class="btn btn-success btn-block scanner-mt-half" on:click=on_reset>
                        "Scan Next"
                    </button>

                    // Undo check-in button — available for 30 seconds after check-in
                    {move || {
                        let expired = undo_expired.get();
                        let confirmed = undo_confirm.get();
                        let secs = undo_timer_secs.get();
                        if expired {
                            view! { <div></div> }.into_any()
                        } else if confirmed {
                            view! {
                                <button
                                    class="btn btn-danger btn-block scanner-undo-confirm"
                                    on:click=on_undo.clone()
                                >
                                    "\u{26a0} Confirm Undo?"
                                </button>
                                <p class="scanner-hint scanner-hint-timer">
                                    {format!("Undo available for {}s", secs)}
                                </p>
                            }.into_any()
                        } else {
                            view! {
                                <button
                                    class="btn btn-danger btn-sm btn-block scanner-undo-btn"
                                    on:click=on_undo.clone()
                                >
                                    "\u{21a9} Undo Check-In"
                                </button>
                                <p class="scanner-hint scanner-hint-timer">
                                    {format!("Undo available for {}s", secs)}
                                </p>
                            }.into_any()
                        }
                    }}
                </div>
            }
            .into_any()
        }
        CheckInState::Error => view! {
            <div>
                <div class="result-error">
                    <h2>"Error"</h2>
                    <div class="result-details">
                        <p>"Something went wrong. Please try again."</p>
                    </div>
                </div>
                <button
                    class="btn btn-outline btn-block scanner-mt-1"
                    on:click=on_reset
                >
                    "Try Again"
                </button>
            </div>
        }
        .into_any(),

        // --- Escrow on-chain check-in states ---
        CheckInState::EscrowChooseWallet { check_in_data, .. } => {
            let name = check_in_data.name.clone();
            view! {
                <div>
                    <div class="result-success">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2>"On-Chain Check-In"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p>"Connect your organizer wallet to record check-in on Solana."</p>
                        </div>
                    </div>

                    <div class="scanner-mt-1">
                        {if wallets.is_empty() {
                            view! {
                                <div class="result-warning scanner-wallet-warning">
                                    <p class="scanner-wallet-warning-text">"No Solana wallet detected. Install Phantom or Solflare."</p>
                                </div>
                            }
                                .into_any()
                        } else {
                            let cb = on_escrow_wallet_connect.clone();
                            wallets
                                .into_iter()
                                .map(move |w| {
                                    let w_clone = w.clone();
                                    let cb = cb.clone();
                                    view! {
                                        <button
                                            class="btn btn-outline btn-block scanner-wallet-connect-btn"
                                            on:click=move |_| cb(w_clone.clone())
                                        >
                                            {format!("Connect {}", w)}
                                        </button>
                                    }
                                })
                                .collect::<Vec<_>>()
                                .into_any()
                        }}
                    </div>

                    <button class="btn btn-outline btn-block scanner-mt-half" on:click=on_reset>
                        "Skip & Scan Next"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::EscrowWalletConnected { check_in_data, wallet_name, public_key, .. } => {
            let name = check_in_data.name.clone();
            let short_pk = if public_key.len() > 8 {
                format!("{}...{}", &public_key[..4], &public_key[public_key.len()-4..])
            } else {
                public_key.clone()
            };
            let wallet_label = wallet_name.clone();
            view! {
                <div>
                    <div class="result-success">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2>"Ready to Sign"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p>
                                <span class="scanner-muted-text">{format!("{} ({})", wallet_label, short_pk)}</span>
                            </p>
                        </div>
                    </div>

                    <button
                        class="btn btn-primary btn-block scanner-mt-1"
                        on:click=on_escrow_sign
                    >
                        <Icon icon=IconName::Lock class="icon-sm" />" Sign On-Chain Check-In"
                    </button>

                    <button class="btn btn-outline btn-block scanner-mt-half" on:click=on_reset>
                        "Skip & Scan Next"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::EscrowSigning { wallet_name } => view! {
            <div class="scanner-state-loading">
                <div class="page-loading">
                    <span class="spinner spinner-lg"></span>
                    <span>{format!("Waiting for {} to approve...", wallet_name)}</span>
                </div>
            </div>
        }
        .into_any(),
        CheckInState::EscrowConfirmed { check_in_data, signature } => {
            let name = check_in_data.name.clone();
            let short_sig = if signature.len() > 16 {
                format!("{}...{}", &signature[..8], &signature[signature.len()-8..])
            } else {
                signature.clone()
            };
            view! {
                <div>
                    <div class="result-success">
                        <div class="success-check">
                            <svg viewBox="0 0 24 24">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        </div>
                        <h2>"On-Chain Check-In Confirmed!"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p class="scanner-tx-detail">
                                {format!("TX: {}", short_sig)}
                            </p>
                            <a
                                href={utils::solscan_tx_url(&signature, &utils::get_cluster())}
                                target="_blank"
                                rel="noopener noreferrer"
                                class="scanner-solscan-link"
                            >
                                "View on Solscan ↗"
                            </a>
                        </div>
                    </div>

                    <button class="btn btn-success btn-block scanner-mt-1" on:click=on_reset>
                        "Scan Next"
                    </button>
                </div>
            }
            .into_any()
        }
        CheckInState::EscrowError { check_in_data, message } => {
            let name = check_in_data.name.clone();
            view! {
                <div>
                    <div class="result-error">
                        <h2>"On-Chain Check-In Failed"</h2>
                        <div class="result-details">
                            <p class="scanner-attendee-name">{name}</p>
                            <p class="scanner-error-warning">
                                {message}
                            </p>
                        </div>
                    </div>

                    <button class="btn btn-outline btn-block scanner-mt-1" on:click=on_reset>
                        "Skip & Scan Next"
                    </button>
                </div>
            }
            .into_any()
        }
        // Walk-in states are rendered directly in the view; these arms should not be reached.
        CheckInState::WalkinForm => view! { <div></div> }.into_any(),
        CheckInState::WalkinRegistering => view! { <div></div> }.into_any(),
        CheckInState::WalkinSuccess { .. } => view! { <div></div> }.into_any(),
        CheckInState::WalkinCapacityWarning { .. } => view! { <div></div> }.into_any(),
    }
}
