//! In-person attendee view — hero, QR, info, deposit, status badges.

use event_checkin_domain::models::attendee::ParticipationType;
use leptos::prelude::*;
use leptos_router::components::A;
use wasm_bindgen::prelude::*;

use super::access_logistics::access_logistics_section;
use super::action_cards::*;
use super::calendar_links::CalendarLinks;
use super::credit_chip::CreditBalanceChip;
use super::event_context::EventContext;
use super::nft_badge::NftClaimedBadge;
use super::qr_section::QrSection;
use super::video_section::VideoSection;
use super::view_data::TicketViewData;
use crate::api::DepositMethod;
use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName};
use crate::utils;

#[wasm_bindgen(module = "/js/clipboard.js")]
extern "C" {
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;
}

/// Which status the in-person hero shows.
#[derive(Clone, Copy)]
enum HeroKind {
    CheckedIn,
    PendingApproval,
    AwaitingDeposit,
    Ready,
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
        ticket_note,
        name,
        ticket_name,
        participation,
        masked_email,
        is_checked_in,
        is_approved,
        claimed,
        claimed_asset_id,
        checked_in_at,
        checked_in_by,
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
        event_name,
        event_tagline,
        event_location,
        event_location_map_url,
        event_link,
        nft_image_url,
        deposit_href,
        orb_link,
        event_id,
        api_id,
        rollover_target_event,
        community_links,
        calendar_subscribe_url,
        ..
    } = view_data;

    // Split community links: "guide"-tagged entries are logistics guides
    // (building access, ID exchange, transportation) routed to the dedicated
    // Access & Logistics card; everything else stays in "Join the Community".
    // See `access_logistics::GUIDE_PLATFORM`.
    let (guide_links, social_links): (Vec<_>, Vec<_>) = community_links
        .into_iter()
        .partition(|l| l.platform == super::access_logistics::GUIDE_PLATFORM);

    let i18n = use_i18n();

    // Determine hero variant
    let hero_kind = if is_checked_in {
        HeroKind::CheckedIn
    } else if !is_approved {
        HeroKind::PendingApproval
    } else if deposit_info.as_ref().is_some_and(|d| !d.verified) {
        HeroKind::AwaitingDeposit
    } else {
        HeroKind::Ready
    };
    let (hero_variant, hero_icon) = match hero_kind {
        HeroKind::CheckedIn => ("ticket-hero--checked-in", IconName::Check),
        HeroKind::PendingApproval => ("ticket-hero--pending", IconName::Clock),
        HeroKind::AwaitingDeposit => ("ticket-hero--pending", IconName::Hourglass),
        HeroKind::Ready => ("ticket-hero--ready", IconName::QrCode),
    };
    let hero_title = move || match hero_kind {
        HeroKind::CheckedIn => t_string!(i18n, ticket.hero.checked_in),
        HeroKind::PendingApproval => t_string!(i18n, ticket.hero.pending_approval),
        HeroKind::AwaitingDeposit => t_string!(i18n, ticket.hero.awaiting_deposit),
        HeroKind::Ready => t_string!(i18n, ticket.hero.ready),
    };
    // Checked-in detail: "<time> by <staff>", either part may be missing.
    let check_in_time = checked_in_at.filter(|ts| !ts.is_empty());
    let check_in_by = checked_in_by
        .filter(|by| !by.is_empty())
        .map(|by| utils::escape_html(&by));
    let hero_subtitle: Option<ViewFn> = match (is_checked_in, check_in_time, check_in_by) {
        (false, _, _) | (true, None, None) => None,
        (true, time, by) => Some(ViewFn::from(move || {
            let time = time.clone();
            let by = by.clone();
            view! {
                {move || time.as_deref().map(super::view_data::format_check_in_time)}
                {by.map(|name| t!(i18n, ticket.hero.checked_in_by, name))}
            }
        })),
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
        {match hero_subtitle {
            Some(sub) => view! {
                <super::hero::TicketHero
                    variant=hero_variant
                    icon=hero_icon
                    title=hero_title
                    subtitle=sub
                />
            }.into_any(),
            None => view! {
                <super::hero::TicketHero variant=hero_variant icon=hero_icon title=hero_title />
            }.into_any(),
        }}

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
                            on_copy=Box::new(copy_to_clipboard_js)
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
                name=event_name.clone()
                tagline=event_tagline.clone()
                location=event_location.clone()
                location_map_url=event_location_map_url.clone()
                event_link=event_link.clone()
            />

            // ── Calendar links ──
            <CalendarLinks subscribe_url=calendar_subscribe_url.clone() />

            // ── QR Code section ──
            <QrSection
                view_data=qr_view_data
                show_qr=show_qr
                set_show_qr=set_show_qr
                set_fullscreen_qr=set_fullscreen_qr
            />

            // ── Access & Logistics (in-person only) ──
            // Building access / ID exchange / transportation guides.
            // Empty (hidden) unless the organizer configured guide links.
            {super::announcement::announcement_section(ticket_note.clone())}
            {access_logistics_section(guide_links.clone())}

            // ── Attendee info ──
            <div class="ticket-info">
                <div class="ticket-info-row">
                    <span class="ticket-info-label">{t!(i18n, ticket.info.name)}</span>
                    <span class="ticket-info-value">
                        {utils::escape_html(&utils::capitalize_name(&name))}
                    </span>
                </div>
                {if !masked_email.is_empty() {
                    let email = masked_email;
                    view! {
                        <div class="ticket-info-row">
                            <span class="ticket-info-label">{t!(i18n, ticket.info.email)}</span>
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
                            <span class="ticket-info-label">{t!(i18n, ticket.info.ticket)}</span>
                            <span class="ticket-info-value">
                                {utils::escape_html(&tn)}
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                {if !participation.is_empty() {
                    // Display label only; the stored value stays `participation`.
                    let kind = ParticipationType::parse(&participation);
                    let pt = move || match kind {
                        ParticipationType::InPerson => t_string!(i18n, ticket.participation.in_person),
                        ParticipationType::Online => t_string!(i18n, ticket.participation.online),
                        ParticipationType::Retrospective => t_string!(i18n, ticket.participation.retrospective),
                        ParticipationType::Other => t_string!(i18n, ticket.participation.other),
                    };
                    view! {
                        <div class="ticket-info-row">
                            <span class="ticket-info-label">{t!(i18n, ticket.info.kind)}</span>
                            <span class="ticket-info-value">
                                {pt}
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>

            // ── Status section: deposit/refund/claim action cards ──
            // Rolling deposit-credit balance (Issue #061 Phase 1 polish). The chip
            // fetches the attendee's own balance on mount and renders nothing when
            // it is zero, so attendees without credit see no change. Gated on
            // check-in because that is when holding a deposit becomes possible.
            {if is_checked_in {
                view! { <CreditBalanceChip /> }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
            // Deposit status
            {if let Some(ref dep) = deposit_info {
                view! {
                    {if dep.verified {
                        view! {
                            <DepositVerifiedCard />
                        }.into_any()
                    } else {
                        view! {
                            <DepositPendingCard method=dep.method />
                        }.into_any()
                    }}
                    // Per-method refund guidance (post-check-in). Surfaces the refund
                    // step that used to be missing/mislabeled: USDC is an attendee-
                    // claimed on-chain refund (deposit page); THB is either kept as
                    // credit (card below) or a cash refund arranged with the organizer.
                    {if dep.verified && is_checked_in && !dep.refunded {
                        match dep.method {
                            DepositMethod::Usdc => {
                                let href = deposit_href.clone();
                                view! {
                                    <div class="ticket-action-card ticket-action-card--info">
                                        <div class="ticket-action-icon"><Icon icon=IconName::Wallet class="icon-sm" /></div>
                                        <div>
                                            <div class="ticket-action-title">{t!(i18n, ticket.usdc_refund.title)}</div>
                                            <p class="ticket-action-desc">{t!(i18n, ticket.usdc_refund.body)}</p>
                                            <a href=href class="btn btn-outline btn-sm ticket-action-btn">{t!(i18n, ticket.usdc_refund.cta)}</a>
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            DepositMethod::Thb if !dep.held_as_credit => {
                                let amount = deposit_amount_thb;
                                view! {
                                    <div class="ticket-action-card ticket-action-card--info">
                                        <div class="ticket-action-icon"><Icon icon=IconName::Info class="icon-sm" /></div>
                                        <div>
                                            <div class="ticket-action-title">{t!(i18n, ticket.thb_options.title)}</div>
                                            <p class="ticket-action-desc">{t!(i18n, ticket.thb_options.body, amount)}</p>
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            _ => view! { <div></div> }.into_any(),
                        }
                    } else {
                        view! { <div></div> }.into_any()
                    }}
                    // Rollover opportunity (checked-in with verified USDC deposit and target event available)
                    {if let Some(ref target) = rollover_target_event {
                        if dep.verified && is_checked_in {
                            let target_name = target.event_name.clone();
                            let target_eid = target.event_id.clone();
                            let source_eid = event_id.clone();
                            let aid = api_id.clone();
                            view! {
                                <RolloverActionCard
                                    deposit_amount_usdc=deposit_amount_usdc
                                    target_event_name=target_name
                                    target_event_id=target_eid
                                    source_event_id=source_eid
                                    attendee_id=aid
                                />
                            }.into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }
                    } else {
                        view! { <div></div> }.into_any()
                    }}
                    // Hold-as-credit opportunity (checked-in with verified THB deposit, not yet refunded).
                    // THB-only counterpart to the USDC RolloverActionCard above.
                    // Backend is idempotent: the source deposit is settled via `held_as_credit`,
                    // which the server reports back and we forward as `already_held`. The card then
                    // mounts in its AlreadyHeld confirmation (no CTA), so reload is safe and the
                    // backend guard is the defense-in-depth backstop (Issue #061 §8 — resolved).
                    {if dep.verified && !dep.refunded && is_checked_in && dep.method == DepositMethod::Thb {
                        let hold_eid = event_id.clone();
                        let hold_aid = api_id.clone();
                        let hold_amount = deposit_amount_thb;
                        let hold_already = dep.held_as_credit;
                        view! {
                            <HoldDepositCard
                                event_id=hold_eid
                                attendee_id=hold_aid
                                deposit_amount_thb=hold_amount
                                already_held=hold_already
                            />
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }}
                    // Phase 3 exit path — "Request Return of Held Credit"
                    // (Issue #061 §D3). Rendered only when the attendee has
                    // actually held their deposit as credit, so the exit is
                    // meaningful. The card fetches its own already-requested
                    // state on mount (mirrors the held_as_credit UX pattern),
                    // so no props are needed.
                    {if dep.held_as_credit {
                        view! {
                            <RequestCreditRefundCard />
                        }.into_any()
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
                                <div class="ticket-action-title">{t!(i18n, ticket.organizer_refund.title)}</div>
                                <a
                                    href=link
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    class="ticket-action-link"
                                >
                                    {t!(i18n, ticket.organizer_refund.cta)}
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

            // Status badge (only for non-checked-in states — hero already shows checked-in status)
            {if !is_approved {
                view! {
                    <div class="ticket-action-card ticket-action-card--pending">
                        <div class="ticket-action-icon">
                            <Icon icon=IconName::Clock class="icon-sm" />
                        </div>
                        <div>
                            <div class="ticket-action-title">{t!(i18n, ticket.hero.pending_approval)}</div>
                            <div class="ticket-action-desc">
                                {t!(i18n, ticket.pending_desc)}
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else if !is_checked_in && deposit_info.as_ref().is_none_or(|d| d.verified) {
                // Ready for check-in: approved + no pending deposit
                view! {
                    <div class="ticket-action-card ticket-action-card--ready">
                        <div class="ticket-action-icon">
                            <Icon icon=IconName::QrCode class="icon-sm" />
                        </div>
                        <div>
                            <div class="ticket-action-title">{t!(i18n, ticket.hero.ready)}</div>
                            <div class="ticket-action-desc">
                                {t!(i18n, ticket.ready_desc)}
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
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

        // Community links (social only — guide links are rendered above
        // in the Access & Logistics card)
        {crate::pages::ticket::community_links::community_links_section(social_links.clone(), crate::pages::ticket::community_links::CommunityLinksVariant::Ticket)}

        // 5b. Event series navigation (Plan 013) — "Part of {Series}" + prev/next.
        // Renders nothing when the event has no campaign, so it's safe to mount
        // unconditionally; the component self-hides on 404/error/loading.
        <super::series_nav::SeriesNav event_id=event_id.clone() />

        // 6. Footer
        <div class="ticket-footer">
            <div class="ticket-nav">
                <A href="/">{t!(i18n, ticket.nav_home)}</A>
                <A href="/profile">{t!(i18n, ticket.nav_profile)}</A>
            </div>
            {if is_checked_in {
                view! {
                    <p class="ticket-footer-hint">
                        {t!(i18n, ticket.footer_checked_in)}
                    </p>
                }.into_any()
            } else if !is_approved {
                view! {
                    <p class="ticket-footer-hint">
                        {t!(i18n, ticket.footer_pending)}
                    </p>
                }.into_any()
            } else {
                view! {
                    <p class="ticket-footer-hint">
                        {t!(i18n, ticket.footer_ready)}
                    </p>
                }.into_any()
            }}
        </div>
    }
}
