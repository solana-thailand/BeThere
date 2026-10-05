use super::attribution::{organizer_line, sponsor_row};
use super::deposit_section::deposit_section;
use super::details_card::details_card;
use super::event_hero::event_hero;
use super::registered_state::registered_state;
use super::registration_form::registration_form;
use super::share_button::share_button;
use super::types::*;
use crate::i18n::{t, t_string, td_string, use_i18n};
use crate::icons::{Icon, IconName};
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params;

#[allow(non_snake_case)]
pub fn PublicEvent() -> impl IntoView {
    let i18n = use_i18n();
    let params = use_params::<PublicEventParams>();

    // Reactive state
    let (state, set_state) = signal(PublicEventState::Loading);
    let (countdown, set_countdown) = signal(String::new());
    // Start time the countdown ticks toward. The interval lives in an Effect
    // keyed on this signal, because an `on_cleanup` inside the fetch task runs
    // after `.await`, where there is no owner, so it never ran and each visit
    // left a 1 s interval behind.
    let (countdown_start_ms, set_countdown_start_ms) = signal(None::<i64>);
    Effect::new(move |_| {
        let Some(start_ms) = countdown_start_ms.get() else {
            return;
        };
        if let Ok(handle) = set_interval_with_handle(
            move || {
                let remaining = start_ms - js_sys::Date::now() as i64;
                if remaining <= 0 {
                    set_countdown.set(String::new());
                    set_countdown_start_ms.set(None);
                } else {
                    // Re-rendered every second, so a language switch shows within 1 s.
                    set_countdown.set(format_countdown(remaining, i18n.get_locale_untracked()));
                }
            },
            std::time::Duration::from_secs(1),
        ) {
            on_cleanup(move || handle.clear());
        }
    });
    let (event_completed, set_event_completed) = signal(false);
    let (event_name, set_event_name) = signal(String::new());
    let (share_copied, set_share_copied) = signal(false);

    // Auth state
    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    let (reg_lookup, set_reg_lookup) = signal(RegistrationLookup::Pending);
    // Wallet-only session (Plan 017): drives the "enter your email" input on
    // the reservation form.
    let (wallet_only, set_wallet_only) = signal(false);
    // Rolling deposit credit (THB whole baht) for the signed-in attendee — shown
    // on the reserve card so returning attendees know their credit will apply.
    let (credit_thb, set_credit_thb) = signal(0u64);

    // Get slug from params
    let slug_val = match params.get() {
        Ok(p) => p.slug.unwrap_or_default(),
        Err(_) => String::new(),
    };

    // Fetch event data on mount
    let slug_for_event = slug_val.clone();
    Effect::new(move |_| {
        let slug = match params.get() {
            Ok(p) => p.slug.unwrap_or_default(),
            Err(e) => {
                log::error!("[public_event] params error: {e:?}");
                set_state.set(PublicEventState::NotFound);
                return;
            }
        };

        if slug.is_empty() {
            log::error!("[public_event] slug is empty");
            set_state.set(PublicEventState::NotFound);
            return;
        }

        // A new slug must not keep ticking toward the previous event's start.
        set_countdown_start_ms.set(None);
        set_countdown.set(String::new());
        log::info!("[public_event] fetching slug: {slug}");
        let slug_clone = slug.clone();
        // Client-side error text, in the language current at load time.
        let locale = i18n.get_locale_untracked();
        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or_else(|_| "http://localhost:8787".to_string());
            let url = format!("{origin}/api/public/event/{slug_clone}");
            log::info!("[public_event] fetch URL: {url}");

            match crate::api::fetch::get(&url, &[]).await {
                Ok(resp) => {
                    log::info!("[public_event] response status: {}", resp.status());
                    if resp.status() == 404 {
                        set_state.set(PublicEventState::NotFound);
                        return;
                    }

                    match crate::api::fetch::response_text(&resp).await {
                        Ok(body) => {
                            log::info!("[public_event] body length: {}", body.len());
                            match serde_json::from_str::<PublicEventResponse>(&body) {
                                Ok(api_resp) => {
                                    log::info!(
                                        "[public_event] parsed OK, success={}",
                                        api_resp.success
                                    );
                                    if api_resp.success {
                                        if let Some(data) = api_resp.data {
                                            let is_completed = data.status == "completed"
                                                || data.status == "Completed";
                                            let start_ms = data.event_start_ms;
                                            let name = data.name.clone();
                                            set_event_name.set(name);
                                            set_event_completed.set(is_completed);
                                            set_state.set(PublicEventState::Loaded(data));

                                            // Start countdown if event is in the future
                                            let now_ms = js_sys::Date::now() as i64;
                                            if !is_completed && start_ms > now_ms {
                                                set_countdown.set(format_countdown(
                                                    start_ms - now_ms,
                                                    i18n.get_locale_untracked(),
                                                ));
                                                set_countdown_start_ms.set(Some(start_ms));
                                            }
                                        } else {
                                            set_state.set(PublicEventState::Error(
                                                td_string!(locale, event.err_no_event_data)
                                                    .to_string(),
                                            ));
                                        }
                                    } else {
                                        set_state.set(PublicEventState::Error(
                                            api_resp.error.unwrap_or_else(|| {
                                                td_string!(locale, event.err_unknown).to_string()
                                            }),
                                        ));
                                    }
                                }
                                Err(e) => {
                                    log::error!("[public_event] JSON parse error: {e}");
                                    let msg = td_string!(locale, event.err_parse_response);
                                    set_state.set(PublicEventState::Error(format!("{msg}: {e}")));
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("[public_event] body read error: {e}");
                            set_state.set(PublicEventState::Error(
                                td_string!(locale, event.err_read_response).to_string(),
                            ));
                        }
                    }
                }
                Err(e) => {
                    log::error!("[public_event] fetch error: {e}");
                    let msg = td_string!(locale, event.err_fetch_event);
                    set_state.set(PublicEventState::Error(format!("{msg}: {e}")));
                }
            }
        });
    });

    // Check auth status on mount
    leptos::task::spawn_local(async move {
        let window = web_sys::window().expect("no window");
        let origin = window
            .location()
            .origin()
            .unwrap_or_else(|_| "http://localhost:8787".to_string());
        let url = format!("{origin}/api/auth/me");

        match crate::api::fetch::get(&url, &[]).await {
            Ok(resp) => {
                if resp.status() == 200 {
                    if let Ok(body) = crate::api::fetch::response_text(&resp).await {
                        if let Ok(api_resp) = serde_json::from_str::<serde_json::Value>(&body) {
                            let email = api_resp
                                .get("data")
                                .and_then(|d| d.get("email"))
                                .and_then(|e| e.as_str())
                                .unwrap_or("")
                                .to_string();
                            if !email.is_empty() {
                                log::info!("[public_event] user signed in: {email}");
                                set_wallet_only.set(
                                    api_resp
                                        .get("data")
                                        .and_then(|d| d.get("wallet_only"))
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false),
                                );
                                set_auth_state.set(AuthState::SignedIn(email));
                            } else {
                                set_auth_state.set(AuthState::NotSignedIn);
                            }
                        } else {
                            set_auth_state.set(AuthState::NotSignedIn);
                        }
                    } else {
                        set_auth_state.set(AuthState::NotSignedIn);
                    }
                } else {
                    log::info!(
                        "[public_event] auth/me returned {} — not signed in",
                        resp.status()
                    );
                    set_auth_state.set(AuthState::NotSignedIn);
                }
            }
            Err(e) => {
                log::warn!("[public_event] auth/me fetch error: {e}");
                set_auth_state.set(AuthState::NotSignedIn);
            }
        }
    });

    // When auth becomes SignedIn, check if already registered
    let slug_for_reg_lookup = slug_val.clone();
    Effect::new(move |_| {
        let auth = auth_state.get();
        match auth {
            AuthState::SignedIn(ref email) => {
                let email_clone = email.clone();
                let slug = slug_for_reg_lookup.clone();
                let current_lookup = reg_lookup.get();
                if matches!(current_lookup, RegistrationLookup::Pending) {
                    leptos::task::spawn_local(async move {
                        let window = web_sys::window().expect("no window");
                        let origin = window
                            .location()
                            .origin()
                            .unwrap_or_else(|_| "http://localhost:8787".to_string());
                        let url = format!("{origin}/api/my-registration/{slug}");

                        match crate::api::fetch::get(&url, &[]).await {
                            Ok(resp) => {
                                if resp.status() == 404 {
                                    log::info!(
                                        "[public_event] {email_clone} not registered for {slug}"
                                    );
                                    set_reg_lookup.set(RegistrationLookup::NotRegistered);
                                } else if resp.status() == 200 {
                                    if let Ok(body) = crate::api::fetch::response_text(&resp).await
                                    {
                                        match serde_json::from_str::<MyRegistrationResponse>(&body)
                                        {
                                            Ok(api_resp) => {
                                                if let Some(data) = api_resp.data {
                                                    log::info!(
                                                        "[public_event] {email_clone} already registered for {slug}"
                                                    );
                                                    set_reg_lookup
                                                        .set(RegistrationLookup::Registered(data));
                                                } else {
                                                    set_reg_lookup
                                                        .set(RegistrationLookup::NotRegistered);
                                                }
                                            }
                                            Err(e) => {
                                                log::warn!(
                                                    "[public_event] my-registration parse error: {e}"
                                                );
                                                set_reg_lookup
                                                    .set(RegistrationLookup::NotRegistered);
                                            }
                                        }
                                    } else {
                                        set_reg_lookup.set(RegistrationLookup::NotRegistered);
                                    }
                                } else {
                                    log::warn!(
                                        "[public_event] my-registration returned {}",
                                        resp.status()
                                    );
                                    set_reg_lookup.set(RegistrationLookup::Error(format!(
                                        "Status {}",
                                        resp.status()
                                    )));
                                }
                            }
                            Err(e) => {
                                log::warn!("[public_event] my-registration fetch error: {e}");
                                set_reg_lookup
                                    .set(RegistrationLookup::Error(format!("Fetch error: {e}")));
                            }
                        }
                    });
                }
            }
            AuthState::Checking | AuthState::NotSignedIn => {}
        }
    });

    // Fetch rolling deposit credit once signed in (best-effort, reassurance only).
    Effect::new(move |_| {
        if matches!(auth_state.get(), AuthState::SignedIn(_)) {
            leptos::task::spawn_local(async move {
                if let Ok(resp) = crate::api::fetch::get("/api/deposit/credit-balance", &[]).await
                    && resp.status() == 200
                    && let Ok(v) =
                        crate::api::fetch::response_json::<serde_json::Value>(&resp).await
                {
                    let data = v.get("data").unwrap_or(&v);
                    let t = data.get("credit_thb").and_then(|x| x.as_u64()).unwrap_or(0);
                    set_credit_thb.set(t);
                }
            });
        }
    });

    // Dynamic title
    let title_text = move || {
        let name = event_name.get();
        if name.is_empty() {
            t_string!(i18n, event.page_title).to_string()
        } else {
            format!("{name} — BeThere")
        }
    };

    view! {
        <Title text=title_text />
        {move || {
            // OG meta tags — update when event name or data changes
            let name = event_name.get();
            if !name.is_empty() {
                let slug = slug_val.clone();
                view! {
                    <Meta name="og:title" content=format!("{name} — BeThere") />
                    <Meta property="og:type" content="website" />
                    <Meta property="og:url" content=format!("https://bethere.solana-thailand.workers.dev/e/{slug}") />
                }.into_any()
            } else {
                ().into_any()
            }
        }}
        <div class="center-page pe-bg-anim">
            // Latent-space animated background layer (nebula mesh, color blobs,
            // aurora sweep, twinkling starfield). Purely decorative — aria-hidden,
            // pointer-events disabled in CSS.
            <div class="pe-bg-layer" aria-hidden="true">
                <div class="pe-bg-nebula"></div>
                <div class="pe-bg-aurora"></div>
                <div class="pe-bg-orb pe-bg-orb-1"></div>
                <div class="pe-bg-orb pe-bg-orb-2"></div>
                <div class="pe-bg-orb pe-bg-orb-3"></div>
                <div class="pe-bg-orb pe-bg-orb-4"></div>
                <div class="pe-bg-stars"></div>
            </div>
            <div class="container layout-col-center pe-container-nogap">

                // Back link
                <div class="pe-back-wrap">
                    <a href="/" class="pe-back-link">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.back_home))}
                    </a>
                </div>

                // Page state
                {move || {
                    let s = state.get();
                    match s {
                        PublicEventState::Loading => {
                            // Skeleton mirrors the real layout (hero → name → CTA →
                            // cards) so the page reads as "loading content" rather
                            // than a blank/spinner — better perceived speed on
                            // venue wifi.
                            view! {
                                <div class="pe-skeleton" aria-busy="true" aria-label=crate::locale::tr(|l| crate::i18n::td_string!(l, event.loading_aria))>
                                    <div class="pe-skel pe-skel-hero"></div>
                                    <div class="pe-skel pe-skel-title"></div>
                                    <div class="pe-skel pe-skel-sub"></div>
                                    <div class="pe-skel pe-skel-cta"></div>
                                    <div class="pe-skel pe-skel-card"></div>
                                    <div class="pe-skel pe-skel-card"></div>
                                </div>
                            }.into_any()
                        }
                        PublicEventState::NotFound => {
                            view! {
                                <div class="pe-loading">
                                    <div class="pe-icon-mb"><Icon icon=IconName::Search class="icon-2xl" /></div>
                                    <h1 class="pe-error-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.not_found_title))}</h1>
                                    <p class="pe-detail-secondary pe-msg-mb-lg">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.not_found_body))}
                                    </p>
                                    <a href="/" class="btn btn-primary">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.go_home))}</a>
                                </div>
                            }.into_any()
                        }
                        PublicEventState::Error(msg) => {
                            let msg_display = msg.clone();
                            view! {
                                <div class="pe-loading">
                                    <div class="pe-icon-mb"><Icon icon=IconName::Warning class="icon-md icon-danger" /></div>
                                    <h1 class="pe-error-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.error_title))}</h1>
                                    <p class="pe-detail-secondary pe-msg-mb-lg">{msg_display}</p>
                                    <div class="pe-flex-row-gap">
                                        <button
                                            class="btn btn-primary"
                                            on:click=move |_| {
                                                // Retry by re-triggering the fetch
                                                set_state.set(PublicEventState::Loading);
                                                // The Effect will re-run because state changed
                                            }
                                        >
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, event.try_again))}
                                        </button>
                                        <a href="/" class="btn btn-outline">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.go_home))}</a>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        PublicEventState::Loaded(data) => {
                            render_loaded_event(
                                data,
                                countdown,
                                event_completed,
                                auth_state,
                                reg_lookup,
                                slug_for_event.clone(),
                                share_copied,
                                set_share_copied,
                                wallet_only,
                                credit_thb,
                            )
                        }
                    }
                }}

                // Footer
                <div class="pe-footer">
                    <p>
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.powered_by))}" "
                        <a href="/" class="pe-footer-link">"BeThere"</a>
                    </p>
                </div>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Render loaded event — orchestrates all sub-components
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)] // plain render helper; signature refactor is out of scope
fn render_loaded_event(
    data: PublicEventData,
    countdown: ReadSignal<String>,
    event_completed: ReadSignal<bool>,
    auth_state: ReadSignal<AuthState>,
    reg_lookup: ReadSignal<RegistrationLookup>,
    current_slug: String,
    share_copied: ReadSignal<bool>,
    set_share_copied: WriteSignal<bool>,
    wallet_only: ReadSignal<bool>,
    credit_thb: ReadSignal<u64>,
) -> AnyView {
    if data.status.eq_ignore_ascii_case("completed") {
        return completed_event_gateway(data, countdown, event_completed);
    }
    let i18n = use_i18n();

    let (reserve_in_view, set_reserve_in_view) = signal(false);
    // The hero CTA sits in the first screen, so the bar starts hidden: two
    // identical buttons on one screen, and the bar would cover the WHEN row
    // (QA 2026-09-29, item 1). It appears once the hero CTA scrolls away.
    let (hero_cta_in_view, set_hero_cta_in_view) = signal(true);
    let measure = move || {
        let viewport = window()
            .inner_height()
            .ok()
            .and_then(|h| h.as_f64())
            .unwrap_or(0.0);
        let in_view = document()
            .get_element_by_id("reserve")
            .is_some_and(|el| el.get_bounding_client_rect().top() < viewport);
        if in_view != reserve_in_view.get_untracked() {
            set_reserve_in_view.set(in_view);
        }
        let hero_in_view = document()
            .query_selector(".pe-hero-cta")
            .ok()
            .flatten()
            .is_some_and(|el| {
                let rect = el.get_bounding_client_rect();
                rect.bottom() > 0.0 && rect.top() < viewport
            });
        if hero_in_view != hero_cta_in_view.get_untracked() {
            set_hero_cta_in_view.set(hero_in_view);
        }
    };
    let scroll = window_event_listener(leptos::ev::scroll, move |_| measure());
    // Measure once after mount too: on a short screen the hero CTA starts
    // below the fold, and without this the first screen had no Reserve
    // action at all until the first scroll (390x600, .issues/182).
    request_animation_frame(measure);
    // Dropping a `WindowListenerHandle` does not remove the listener.
    on_cleanup(move || scroll.remove());

    let has_nft_image = !data.nft_image_url.is_empty();
    let event_id_for_ticket = data.id.clone();
    let cta_caption = super::attendance::cta_caption_of(&data);
    let has_description = !data.description.is_empty();
    let has_link = !data.link.is_empty();
    let has_deposit =
        data.deposit_enabled && (data.deposit_amount_usdc > 0 || data.deposit_amount_thb > 0);
    let is_hybrid = data.event_format == crate::api::EventFormat::Hybrid;
    let is_online_only = data.event_format == crate::api::EventFormat::Online;

    let escrow_status = data.escrow_status.as_deref().unwrap_or("");
    let escrow_closed =
        escrow_status == "closed" || escrow_status == "cancelled" || escrow_status == "deactivated";

    let name = data.name.clone();
    let tagline = data.tagline.clone();
    let description = data.description.clone();
    let link = data.link.clone();
    let nft_image_url = data.nft_image_url.clone();
    let poster_url = data.poster_url.clone();
    let community_links = data.community_links.clone();

    // Deposit label for registration form checkbox
    let deposit_label = crate::utils::money::deposit_consent_label(
        data.deposit_amount_thb,
        data.deposit_amount_usdc,
        escrow_closed,
    );

    let show_reg_form = !event_completed.get();
    let require_contact = data.require_contact_info;
    let require_photo_consent = data.require_photo_consent;
    let dev_profile_enabled = data.dev_profile_enabled;

    // Dynamic form config (Issue #049 Phase 2)
    let form_config = data.form_config.clone();

    let in_person_available = data.in_person_available;
    let online_available = data.online_available;
    let in_person_remaining = data.in_person_remaining;
    let online_remaining = data.online_remaining;

    // Registration form signals
    let (reg_name, set_reg_name) = signal(String::new());
    let (reg_email, set_reg_email) = signal(String::new());
    let (reg_participation, set_reg_participation) = signal(String::new());
    let (reg_contact_channel, set_reg_contact_channel) = signal(String::new());
    let (reg_contact_handle, set_reg_contact_handle) = signal(String::new());
    let (reg_deposit_agreed, set_reg_deposit_agreed) = signal(false);
    let (reg_consent_given, set_reg_consent_given) = signal(false);
    let (reg_photo_consent_given, set_reg_photo_consent_given) = signal(false);
    let (reg_consent_marketing, set_reg_consent_marketing) = signal(false);
    let (reg_state, set_reg_state) = signal(RegState::Idle);

    // Dynamic form field values (Issue #049 Phase 2)
    // Key = field key, Value = serialized value (string for text/select, JSON array for multiselect)
    let (dynamic_field_values, set_dynamic_field_values) =
        signal(std::collections::HashMap::<String, String>::new());

    let slug_for_signin = current_slug.clone();
    let slug_for_reg = data.slug.clone();

    // OG image meta tag — prefer the marketing poster, fall back to the NFT badge image.
    let og_image = if !poster_url.is_empty() {
        poster_url.clone()
    } else {
        nft_image_url.clone()
    };

    view! {
        // OG image meta
        {if !og_image.is_empty() {
            let img = og_image.clone();
            view! {
                <Meta property="og:image" content=img />
                <Meta property="twitter:card" content="summary_large_image" />
            }.into_any()
        } else {
            ().into_any()
        }}

        // Event hero — prefer marketing poster, fall back to NFT badge image, then Ticket icon.
        {event_hero(&poster_url, &nft_image_url, &data.slug)}

        // Event Name + Tagline
        <div class="pe-name-block">
            <h1 class="pe-name">{name}</h1>
            {organizer_line(&data.organizer_name)}
            {if !tagline.is_empty() {
                let t = tagline.clone();
                view! {
                    <p class="pe-tagline">{t}</p>
                }.into_any()
            } else {
                ().into_any()
            }}
        </div>

        // Postponed notice, above the CTA: the new date is the first thing a
        // returning registrant needs. Registration itself stays open.
        {crate::components::postponed_banner(&data.postponed_note)}

        // Hero CTA — the primary action, reachable above the fold. Jumps to the
        // reserve/action zone (the form itself is further down the page). Label
        // adapts once we know the attendee's registration state.
        {move || {
            if !show_reg_form {
                return ().into_any();
            }
            let label = match reg_lookup.get() {
                RegistrationLookup::Registered(_) => t_string!(i18n, event.cta_view_ticket),
                _ => t_string!(i18n, event.cta_reserve),
            };
            view! {
                <a href="#reserve" class="btn btn-primary btn-block pe-hero-cta">{label}</a>
                {cta_caption.map(super::attendance::cta_caption_view)}
            }.into_any()
        }}

        // Sticky mobile CTA — a persistent bottom action bar on phones (CSS hides
        // it on desktop). Keeps the primary action one tap away while scrolling,
        // and stays away while the hero CTA or the reserve zone is on screen:
        // it would otherwise double a "Reserve" button (.issues/173 C4).
        {move || {
            if !show_reg_form || reserve_in_view.get() || hero_cta_in_view.get() {
                return ().into_any();
            }
            let label = match reg_lookup.get() {
                RegistrationLookup::Registered(_) => t_string!(i18n, event.cta_view_ticket),
                _ => t_string!(i18n, event.cta_reserve),
            };
            view! {
                <a href="#reserve" class="pe-sticky-cta">{label}</a>
            }.into_any()
        }}

        // Event Details Card
        {details_card(&data, countdown, event_completed)}

        // Share / Add-to-Calendar — secondary actions, placed AFTER the details
        // (so they don't crowd the primary hero CTA, and "Add to Calendar" sits
        // right next to the date/time it saves).
        {share_button(&current_slug, &data.name, share_copied, set_share_copied)}

        // NFT Badge — the reward, shown above the deposit/reserve ask to motivate
        // ("here's what you'll earn") rather than buried after the form.
        {if has_nft_image {
            let url = nft_image_url.clone();
            view! {
                <div class="pe-card">
                    <h2 class="pe-section-title">
                        <Icon icon=IconName::Ticket class="icon-md" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.nft_badge_title))}
                    </h2>
                    <p class="pe-detail-secondary pe-mb-075">
                        {move || if is_online_only { t_string!(i18n, event.nft_badge_online) } else { t_string!(i18n, event.nft_badge_attend) }}
                    </p>
                    <img src=url alt=crate::locale::tr(|l| crate::i18n::td_string!(l, event.nft_badge_title)) class="pe-nft-img" />
                </div>
            }.into_any()
        } else {
            ().into_any()
        }}

        // Deposit Info Section
        {deposit_section(&data)}

        // Anchor target for the hero / sticky CTAs (scrolls the action zone into view).
        <div id="reserve" class="pe-anchor"></div>

        // Registration Section — auth-gated
        {if !show_reg_form {
            ().into_any()
        } else {
            let slug_for_signin = slug_for_signin.clone();
            let slug_for_reg = slug_for_reg.clone();
            view! {
                {move || {
                    let auth = auth_state.get();
                    match &auth {
                        AuthState::Checking => {
                            view! {
                                <div class="pe-card pe-text-center">
                                    <p class="pe-detail-secondary">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.checking_signin))}</p>
                                </div>
                            }.into_any()
                        }
                        AuthState::NotSignedIn => {
                            let slug = slug_for_signin.clone();
                            view! {
                                <div class="pe-card">
                                    <h2 class="pe-section-title">
                                        <Icon icon=IconName::Ticket class="icon-md" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.reserve_title))}
                                    </h2>
                                    <p class="pe-detail-secondary pe-mb-1">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.signin_prompt))}</p>
                                    <div style="display: flex; flex-direction: column; gap: 12px; margin-top: 16px;">
                                        <button
                                            class="btn-google"
                                            on:click=move |_| {
                                                let slug = slug.clone();
                                                leptos::task::spawn_local(async move {
                                                    let window = web_sys::window().expect("no window");
                                                    let origin = window.location().origin().unwrap_or_else(|_| "http://localhost:8787".to_string());
                                                    let redirect = format!("/e/{slug}");
                                                    let api_url = format!(
                                                        "{origin}/api/auth/url?redirect={}",
                                                        urlencoding::encode(&redirect)
                                                    );
                                                    match crate::api::fetch::get(&api_url, &[]).await {
                                                        Ok(resp) => {
                                                            if let Ok(body) = crate::api::fetch::response_text(&resp).await
                                                                && let Ok(json) = serde_json::from_str::<serde_json::Value>(&body)
                                                                    && let Some(auth_url) = json.get("data").and_then(|d| d.get("auth_url")).and_then(|u| u.as_str()) {
                                                                        navigateTo(auth_url);
                                                                        return;
                                                                    }
                                                        }
                                                        Err(e) => {
                                                            log::error!("[public_event] failed to get auth URL: {e}");
                                                        }
                                                    }
                                                    navigateTo("/login");
                                                });
                                            }
                                        >
                                            <span inner_html=google_icon()></span>
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, login.google))}
                                        </button>

                                        <crate::wallet_signin::WalletSignInButton
                                            on_success=Callback::new(move |_| {
                                                // Session cookie is set; reload so the page's
                                                // mount auth-check picks it up and shows the
                                                // registration form in place.
                                                if let Some(win) = web_sys::window() {
                                                    let _ = win.location().reload();
                                                }
                                            })
                                        />
                                    </div>
                                </div>
                            }.into_any()
                        }
                        AuthState::SignedIn(email) => {
                            let lookup = reg_lookup.get();
                            match &lookup {
                                RegistrationLookup::Pending => {
                                    view! {
                                        <div class="pe-card pe-text-center">
                                            <p class="pe-detail-secondary">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.checking_registration))}</p>
                                        </div>
                                    }.into_any()
                                }
                                RegistrationLookup::Registered(reg_data) => {
                                    registered_state(reg_data, email, &current_slug, &event_id_for_ticket)
                                }
                                RegistrationLookup::Error(err_msg) => {
                                    log::warn!("[public_event] registration lookup failed: {err_msg}");
                                    let email_val = email.clone();
                                    registration_form(
                                        slug_for_reg.clone(),
                                        email_val,
                                        wallet_only.get(),
                                        is_hybrid,
                                        require_contact,
                                        require_photo_consent,
                                        has_deposit,
                                        deposit_label.clone(),
                                        in_person_available,
                                        online_available,
                                        in_person_remaining,
                                        online_remaining,
                                        reg_name, set_reg_name,
                                        reg_email, set_reg_email,
                                        reg_participation, set_reg_participation,
                                        reg_contact_channel, set_reg_contact_channel,
                                        reg_contact_handle, set_reg_contact_handle,
                                        reg_deposit_agreed, set_reg_deposit_agreed,
                                        reg_consent_given, set_reg_consent_given,
                                        reg_photo_consent_given, set_reg_photo_consent_given,
                                        reg_consent_marketing, set_reg_consent_marketing,
                                        reg_state, set_reg_state,
                                        dev_profile_enabled,
                                        form_config.as_ref(),
                                        dynamic_field_values, set_dynamic_field_values,
                                    )
                                }
                                RegistrationLookup::NotRegistered => {
                                    let email_val = email.clone();
                                    // Reassure returning attendees that their rolling
                                    // credit will cover this event's deposit (THB path).
                                    let credit_amt = credit_thb.get();
                                    let show_credit = has_deposit && credit_amt > 0;
                                    // Wallet-only sessions can't spend credit until the
                                    // wallet is bound / they use Google (credit is tied to
                                    // a proven email). Explain that instead of silently
                                    // showing nothing on a deposit event.
                                    let show_wallet_credit_hint =
                                        has_deposit && wallet_only.get() && credit_amt == 0;
                                    let form = registration_form(
                                        slug_for_reg.clone(),
                                        email_val,
                                        wallet_only.get(),
                                        is_hybrid,
                                        require_contact,
                                        require_photo_consent,
                                        has_deposit,
                                        deposit_label.clone(),
                                        in_person_available,
                                        online_available,
                                        in_person_remaining,
                                        online_remaining,
                                        reg_name, set_reg_name,
                                        reg_email, set_reg_email,
                                        reg_participation, set_reg_participation,
                                        reg_contact_channel, set_reg_contact_channel,
                                        reg_contact_handle, set_reg_contact_handle,
                                        reg_deposit_agreed, set_reg_deposit_agreed,
                                        reg_consent_given, set_reg_consent_given,
                                        reg_photo_consent_given, set_reg_photo_consent_given,
                                        reg_consent_marketing, set_reg_consent_marketing,
                                        reg_state, set_reg_state,
                                        dev_profile_enabled,
                                        form_config.as_ref(),
                                        dynamic_field_values, set_dynamic_field_values,
                                    );
                                    view! {
                                        {if show_credit {
                                            view! {
                                                <div class="pe-card" style="background:rgba(20,241,149,0.08);border:1px solid rgba(20,241,149,0.3);">
                                                    <p class="pe-detail-secondary" style="margin:0;color:#14F195;font-weight:600;">
                                                        <Icon icon=IconName::CreditCard class="icon-sm" />" "{t!(i18n, event.credit_have, amount = credit_amt)}
                                                    </p>
                                                    <p class="pe-detail-secondary" style="margin:4px 0 0;">
                                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.credit_applied))}
                                                    </p>
                                                </div>
                                            }.into_any()
                                        } else if show_wallet_credit_hint {
                                            view! {
                                                <div class="pe-card" style="background:rgba(153,69,255,0.06);border:1px solid rgba(153,69,255,0.22);">
                                                    <p class="pe-detail-secondary" style="margin:0;font-size:0.82rem;line-height:1.45;">
                                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.credit_wallet_hint))}
                                                    </p>
                                                </div>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                        {form}
                                    }.into_any()
                                }
                            }
                        }
                    }
                }}
            }.into_any()
        }}

        // About this Event — moved below the action zone: it's reference content,
        // not a gate to the primary CTA (which is above the fold + sticky on mobile).
        {if has_description {
            let desc = description.clone();
            view! {
                <div class="pe-card">
                    <h2 class="pe-section-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.about_title))}</h2>
                    <p class="pe-description">{desc}</p>
                </div>
            }.into_any()
        } else {
            ().into_any()
        }}

        // Join the Community — engagement/social content, kept at the bottom
        // (below the action zone), not competing with the reserve CTA.
        {crate::pages::ticket::community_links::community_links_section(community_links.clone(), crate::pages::ticket::community_links::CommunityLinksVariant::PublicEvent)}

        // External Link
        {if has_link {
            let href = link.clone();
            view! {
                <div class="pe-card">
                    <h2 class="pe-section-title">
                        <Icon icon=IconName::Link class="icon-sm" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.external_link_title))}
                    </h2>
                    <a href=href target="_blank" rel="noopener noreferrer" class="pe-ext-link">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.view_event_page))}
                    </a>
                </div>
            }.into_any()
        } else {
            ().into_any()
        }}

        // Sponsors — last, directly above the footer.
        {sponsor_row(&data.sponsors)}
    }.into_any()
}

/// Completed events deliberately use a different action surface from live
/// events. Retrospective enrollment is a learning/community lead: it must not
/// expose reservation, deposit, check-in, quiz, or NFT-claim controls.
fn completed_event_gateway(
    data: PublicEventData,
    countdown: ReadSignal<String>,
    event_completed: ReadSignal<bool>,
) -> AnyView {
    let name = data.name.clone();
    let tagline = data.tagline.clone();
    let description = data.description.clone();
    let slug = data.slug.clone();
    let archive_url = data.archive_url.clone();
    let enrollment_open = data.post_event_registration_accepting;
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
            <p class="pe-detail-secondary pe-mb-075">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.ended_body))}</p>

            <div class="pe-btn-row-center">
                {if !archive_url.is_empty() {
                    let href = archive_url.clone();
                    view! {
                        <a href=href target="_blank" rel="noopener noreferrer" class="btn btn-outline btn-sm">
                            <Icon icon=IconName::Link class="icon-sm" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.view_archive))}
                        </a>
                    }.into_any()
                } else {
                    view! {
                        <span class="pe-detail-secondary">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.archive_soon))}</span>
                    }.into_any()
                }}

                {if enrollment_open {
                    let href = format!("/events/{slug}/post-event-register");
                    view! {
                        <a href=href class="btn btn-primary btn-sm">" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.join_community))}</a>
                    }.into_any()
                } else {
                    ().into_any()
                }}
            </div>
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
