//! `<EventFormComponent>`: builds the shared state and composes the sections.

use leptos::prelude::*;

use super::actions::FormActions;
use super::basic::{BasicSection, ScheduleSection};
use super::community::CommunitySection;
use super::ctx::FormCtx;
use super::deposit::DepositSection;
use super::nft::NftSection;
use super::people::{AnnouncementsSection, PeopleSection};
use super::poster::PosterSection;
use super::settings::{CapacitySection, SettingsSection};
use super::sheets::SheetsSection;
use super::sponsors::SponsorsSection;
use super::types::{EventForm, OnDone};
use crate::{api, components};

/// Full event form component shared by Create and Edit modes.
///
/// Handles all form fields, validation, save logic (create/update + escrow init),
/// and wallet connection for combined Create + Escrow Init flow.
#[component]
pub fn EventFormComponent(
    #[prop(name = "set_toast")] set_toast: WriteSignal<Option<components::ToastMessage>>,
    #[prop(name = "form")] form: ReadSignal<EventForm>,
    #[prop(name = "set_form")] set_form: WriteSignal<EventForm>,
    #[prop(name = "editing_id")] editing_id: ReadSignal<Option<String>>,
    #[prop(name = "is_create")] is_create: bool,
    #[prop(name = "events")] events: ReadSignal<Vec<api::EventMeta>>,
    #[prop(name = "on_done")] on_done: OnDone,
) -> impl IntoView {
    let (cl_links, set_cl_links) = signal(form.get().community_links.clone());
    let sponsors = RwSignal::new(form.get().sponsors.clone());
    let (slug_taken, set_slug_taken) = signal(false);
    let (saving, set_saving) = signal(false);
    let (poster_busy, set_poster_busy) = signal(false);
    let (create_wallet_name, set_create_wallet_name) = signal(String::new());
    let (create_wallet_pk, set_create_wallet_pk) = signal(String::new());
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());
    let ctx = FormCtx {
        form,
        set_form,
        set_toast,
        editing_id,
        is_create,
        events,
        saving,
        set_saving,
        poster_busy,
        set_poster_busy,
        slug_taken,
        set_slug_taken,
        cl_links,
        set_cl_links,
        sponsors,
        create_wallet_name,
        set_create_wallet_name,
        create_wallet_pk,
        set_create_wallet_pk,
        detected_wallets,
    };

    // Detect installed wallets on mount (poll for late-injecting extensions)
    {
        let set_dw = set_detected_wallets;
        leptos::task::spawn_local(async move {
            let mut wallets = crate::pages::escrow_init::get_detected_wallets_js();
            if wallets.is_empty() {
                for _ in 0..10 {
                    gloo_timers::future::TimeoutFuture::new(300).await;
                    wallets = crate::pages::escrow_init::get_detected_wallets_js();
                    if !wallets.is_empty() {
                        break;
                    }
                }
            }
            log::info!("[event-form] detected wallets: {:?}", wallets);
            set_dw.set(wallets);
        });
    }

    view! {
        <div class="card">
            <h2 class="admin-section-heading">{if is_create { "Create Event" } else { "Edit Event" }}</h2>
            <BasicSection ctx=ctx />
            <ScheduleSection ctx=ctx />
            <SheetsSection ctx=ctx />
            <PosterSection ctx=ctx />
            <NftSection ctx=ctx />
            <SettingsSection ctx=ctx />
            <Show when=move || {
                let fmt = form.get().event_format;
                fmt == api::EventFormat::InPerson || fmt == api::EventFormat::Hybrid
            } fallback=|| view! { <div></div> }>
                <CapacitySection ctx=ctx />
            </Show>
            <Show when=move || form.get().deposit_enabled fallback=|| view! { <div></div> }>
                <DepositSection ctx=ctx />
            </Show>
            <PeopleSection ctx=ctx />
            <AnnouncementsSection ctx=ctx />
            <CommunitySection ctx=ctx />
            <SponsorsSection ctx=ctx />
            <FormActions ctx=ctx on_done=on_done />
        </div>
    }
}
