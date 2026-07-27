use super::capacity_indicator::capacity_indicator;
use super::details_card::details_card;
use super::deposit_section::deposit_section;
use super::event_hero::event_hero;
use super::registered_state::registered_state;
use super::registration_form::registration_form;
use super::share_button::share_button;
use super::types::*;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params;

#[allow(non_snake_case)]
pub fn PublicEvent() -> impl IntoView {
    let params = use_params::<PublicEventParams>();

    // Reactive state
    let (state, set_state) = signal(PublicEventState::Loading);
    let (countdown, set_countdown) = signal(String::new());
    let (event_completed, set_event_completed) = signal(false);
    let (event_name, set_event_name) = signal(String::new());
    let (share_copied, set_share_copied) = signal(false);

    // Auth state
    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    let (reg_lookup, set_reg_lookup) = signal(RegistrationLookup::Pending);

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

        log::info!("[public_event] fetching slug: {slug}");
        let slug_clone = slug.clone();
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
                                    log::info!("[public_event] parsed OK, success={}", api_resp.success);
                                    if api_resp.success {
                                        if let Some(data) = api_resp.data {
                                            let is_completed =
                                                data.status == "completed" || data.status == "Completed";
                                            let start_ms = data.event_start_ms;
                                            let name = data.name.clone();
                                            set_event_name.set(name);
                                            set_event_completed.set(is_completed);
                                            set_state.set(PublicEventState::Loaded(data));

                                            // Start countdown if event is in the future
                                            let now_ms = js_sys::Date::now() as i64;
                                            if !is_completed && start_ms > now_ms {
                                                set_countdown.set(format_countdown(start_ms - now_ms));

                                                if let Ok(handle) = set_interval_with_handle(
                                                    move || {
                                                        let now = js_sys::Date::now() as i64;
                                                        let remaining = start_ms - now;
                                                        if remaining <= 0 {
                                                            set_countdown.set(String::new());
                                                        } else {
                                                            set_countdown.set(format_countdown(remaining));
                                                        }
                                                    },
                                                    std::time::Duration::from_secs(1),
                                                ) {
                                                    on_cleanup(move || handle.clear());
                                                }
                                            }
                                        } else {
                                            set_state.set(PublicEventState::Error(
                                                "No event data returned".to_string(),
                                            ));
                                        }
                                    } else {
                                        set_state.set(PublicEventState::Error(
                                            api_resp.error.unwrap_or_else(|| "Unknown error".to_string()),
                                        ));
                                    }
                                }
                                Err(e) => {
                                    log::error!("[public_event] JSON parse error: {e}");
                                    set_state.set(PublicEventState::Error(format!(
                                        "Failed to parse response: {e}"
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("[public_event] body read error: {e}");
                            set_state.set(PublicEventState::Error("Failed to read response".to_string()));
                        }
                    }
                }
                Err(e) => {
                    log::error!("[public_event] fetch error: {e}");
                    set_state.set(PublicEventState::Error(format!("Failed to fetch event: {e}")));
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
                                    if let Ok(body) = crate::api::fetch::response_text(&resp).await {
                                        match serde_json::from_str::<MyRegistrationResponse>(&body) {
                                            Ok(api_resp) => {
                                                if let Some(data) = api_resp.data {
                                                    log::info!(
                                                        "[public_event] {email_clone} already registered for {slug}"
                                                    );
                                                    set_reg_lookup
                                                        .set(RegistrationLookup::Registered(data));
                                                } else {
                                                    set_reg_lookup.set(RegistrationLookup::NotRegistered);
                                                }
                                            }
                                            Err(e) => {
                                                log::warn!(
                                                    "[public_event] my-registration parse error: {e}"
                                                );
                                                set_reg_lookup.set(RegistrationLookup::NotRegistered);
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
                                set_reg_lookup.set(RegistrationLookup::Error(format!(
                                    "Fetch error: {e}"
                                )));
                            }
                        }
                    });
                }
            }
            AuthState::Checking | AuthState::NotSignedIn => {}
        }
    });

    // Dynamic title
    let title_text = move || {
        let name = event_name.get();
        if name.is_empty() {
            "Event — BeThere".to_string()
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
                        "← Back to BeThere"
                    </a>
                </div>

                // Page state
                {move || {
                    let s = state.get();
                    match s {
                        PublicEventState::Loading => {
                            view! {
                                <div class="pe-loading">
                                    <div class="pe-icon-mb"><Icon icon=IconName::Ticket class="icon-2xl" /></div>
                                    <p class="pe-detail-secondary">"Loading event..."</p>
                                </div>
                            }.into_any()
                        }
                        PublicEventState::NotFound => {
                            view! {
                                <div class="pe-loading">
                                    <div class="pe-icon-mb"><Icon icon=IconName::Search class="icon-2xl" /></div>
                                    <h1 class="pe-error-title">"Event Not Found"</h1>
                                    <p class="pe-detail-secondary pe-msg-mb-lg">
                                        "This event doesn't exist or is not publicly available."
                                    </p>
                                    <a href="/" class="btn btn-primary">"Go Home"</a>
                                </div>
                            }.into_any()
                        }
                        PublicEventState::Error(msg) => {
                            let msg_display = msg.clone();
                            view! {
                                <div class="pe-loading">
                                    <div class="pe-icon-mb"><Icon icon=IconName::Warning class="icon-md icon-danger" /></div>
                                    <h1 class="pe-error-title">"Something went wrong"</h1>
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
                                            "Try Again"
                                        </button>
                                        <a href="/" class="btn btn-outline">"Go Home"</a>
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
                            )
                        }
                    }
                }}

                // Footer
                <div class="pe-footer">
                    <p>
                        "Powered by "
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

fn render_loaded_event(
    data: PublicEventData,
    countdown: ReadSignal<String>,
    event_completed: ReadSignal<bool>,
    auth_state: ReadSignal<AuthState>,
    reg_lookup: ReadSignal<RegistrationLookup>,
    current_slug: String,
    share_copied: ReadSignal<bool>,
    set_share_copied: WriteSignal<bool>,
) -> AnyView {
    let has_nft_image = !data.nft_image_url.is_empty();
    let has_description = !data.description.is_empty();
    let has_link = !data.link.is_empty();
    let has_deposit = data.deposit_enabled
        && (data.deposit_amount_usdc > 0.0 || data.deposit_amount_thb > 0.0);
    let is_hybrid = data.event_format == crate::api::EventFormat::Hybrid;
    let is_online_only = data.event_format == crate::api::EventFormat::Online;

    let escrow_status = data.escrow_status.as_deref().unwrap_or("");
    let escrow_closed = escrow_status == "closed"
        || escrow_status == "cancelled"
        || escrow_status == "deactivated";

    let name = data.name.clone();
    let tagline = data.tagline.clone();
    let description = data.description.clone();
    let link = data.link.clone();
    let nft_image_url = data.nft_image_url.clone();
    let poster_url = data.poster_url.clone();
    let community_links = data.community_links.clone();

    // Deposit label for registration form checkbox
    let deposit_label = if escrow_closed {
        if data.deposit_amount_thb > 0.0 {
            format!("{} Baht", format_thb(data.deposit_amount_thb))
        } else {
            format_usdc(data.deposit_amount_usdc)
        }
    } else if data.deposit_amount_usdc > 0.0 && data.deposit_amount_thb > 0.0 {
        format!(
            "{} (~{} Baht)",
            format_usdc(data.deposit_amount_usdc),
            format_thb(data.deposit_amount_thb)
        )
    } else if data.deposit_amount_usdc > 0.0 {
        format_usdc(data.deposit_amount_usdc)
    } else {
        format_thb(data.deposit_amount_thb)
    };

    let show_reg_form = !event_completed.get();
    let require_contact = data.require_contact_info;
    let dev_profile_enabled = data.dev_profile_enabled;

    // Dynamic form config (Issue #049 Phase 2)
    let form_config = data.form_config.clone();

    let in_person_available = data.in_person_available;
    let online_available = data.online_available;
    let in_person_remaining = data.in_person_remaining;
    let online_remaining = data.online_remaining;
    let in_person_capacity = data.in_person_capacity;

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
    let (dynamic_field_values, set_dynamic_field_values) = signal(std::collections::HashMap::<String, String>::new());

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
        {event_hero(&poster_url, &nft_image_url)}

        // Event Name + Tagline
        <div class="pe-name-block">
            <h1 class="pe-name">{name}</h1>
            {if !tagline.is_empty() {
                let t = tagline.clone();
                view! {
                    <p class="pe-tagline">{t}</p>
                }.into_any()
            } else {
                ().into_any()
            }}
        </div>

        // Share button
        {share_button(&current_slug, &data.name, share_copied, set_share_copied)}

        // Event Details Card
        {details_card(&data, countdown, event_completed)}

        // About this Event
        {if has_description {
            let desc = description.clone();
            view! {
                <div class="pe-card">
                    <h2 class="pe-section-title">"About this Event"</h2>
                    <p class="pe-description">{desc}</p>
                </div>
            }.into_any()
        } else {
            ().into_any()
        }}

        // Community Links
        {crate::pages::ticket::community_links::community_links_section(community_links.clone(), crate::pages::ticket::community_links::CommunityLinksVariant::PublicEvent)}

        // Deposit Info Section
        {deposit_section(&data)}

        // Capacity indicator
        {capacity_indicator(in_person_capacity, online_remaining, in_person_remaining)}

        // Signed-in indicator + logout
        {move || {
            match &auth_state.get() {
                AuthState::SignedIn(email) => {
                    let email_disp = email.clone();
                    view! {
                        <div class="pe-auth-bar">
                            <span class="pe-detail-secondary">
                                {format!("👤 {email_disp}")}
                            </span>
                            <button
                                class="btn btn-outline btn-xs"
                                on:click=move |_| {
                                    leptos::task::spawn_local(async move {
                                        let _ = crate::api::fetch::post("/api/auth/logout", &[], None).await;
                                        navigateTo("/");
                                    });
                                }
                            >
                                "Sign out"
                            </button>
                        </div>
                    }.into_any()
                }
                _ => ().into_any(),
            }
        }}

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
                                    <p class="pe-detail-secondary">"Checking sign-in status..."</p>
                                </div>
                            }.into_any()
                        }
                        AuthState::NotSignedIn => {
                            let slug = slug_for_signin.clone();
                            view! {
                                <div class="pe-card">
                                    <h2 class="pe-section-title">
                                        <Icon icon=IconName::Ticket class="icon-md" />" Reserve Your Spot"
                                    </h2>
                                    <p class="pe-detail-secondary pe-mb-1">
                                        "Sign in with Google to register for this event."
                                    </p>
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
                                        "Sign in with Google"
                                    </button>
                                </div>
                            }.into_any()
                        }
                        AuthState::SignedIn(email) => {
                            let lookup = reg_lookup.get();
                            match &lookup {
                                RegistrationLookup::Pending => {
                                    view! {
                                        <div class="pe-card pe-text-center">
                                            <p class="pe-detail-secondary">"Checking registration..."</p>
                                        </div>
                                    }.into_any()
                                }
                                RegistrationLookup::Registered(reg_data) => {
                                    registered_state(reg_data, email, &current_slug)
                                }
                                RegistrationLookup::Error(err_msg) => {
                                    log::warn!("[public_event] registration lookup failed: {err_msg}");
                                    let email_val = email.clone();
                                    registration_form(
                                        slug_for_reg.clone(),
                                        email_val,
                                        is_hybrid,
                                        require_contact,
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
                                    registration_form(
                                        slug_for_reg.clone(),
                                        email_val,
                                        is_hybrid,
                                        require_contact,
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
                            }
                        }
                    }
                }}
            }.into_any()
        }}

        // NFT Badge Section
        {if has_nft_image {
            let url = nft_image_url.clone();
            view! {
                <div class="pe-card">
                    <h2 class="pe-section-title">
                        <Icon icon=IconName::Ticket class="icon-md" />" NFT Badge"
                    </h2>
                    <p class="pe-detail-secondary pe-mb-075">
                        {if is_online_only { "Earn this NFT badge when you complete the quest after the event." } else { "Earn a commemorative NFT badge when you attend." }}
                    </p>
                    <img src=url alt="NFT Badge" class="pe-nft-img" />
                </div>
            }.into_any()
        } else {
            ().into_any()
        }}

        // External Link
        {if has_link {
            let href = link.clone();
            view! {
                <div class="pe-card">
                    <h2 class="pe-section-title">
                        <Icon icon=IconName::Link class="icon-sm" />" External Link"
                    </h2>
                    <a href=href target="_blank" rel="noopener noreferrer" class="pe-ext-link">
                        "View Event Page →"
                    </a>
                </div>
            }.into_any()
        } else {
            ().into_any()
        }}
    }.into_any()
}
