//! In-person attendee view — hero, QR, info, deposit, status badges.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use super::action_cards::*;
use super::event_context::EventContext;
use super::nft_badge::NftClaimedBadge;
use super::qr_section::QrSection;
use super::video_section::VideoSection;
use super::view_data::TicketViewData;
use crate::icons::{Icon, IconName};
use crate::utils;

#[wasm_bindgen(module = "/js/qr_generate.js")]
extern "C" {
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;
}

/// In-person attendee view component.
#[component]
pub fn InPersonView(
    /// Pre-computed view data
    view_data: TicketViewData,
    /// Whether QR section is expanded
    show_qr: ReadSignal<bool>,
    /// Toggle QR expansion
    set_show_qr: WriteSignal<bool>,
    /// Open fullscreen QR overlay
    set_fullscreen_qr: WriteSignal<bool>,
) -> impl IntoView {
    // Clone for QR section before consuming view_data
    let qr_view_data = view_data.clone();

    let TicketViewData {
        qr_image: _,
        has_qr: _,
        name,
        ticket_name,
        participation,
        masked_email,
        is_checked_in,
        is_approved,
        claimed,
        claimed_asset_id,
        status_detail,
        claim_href,
        has_claim,
        deposit_enabled,
        deposit_deadline_hours,
        deposit_amount_thb,
        deposit_amount_usdc,
        deadline_expired,
        in_person_available,
        refund_link,
        deposit_info,
        escrow_closed,
        has_video,
        video_url,
        event_tagline,
        event_location,
        event_link,
        nft_image_url,
        deposit_href,
        orb_link,
        rollover_target_event,
        ..
    } = view_data;

    // Determine hero variant
    let (hero_variant, hero_icon, hero_title, hero_subtitle) = if is_checked_in {
        (
            "ticket-hero--checked-in".to_string(),
            IconName::Check,
            "Checked In".to_string(),
            status_detail.clone(),
        )
    } else if !is_approved {
        (
            "ticket-hero--pending".to_string(),
            IconName::Clock,
            "Pending Approval".to_string(),
            String::new(),
        )
    } else if deposit_info.as_ref().is_some_and(|d| !d.verified) {
        (
            "ticket-hero--pending".to_string(),
            IconName::Hourglass,
            "Awaiting Deposit Verification".to_string(),
            String::new(),
        )
    } else {
        (
            "ticket-hero--ready".to_string(),
            IconName::QrCode,
            "Ready for Check-In".to_string(),
            String::new(),
        )
    };

    // NFT hero section (pre-computed to avoid FnOnce issues)
    let nft_hero = if is_checked_in && claimed {
        let asset_id = claimed_asset_id.clone().unwrap_or_default();
        Some(("claimed", asset_id, orb_link.unwrap_or_default()))
    } else if is_checked_in && !claimed && has_claim {
        Some(("cta", String::new(), String::new()))
    } else {
        None
    };

    view! {
        // 1. Hero banner
        <super::hero::TicketHero
            variant=hero_variant
            icon=hero_icon
            title=hero_title
            subtitle=hero_subtitle
        />

        // 2. Main card
        <div class="ticket-main-card">

            // ── NFT/Claim hero section ──
            {match &nft_hero {
                Some(("claimed", asset_id, ol)) => {
                    let aid = asset_id.clone();
                    let orb = ol.clone();
                    view! {
                        <NftClaimedBadge
                            asset_id=aid
                            orb_link=orb
                            on_copy=Box::new(|text| copy_to_clipboard_js(text))
                        />
                    }.into_any()
                }
                Some(("cta", _, _)) => {
                    view! {
                        <ClaimActionCard claim_href=claim_href.clone() />
                    }.into_any()
                }
                _ => view! { <div></div> }.into_any(),
            }}

            // ── Event context ──
            <EventContext
                nft_image_url=nft_image_url.clone()
                tagline=event_tagline.clone()
                location=event_location.clone()
                event_link=event_link.clone()
            />

            // ── QR Code section ──
            <QrSection
                view_data=qr_view_data
                show_qr=show_qr
                set_show_qr=set_show_qr
                set_fullscreen_qr=set_fullscreen_qr
            />

            // ── Attendee info ──
            <div class="ticket-info">
                <div class="ticket-info-row">
                    <span class="ticket-info-label">"Name"</span>
                    <span class="ticket-info-value">
                        {utils::escape_html(&name)}
                    </span>
                </div>
                {if !masked_email.is_empty() {
                    let email = masked_email;
                    view! {
                        <div class="ticket-info-row">
                            <span class="ticket-info-label">"Email"</span>
                            <span class="ticket-info-value">
                                {utils::escape_html(&email)}
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                {if !ticket_name.is_empty() {
                    let tn = ticket_name;
                    view! {
                        <div class="ticket-info-row">
                            <span class="ticket-info-label">"Ticket"</span>
                            <span class="ticket-info-value">
                                {utils::escape_html(&tn)}
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                {if !participation.is_empty() {
                    let pt = participation;
                    view! {
                        <div class="ticket-info-row">
                            <span class="ticket-info-label">"Type"</span>
                            <span class="ticket-info-value">
                                {utils::escape_html(&pt)}
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>

            // ── Status section: deposit/refund/claim action cards ──
            // Deposit status
            {if let Some(ref dep) = deposit_info {
                view! {
                    {if dep.verified {
                        view! {
                            <DepositVerifiedCard />
                        }.into_any()
                    } else {
                        view! {
                            <DepositPendingCard method=dep.method.clone() />
                        }.into_any()
                    }}
                    // Rollover opportunity (checked-in with verified USDC deposit and target event available)
                    {if let Some(ref target) = rollover_target_event {
                        if dep.verified && is_checked_in {
                            let target_name = target.event_name.clone();
                            let target_event_id = target.event_id.clone();
                            view! {
                                <RolloverActionCard
                                    target_event_name=target_name
                                    on_rollover=Box::new(move || {
                                        // TODO: trigger wallet signing flow
                                        log::info!("[rollover] clicked for target event: {}", target_event_id);
                                    })
                                />
                            }.into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }
                    } else {
                        view! { <div></div> }.into_any()
                    }}
                    {if dep.refunded {
                        view! {
                            <RefundCard refund_proof_url=dep.refund_proof_url.clone().unwrap_or_default() />
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }}
                }.into_any()
            } else if deposit_enabled && deadline_expired && in_person_available.unwrap_or(false) && !escrow_closed {
                view! {
                    <ReclaimActionCard reclaim_href=deposit_href.clone() />
                }.into_any()
            } else if deposit_enabled && deadline_expired && !in_person_available.unwrap_or(true) && !escrow_closed {
                view! {
                    <MovedOnlineCard />
                }.into_any()
            } else if deposit_enabled && !is_checked_in && !escrow_closed {
                view! {
                    <DepositActionCard
                        amount_usdc=deposit_amount_usdc
                        amount_thb=deposit_amount_thb
                        escrow_closed=escrow_closed
                        deadline_hours=deposit_deadline_hours
                        deposit_href=deposit_href.clone()
                    />
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}

            // Organizer refund link (from Google Sheet, independent of deposit)
            {if let Some(link) = refund_link {
                if !link.is_empty() {
                    view! {
                        <div class="ticket-action-card ticket-action-card--info">
                            <div class="ticket-action-icon">
                                <Icon icon=IconName::Link class="icon-sm" />
                            </div>
                            <div>
                                <div class="ticket-action-title">"Organizer Refund Link"</div>
                                <a
                                    href=link
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    class="ticket-action-link"
                                >
                                    "View Refund Details →"
                                </a>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            } else {
                view! { <div></div> }.into_any()
            }}

            // Status badge
            {if is_checked_in {
                let detail = status_detail.clone();
                view! {
                    <div class="ticket-action-card ticket-action-card--verified">
                        <div class="ticket-action-icon">
                            <Icon icon=IconName::Check class="icon-sm" />
                        </div>
                        <div>
                            <div class="ticket-action-title">"Checked In"</div>
                            {if !detail.is_empty() {
                                view! {
                                    <div class="ticket-action-desc">{detail}</div>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }}
                        </div>
                    </div>
                }.into_any()
            } else if !is_approved {
                view! {
                    <div class="ticket-action-card ticket-action-card--pending">
                        <div class="ticket-action-icon">
                            <Icon icon=IconName::Clock class="icon-sm" />
                        </div>
                        <div>
                            <div class="ticket-action-title">"Pending Approval"</div>
                            <div class="ticket-action-desc">
                                "Your registration is being reviewed."
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else if deposit_info.as_ref().is_some_and(|d| !d.verified) {
                view! {
                    <div class="ticket-action-card ticket-action-card--pending">
                        <div class="ticket-action-icon">
                            <Icon icon=IconName::Hourglass class="icon-sm" />
                        </div>
                        <div>
                            <div class="ticket-action-title">"Awaiting Deposit Verification"</div>
                            <div class="ticket-action-desc">
                                "Your deposit is being verified. QR code will appear once confirmed."
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="ticket-action-card ticket-action-card--ready">
                        <div class="ticket-action-icon">
                            <Icon icon=IconName::QrCode class="icon-sm" />
                        </div>
                        <div>
                            <div class="ticket-action-title">"Ready for Check-In"</div>
                            <div class="ticket-action-desc">
                                "Show this QR code to staff at the event."
                            </div>
                        </div>
                    </div>
                }.into_any()
            }}
        </div>

        // 5. Video section
        {if has_video {
            view! {
                <VideoSection video_url=video_url.clone() variant="card".to_string() />
            }.into_any()
        } else {
            view! { <div></div> }.into_any()
        }}

        // 6. Footer
        <div class="ticket-footer">
            <div class="ticket-nav">
                <a href="/">"← Home"</a>
            </div>
            {if is_checked_in {
                view! {
                    <p class="ticket-footer-hint">
                        "You're checked in! Enjoy the event."
                    </p>
                }.into_any()
            } else if !is_approved {
                view! {
                    <p class="ticket-footer-hint">
                        "Your registration is being reviewed. You'll receive a QR code once approved."
                    </p>
                }.into_any()
            } else {
                view! {
                    <p class="ticket-footer-hint">
                        "Present this ticket at the registration desk for check-in."
                    </p>
                }.into_any()
            }}
        </div>
    }
}
