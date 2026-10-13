//! The page of an event that is over (.issues/165): its details and the
//! archive. Someone already registered goes on to their ticket (NFT claim,
//! recording); everyone else registers to watch the recording.

use super::attribution::{organizer_line, sponsor_row};
use super::details_card::details_card;
use super::event_hero::event_hero;
use super::types::*;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

/// Completed events deliberately use a different action surface from live
/// events. Retrospective enrollment is a learning/community lead: it must not
/// expose reservation, deposit, check-in, quiz, or NFT-claim controls.
pub(super) fn completed_event_gateway(
    data: PublicEventData,
    countdown: ReadSignal<String>,
    event_completed: ReadSignal<bool>,
    reg_lookup: ReadSignal<RegistrationLookup>,
) -> AnyView {
    let name = data.name.clone();
    let tagline = data.tagline.clone();
    let description = data.description.clone();
    let slug = data.slug.clone();
    let archive_url = data.archive_url.clone();
    let enrollment_open = data.post_event_registration_accepting;
    let event_id = data.id.clone();
    // With a recording, registering is how people watch it (owner 2026-10-13).
    let to_watch = enrollment_open && data.has_recording;
    let poster_url = data.poster_url.clone();
    let nft_image_url = data.nft_image_url.clone();
    let community_links = data.community_links.clone();

    view! {
        {event_hero(&poster_url, &nft_image_url, &data.slug)}

        <div class="pe-name-block">
            <h1 class="pe-name">{name}</h1>
            {organizer_line(&data.organizer_name)}
            {if !tagline.is_empty() {
                view! { <p class="pe-tagline">{tagline}</p> }.into_any()
            } else {
                ().into_any()
            }}
        </div>

        {details_card(&data, countdown, event_completed)}

        <div class="pe-card">
            <h2 class="pe-section-title">
                <Icon icon=IconName::Party class="icon-md" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.ended_title))}
            </h2>
            // Already registered: straight to the recording, no form
            // (.issues/165). Everyone else: the archive and "register to watch".
            {move || match reg_lookup.get() {
                RegistrationLookup::Registered(reg) => registered_ended(reg, &slug, &event_id),
                _ => guest_ended(to_watch, enrollment_open, &slug, &archive_url),
            }}
        </div>

        {if !description.is_empty() {
            view! {
                <div class="pe-card">
                    <h2 class="pe-section-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.about_title))}</h2>
                    <p class="pe-description">{description}</p>
                </div>
            }.into_any()
        } else {
            ().into_any()
        }}

        {crate::pages::ticket::community_links::community_links_section(
            community_links,
            crate::pages::ticket::community_links::CommunityLinksVariant::PublicEvent,
        )}

        {sponsor_row(&data.sponsors)}
    }
    .into_any()
}

/// Someone already registered follows their normal flow (owner 2026-10-13):
/// on to their ticket, which carries the NFT claim once they checked in and
/// the recording when there is one. A deposit can no longer be paid once the
/// event is over, so that step goes to the ticket too.
fn registered_ended(mut reg: MyRegistrationData, slug: &str, event_id: &str) -> AnyView {
    if reg.next_step._step_type == "deposit" {
        reg.next_step._step_type = "ticket".to_string();
        reg.next_step.url = super::registered_state::ticket_url(&reg.attendee_id, event_id);
    }
    let email = reg.email.clone();
    super::registered_state::registered_state(&reg, &email, slug, event_id)
}

/// The ended card for everyone else: the archive, and registering to watch
/// the recording (or to join the community when there is none).
fn guest_ended(to_watch: bool, enrollment_open: bool, slug: &str, archive_url: &str) -> AnyView {
    let line = match to_watch {
        true => crate::locale::tr(|l| crate::i18n::td_string!(l, event.ended_body_recording)),
        false => crate::locale::tr(|l| crate::i18n::td_string!(l, event.ended_body)),
    };
    let archive = match archive_url.is_empty() {
        false => archive_link(archive_url),
        true => view! {
            <span class="pe-detail-secondary">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.archive_soon))}</span>
        }
        .into_any(),
    };
    let register = enrollment_open.then(|| {
        let (href, label) = match to_watch {
            true => (
                format!("/events/{slug}/post-event-register?watch=1"),
                crate::locale::tr(|l| crate::i18n::td_string!(l, event.watch_recording)),
            ),
            false => (
                format!("/events/{slug}/post-event-register"),
                crate::locale::tr(|l| crate::i18n::td_string!(l, event.join_community)),
            ),
        };
        view! { <a href=href class="btn btn-primary btn-sm">{label}</a> }
    });
    view! {
        <p class="pe-detail-secondary pe-mb-075">{line}</p>
        <div class="pe-btn-row-center">
            {archive}
            {register}
        </div>
    }
    .into_any()
}

/// The canonical Genesis archive link (the Worker only sends a trusted one).
fn archive_link(archive_url: &str) -> AnyView {
    let href = archive_url.to_string();
    view! {
        <a href=href target="_blank" rel="noopener noreferrer" class="btn btn-outline btn-sm">
            <Icon icon=IconName::Link class="icon-sm" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.view_archive))}
        </a>
    }
    .into_any()
}
