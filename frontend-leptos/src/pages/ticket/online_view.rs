//! Online attendee view — timeline, NFT claim, video section.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use super::action_cards::*;
use super::event_context::EventContext;
use super::nft_badge::NftClaimedBadge;
use super::timeline::{Timeline, TimelineStep};
use super::video_section::VideoSection;
use super::view_data::TicketViewData;
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
        ..
    } = view_data;

    // Live countdown: reactive signal updated every 60s
    let (countdown_text, set_countdown_text) = signal(String::new());
    let (event_ended, set_event_ended) =
        signal(event_end_ms > 0 && js_sys::Date::now() as i64 >= event_end_ms);

    let fmt_remaining = move |now_ms: i64| -> String {
        if event_end_ms <= 0 || now_ms >= event_end_ms {
            return String::new();
        }
        let diff_ms = event_end_ms - now_ms;
        let days = diff_ms / (1000 * 60 * 60 * 24);
        let hours = (diff_ms % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60);
        if days > 0 {
            format!("{days}d {hours}h remaining")
        } else {
            let mins = (diff_ms % (1000 * 60 * 60)) / (1000 * 60);
            format!("{hours}h {mins}m remaining")
        }
    };

    // Initial value
    set_countdown_text.set(fmt_remaining(js_sys::Date::now() as i64));

    // Start a 60s interval to refresh countdown
    Effect::new(move |_| {
        let cb = Closure::<dyn Fn()>::new(move || {
            let now_ms = js_sys::Date::now() as i64;
            let ended = event_end_ms > 0 && now_ms >= event_end_ms;
            set_event_ended.set(ended);
            set_countdown_text.set(fmt_remaining(now_ms));
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
    let quest_link = if !is_checked_in && has_claim && quiz_enabled {
        Some((claim_href.clone(), "→ Go to Quest".to_string()))
    } else {
        None
    };

    view! {
        // 1. Hero banner
        <super::hero::TicketHero
            variant="ticket-hero--online"
            icon=IconName::Globe
            title="Online Registration"
            badge="Online Track".to_string()
        />

        // 2. Main card
        <div class="ticket-main-card">
            // Event context
            <EventContext
                nft_image_url=nft_image_url.clone()
                tagline=event_tagline.clone()
                location=event_location.clone()
                event_link=event_link.clone()
            />

            // Attendee info
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
        <Timeline steps=vec![
            TimelineStep {
                done: true,
                number: 1,
                title: "Register".into(),
                desc: "You're all signed up!".into(),
                link: None,
            },
            TimelineStep {
                done: event_ended.get(),
                number: 2,
                title: if event_ended.get() { "Event Ended" } else { "Wait for Event" }.into(),
                desc: if event_ended.get() {
                    "The event has ended — you can proceed to claim.".to_string()
                } else {
                    let ct = countdown_text.get();
                    if !ct.is_empty() { ct } else { "Claims open after the event ends.".to_string() }
                },
                link: None,
            },
            TimelineStep {
                done: is_checked_in,
                number: 3,
                title: if is_checked_in {
                    "Quest Completed"
                } else if quiz_enabled {
                    "Complete Quest"
                } else {
                    "Virtual Check-in"
                }.into(),
                desc: if is_checked_in {
                    "Virtual check-in complete!".into()
                } else if quiz_enabled {
                    "Pass the quiz or adventure to virtually check in.".into()
                } else {
                    "Claim opens after the event ends.".into()
                },
                link: quest_link,
            },
            TimelineStep {
                done: claimed,
                number: 4,
                title: if claimed { "Badge Claimed!" } else { "Claim Your Badge" }.into(),
                desc: if claimed {
                    "Your compressed NFT attendance proof has been minted.".into()
                } else {
                    "Mint your compressed NFT attendance proof.".into()
                },
                link: None,
            },
        ] />

        // 4. NFT section
        {if claimed {
            let asset_id = claimed_asset_id.clone().unwrap_or_default();
            view! {
                <NftClaimedBadge
                    asset_id=asset_id
                    orb_link=orb_link.clone().unwrap_or_default()
                    on_copy=Box::new(|text| copy_to_clipboard_js(text))
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
                            <div class="ticket-action-title">"Claim Available Soon"</div>
                            <div class="ticket-action-desc">
                                "Claim link will be available after the event ends."
                            </div>
                        </div>
                    </div>
                }.into_any()
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
        {crate::pages::ticket::community_links::community_links_section(community_links.clone(), crate::pages::ticket::community_links::CommunityLinksVariant::Ticket)}

        // 6. Footer
        <div class="ticket-footer">
            <div class="ticket-nav">
                <a href="/">"← Home"</a>
            </div>
        </div>
    }
}
