//! Online attendee view — timeline, NFT claim, video section.

use leptos::prelude::*;
use leptos_router::components::A;
use wasm_bindgen::prelude::*;

use super::action_cards::*;
use super::calendar_links::CalendarLinks;
use super::event_context::EventContext;
use super::nft_badge::NftClaimedBadge;
use super::timeline::{Timeline, TimelineStep};
use super::video_section::VideoSection;
use super::view_data::TicketViewData;
use crate::i18n::{t, use_i18n};
use crate::icons::{Icon, IconName};
use crate::utils;

#[wasm_bindgen(module = "/js/clipboard.js")]
extern "C" {
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;
}

/// Online attendee view component.
#[component]
pub fn OnlineView(
    /// Pre-computed view data
    view_data: TicketViewData,
) -> impl IntoView {
    let TicketViewData {
        name,
        masked_email,
        nft_image_url,
        event_tagline,
        event_location,
        event_location_map_url,
        event_link,
        deposit_enabled,
        deposit_info,
        deadline_expired,
        in_person_available,
        deposit_href,
        event_end_ms,
        is_checked_in,
        has_claim,
        claim_href,
        claimed,
        claimed_asset_id,
        orb_link,
        has_video,
        video_url,
        quiz_enabled,
        community_links,
        calendar_subscribe_url,
        ticket_note,
        event_start_ms: _,
        event_name,
        event_id,
        ..
    } = view_data;

    let i18n = use_i18n();

    // Live countdown: the clock is a signal updated every 60s; the words are
    // rendered from it in the view, so they follow a language switch.
    let (now_ms, set_now_ms) = signal(js_sys::Date::now() as i64);
    let (event_ended, set_event_ended) =
        signal(event_end_ms > 0 && js_sys::Date::now() as i64 >= event_end_ms);

    // "2d 3h remaining" / "3h 20m remaining", or `None` once the event is over.
    let countdown = move || -> Option<AnyView> {
        let now = now_ms.get();
        if event_end_ms <= 0 || now >= event_end_ms {
            return None;
        }
        let diff_ms = event_end_ms - now;
        let days = diff_ms / (1000 * 60 * 60 * 24);
        let hours = (diff_ms % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60);
        let mins = (diff_ms % (1000 * 60 * 60)) / (1000 * 60);
        Some(match days > 0 {
            true => t!(i18n, ticket.timeline.remaining_days, days, hours).into_any(),
            false => t!(i18n, ticket.timeline.remaining_hours, hours, mins).into_any(),
        })
    };

    // Start a 60s interval to refresh countdown
    Effect::new(move |_| {
        let cb = Closure::<dyn Fn()>::new(move || {
            let now = js_sys::Date::now() as i64;
            let ended = event_end_ms > 0 && now >= event_end_ms;
            if event_ended.get_untracked() != ended {
                set_event_ended.set(ended);
            }
            set_now_ms.set(now);
        });
        let interval_id = web_sys::window()
            .unwrap()
            .set_interval_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                60_000i32,
            )
            .unwrap();
        cb.forget();
        on_cleanup(move || {
            let _ = web_sys::window().map(|w| {
                w.clear_interval_with_handle(interval_id);
            });
        });
    });

    // Build timeline quest link — only show when quest is actually configured
    // If we have a claim token, link to /claim/{token} which handles quiz gate.
    // If no claim token (e.g. D1 missing claim_token), link to /adventure directly.
    let quest_link: Option<(String, ViewFn)> = if !is_checked_in && quiz_enabled {
        if has_claim {
            Some((
                claim_href.clone(),
                ViewFn::from(move || t!(i18n, ticket.timeline.go_to_quest)),
            ))
        } else {
            let adventure_href = format!("/adventure?event_id={}", event_id);
            Some((
                adventure_href,
                ViewFn::from(move || t!(i18n, ticket.timeline.start_adventure)),
            ))
        }
    } else {
        None
    };

    view! {
        // 1. Hero banner
        <super::hero::TicketHero
            variant="ticket-hero--online"
            icon=IconName::Globe
            title=move || t!(i18n, ticket.hero.online_title)
            badge=ViewFn::from(move || t!(i18n, ticket.hero.online_badge))
        />

        // 2. Main card
        <div class="ticket-main-card">
            // Event context
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

            // Attendee info
            <div class="ticket-info">
                <div class="ticket-info-row">
                    <span class="ticket-info-label">{t!(i18n, ticket.info.name)}</span>
                    <span class="ticket-info-value">
                        {utils::escape_html(&name)}
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
            </div>

            // Deposit notice slot — exactly one for online
            {if deposit_enabled && deposit_info.is_none() {
                if deadline_expired && in_person_available.unwrap_or(false) {
                    view! {
                        <ReclaimActionCard reclaim_href=deposit_href.clone() />
                    }.into_any()
                } else if deadline_expired {
                    view! {
                        <MovedOnlineCard />
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            } else {
                view! { <div></div> }.into_any()
            }}
        </div>

        // 3. Timeline — "What's Next?"
        // Rebuilt when the event ends; the countdown inside ticks on its own.
        {move || {
            let ended = event_ended.get();
            let quest_link = quest_link.clone();
            view! {
                <Timeline steps=vec![
                    TimelineStep {
                        done: true,
                        number: 1,
                        title: ViewFn::from(move || t!(i18n, ticket.timeline.register)),
                        desc: ViewFn::from(move || t!(i18n, ticket.timeline.register_done)),
                        link: None,
                    },
                    TimelineStep {
                        done: ended,
                        number: 2,
                        title: ViewFn::from(move || match ended {
                            true => t!(i18n, ticket.timeline.event_ended).into_any(),
                            false => t!(i18n, ticket.timeline.wait_for_event).into_any(),
                        }),
                        desc: ViewFn::from(move || match ended {
                            true => t!(i18n, ticket.timeline.event_ended_desc).into_any(),
                            false => (move || countdown().unwrap_or_else(|| {
                                t!(i18n, ticket.timeline.claims_open_after).into_any()
                            })).into_any(),
                        }),
                        link: None,
                    },
                    TimelineStep {
                        done: is_checked_in,
                        number: 3,
                        title: ViewFn::from(move || {
                            if is_checked_in {
                                t!(i18n, ticket.timeline.quest_completed).into_any()
                            } else if quiz_enabled {
                                t!(i18n, ticket.timeline.complete_quest).into_any()
                            } else {
                                t!(i18n, ticket.timeline.virtual_check_in).into_any()
                            }
                        }),
                        desc: ViewFn::from(move || {
                            if is_checked_in {
                                t!(i18n, ticket.timeline.virtual_check_in_done).into_any()
                            } else if quiz_enabled {
                                t!(i18n, ticket.timeline.pass_quiz).into_any()
                            } else {
                                t!(i18n, ticket.timeline.claim_opens_after).into_any()
                            }
                        }),
                        link: quest_link,
                    },
                    TimelineStep {
                        done: claimed,
                        number: 4,
                        title: ViewFn::from(move || match claimed {
                            true => t!(i18n, ticket.timeline.badge_claimed).into_any(),
                            false => t!(i18n, ticket.timeline.claim_badge).into_any(),
                        }),
                        desc: ViewFn::from(move || match claimed {
                            true => t!(i18n, ticket.timeline.badge_minted).into_any(),
                            false => t!(i18n, ticket.timeline.mint_badge).into_any(),
                        }),
                        link: None,
                    },
                ] />
            }
        }}

        // 4. NFT section
        {if claimed {
            let asset_id = claimed_asset_id.clone().unwrap_or_default();
            view! {
                <NftClaimedBadge
                    asset_id=asset_id
                    orb_link=orb_link.clone().unwrap_or_default()
                    on_copy=Box::new(copy_to_clipboard_js)
                />
            }.into_any()
        } else {
            let ended = event_ended.get();
            let available = has_claim && ended;
            if available {
                view! {
                    <ClaimActionCard claim_href=claim_href.clone() />
                }.into_any()
            } else if has_claim && !ended {
                view! {
                    <div class="ticket-action-card ticket-action-card--pending">
                        <div class="ticket-action-icon">
                            <Icon icon=IconName::Clock class="icon-sm" />
                        </div>
                        <div>
                            <div class="ticket-action-title">{t!(i18n, ticket.online.claim_soon)}</div>
                            <div class="ticket-action-desc">
                                {t!(i18n, ticket.online.claim_soon_desc)}
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else if is_checked_in {
                // Quest completed but no claim link yet (missing claim_token or event not ended)
                if ended {
                    view! {
                        <div class="ticket-action-card ticket-action-card--pending">
                            <div class="ticket-action-icon">
                                <Icon icon=IconName::Gift class="icon-sm" />
                            </div>
                            <div>
                                <div class="ticket-action-title">{t!(i18n, ticket.online.claim_pending)}</div>
                                <div class="ticket-action-desc">
                                    {t!(i18n, ticket.online.claim_pending_desc)}
                                </div>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="ticket-action-card ticket-action-card--pending">
                            <div class="ticket-action-icon">
                                <Icon icon=IconName::Clock class="icon-sm" />
                            </div>
                            <div>
                                <div class="ticket-action-title">{t!(i18n, ticket.online.claim_soon)}</div>
                                <div class="ticket-action-desc">
                                    {t!(i18n, ticket.online.quest_done_claim_soon)}
                                </div>
                            </div>
                        </div>
                    }.into_any()
                }
            } else {
                view! { <div></div> }.into_any()
            }
        }}

        // 5. Video section
        {if has_video {
            view! {
                <VideoSection video_url=video_url.clone() variant="card".to_string() />
            }.into_any()
        } else {
            view! { <div></div> }.into_any()
        }}

        // Community links
        {super::announcement::announcement_section(ticket_note.clone())}
        {crate::pages::ticket::community_links::community_links_section(community_links.clone(), crate::pages::ticket::community_links::CommunityLinksVariant::Ticket)}

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
        </div>
    }
}
