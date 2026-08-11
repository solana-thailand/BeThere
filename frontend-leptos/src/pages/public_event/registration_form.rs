use super::types::*;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Minimal client-side email sanity check (mirrors the worker's server check).
fn email_looks_valid(email: &str) -> bool {
    let e = email.trim();
    if e.len() < 3 || e.contains(char::is_whitespace) {
        return false;
    }
    match e.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
        }
        None => false,
    }
}

#[allow(clippy::too_many_arguments)] // plain builder fn wiring many Leptos signals; splitting the signature is out of scope
pub fn registration_form(
    slug_for_reg: String,
    locked_email: String,
    // Plan 017: wallet-only session — the email field becomes an editable,
    // required input (locked_email is the synthetic `wallet:<addr>`, not usable).
    wallet_only: bool,
    is_hybrid: bool,
    require_contact: bool,
    has_deposit: bool,
    deposit_label: String,
    in_person_available: bool,
    online_available: bool,
    in_person_remaining: Option<u32>,
    online_remaining: Option<u32>,
    reg_name: ReadSignal<String>,
    set_reg_name: WriteSignal<String>,
    reg_email: ReadSignal<String>,
    set_reg_email: WriteSignal<String>,
    reg_participation: ReadSignal<String>,
    set_reg_participation: WriteSignal<String>,
    reg_contact_channel: ReadSignal<String>,
    set_reg_contact_channel: WriteSignal<String>,
    reg_contact_handle: ReadSignal<String>,
    set_reg_contact_handle: WriteSignal<String>,
    reg_deposit_agreed: ReadSignal<bool>,
    set_reg_deposit_agreed: WriteSignal<bool>,
    reg_consent_given: ReadSignal<bool>,
    set_reg_consent_given: WriteSignal<bool>,
    reg_photo_consent_given: ReadSignal<bool>,
    set_reg_photo_consent_given: WriteSignal<bool>,
    reg_consent_marketing: ReadSignal<bool>,
    set_reg_consent_marketing: WriteSignal<bool>,
    reg_state: ReadSignal<RegState>,
    set_reg_state: WriteSignal<RegState>,
    dev_profile_enabled: bool,
    form_config: Option<&RegistrationFormConfig>,
    dynamic_field_values: ReadSignal<HashMap<String, String>>,
    set_dynamic_field_values: WriteSignal<HashMap<String, String>>,
) -> AnyView {
    // Pre-fill email from JWT — but NOT for wallet-only sessions, where
    // locked_email is a synthetic `wallet:<address>` and the user must type a
    // real one.
    if !wallet_only {
        set_reg_email.set(locked_email.clone());
    }

    let (field_errors, set_field_errors) = signal(FieldErrors::default());

    // Resolve form config: use provided config or defaults
    let resolved_config = match form_config {
        Some(cfg) => cfg.clone(),
        None => RegistrationFormConfig::default(),
    };
    let section_label = Arc::new(resolved_config.section_label.clone());
    let form_fields = Arc::new(resolved_config.fields.clone());

    // Auto-fill from localStorage for returning users
    if let Some(saved_json) = loadDevProfile()
        && let Ok(profile) = serde_json::from_str::<SavedDevProfile>(&saved_json) {
            if !profile.name.is_empty() && reg_name.get().is_empty() {
                set_reg_name.set(profile.name.clone());
            }
            if !profile.contact_channel.is_empty() && reg_contact_channel.get().is_empty() {
                set_reg_contact_channel.set(profile.contact_channel.clone());
            }
            if !profile.contact_handle.is_empty() && reg_contact_handle.get().is_empty() {
                set_reg_contact_handle.set(profile.contact_handle.clone());
            }
            if !profile.fields.is_empty() {
                set_dynamic_field_values.update(|vals| {
                    for (k, v) in &profile.fields {
                        if !v.is_empty() && !vals.contains_key(k) {
                            vals.insert(k.clone(), v.clone());
                        }
                    }
                });
            }
            log::info!(
                "[registration_form] pre-filled {} fields from saved dev profile",
                profile.fields.len()
            );
        }

    view! {
        {move || {
            let current_reg = reg_state.get();
            let dep_label = deposit_label.clone();
            match &current_reg {
                RegState::Success(data) => {
                    let next_url = data.next_step.url.clone();
                    let attendee_id = data.attendee_id.clone();
                    let eid = next_url
                        .split("event_id=")
                        .nth(1)
                        .map(|s| s.split('&').next().unwrap_or(s).to_string())
                        .unwrap_or_default();
                    let slug_for_ls = slug_for_reg.clone();

                    saveProgress(&attendee_id, &eid, &slug_for_ls);

                    // Save profile for auto-fill on future events
                    let saved = SavedDevProfile {
                        name: data.name.clone(),
                        contact_channel: reg_contact_channel.get(),
                        contact_handle: reg_contact_handle.get(),
                        fields: dynamic_field_values.get(),
                    };
                    if let Ok(json) = serde_json::to_string(&saved) {
                        saveDevProfile(&json);
                    }

                    // Plan 017: if the wallet couldn't be linked (email already had an
                    // account), pause on this screen with guidance instead of the fast
                    // auto-redirect, so the user learns how to merge the two.
                    let wallet_not_linked = matches!(data.wallet_linked, Some(false));

                    let redirect_url = next_url.clone();
                    if !wallet_not_linked {
                        leptos::task::spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(800).await;
                            navigateTo(&redirect_url);
                        });
                    }

                    let continue_url = next_url.clone();
                    view! {
                        <div class="pe-card">
                            <div class="pe-text-center">
                                <div class="pe-success-icon-lg">
                                    <Icon icon=IconName::Check class="icon-2xl icon-success" />
                                </div>
                                <h2 class="pe-section-title pe-title-success">
                                    "You're registered!"
                                </h2>
                                <p class="pe-detail-secondary pe-mb-1">
                                    {format!("Welcome, {}!", data.name)}
                                </p>
                                {if wallet_not_linked {
                                    view! {
                                        <div style="background:rgba(153,69,255,0.08);border:1px solid rgba(153,69,255,0.25);border-radius:8px;padding:10px 12px;margin:12px 0;font-size:0.82rem;line-height:1.45;color:#cbd5e1;text-align:left;">
                                            <strong style="color:#fff;">"Heads up: "</strong>
                                            "this email already has an account, so your wallet wasn't linked to it. To sign in with your wallet next time, open your Profile (signed in with Google), then press \"Connect Wallet\"."
                                        </div>
                                        <button class="pe-submit-btn" on:click=move |_| navigateTo(&continue_url)>
                                            "Continue →"
                                        </button>
                                    }.into_any()
                                } else {
                                    view! { <p class="pe-detail-secondary">"Redirecting..."</p> }.into_any()
                                }}
                            </div>
                        </div>
                    }.into_any()
                }
                RegState::Error(msg) => {
                    let msg_clone = msg.clone();
                    view! {
                        <div class="pe-card">
                            <h2 class="pe-section-title">
                                <Icon icon=IconName::Ticket class="icon-md" />" Reserve Your Spot"
                            </h2>
                            <div class="pe-error-box">
                                {msg_clone}
                            </div>
                            <button
                                class="btn btn-outline btn-block"
                                on:click=move |_| set_reg_state.set(RegState::Idle)
                            >
                                "Try Again"
                            </button>
                        </div>
                    }.into_any()
                }
                RegState::Submitting => {
                    view! {
                        <div class="pe-card pe-text-center">
                            <div class="pe-icon-mb-sm"><Icon icon=IconName::Hourglass class="icon-md" /></div>
                            <p class="pe-detail-secondary">"Registering..."</p>
                        </div>
                    }.into_any()
                }
                RegState::Idle => {
                    let slug = slug_for_reg.clone();
                    let email_for_display = locked_email.clone();
                    let email_for_submit = locked_email.clone();
                    view! {
                        <div class="pe-card">
                            <h2 class="pe-section-title">
                                <Icon icon=IconName::Ticket class="icon-md" />" Reserve Your Spot"
                            </h2>
                            <div class="pe-flex-col-gap-md">
                                // Name
                                <div class="pe-field" id="pe-field-name">
                                    <label class="pe-field-label">"Name"<span class="pe-required">" *"</span></label>
                                    <input
                                        type="text"
                                        placeholder="Your name"
                                        class="pe-input"
                                        prop:class=move || if field_errors.get().name.is_some() { "pe-input--error" } else { "" }
                                        prop:value=move || reg_name.get()
                                        on:input=move |ev| {
                                            set_reg_name.set(event_target_value(&ev));
                                            set_field_errors.update(|e| e.name = None);
                                        }
                                    />
                                    {move || match &field_errors.get().name {
                                        Some(err) => view! { <span class="pe-field-error">{err.clone()}</span> }.into_any(),
                                        None => view! { <div></div> }.into_any(),
                                    }}
                                </div>
                                // Email — locked for Google sessions; editable + required for wallet-only
                                <div class="pe-field" id="pe-field-email">
                                    <label class="pe-field-label">
                                        "Email Address"
                                        {if wallet_only { view!{ <span class="pe-required">" *"</span> }.into_any() } else { ().into_any() }}
                                    </label>
                                    {if wallet_only {
                                        view! {
                                            <input
                                                type="email"
                                                placeholder="you@example.com"
                                                class="pe-input"
                                                prop:class=move || if field_errors.get().email.is_some() { "pe-input--error" } else { "" }
                                                prop:value=move || reg_email.get()
                                                on:input=move |ev| {
                                                    set_reg_email.set(event_target_value(&ev));
                                                    set_field_errors.update(|e| e.email = None);
                                                }
                                            />
                                            <span class="pe-field-hint">"We'll link this email to your wallet so the organizer can reach you."</span>
                                            {move || match &field_errors.get().email {
                                                Some(err) => view! { <span class="pe-field-error">{err.clone()}</span> }.into_any(),
                                                None => view! { <div></div> }.into_any(),
                                            }}
                                        }.into_any()
                                    } else {
                                        view! {
                                            <input
                                                type="email"
                                                value=email_for_display
                                                readonly
                                                class="pe-input pe-input--locked"
                                            />
                                        }.into_any()
                                    }}
                                </div>
                                // Participation type (hybrid only)
                                {move || {
                                    if is_hybrid {
                                        let ip_label = match in_person_remaining {
                                            Some(r) => format!("In-Person (on-site) — {r} spots left"),
                                            None => "In-Person (on-site)".to_string(),
                                        };
                                        let on_label = match online_remaining {
                                            Some(r) => format!("Online (virtual) — {r} spots left"),
                                            None => "Online (virtual)".to_string(),
                                        };
                                        view! {
                                            <div class="pe-field">
                                                <label class="pe-field-label">"Select Track"</label>
                                                <select
                                                    class="pe-input"
                                                    on:change=move |ev| set_reg_participation.set(event_target_value(&ev))
                                                >
                                                    <option value="">"Select track..."</option>
                                                    {if in_person_available {
                                                        view! { <option value="In-Person">{ip_label}</option> }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                    {if online_available {
                                                        view! { <option value="Online">{on_label}</option> }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                </select>
                                            </div>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }
                                }}
                                // Contact Channel
                                <div class="pe-field" id="pe-field-channel">
                                    <label class="pe-field-label">
                                        "Preferred Contact Channel"
                                        {if require_contact {
                                            view! { <span class="pe-required">" *"</span> }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </label>
                                    <select
                                        class="pe-input"
                                        prop:class=move || if field_errors.get().contact_channel.is_some() { "pe-input--error" } else { "" }
                                        prop:value=move || reg_contact_channel.get()
                                        on:change=move |ev| {
                                            set_reg_contact_channel.set(event_target_value(&ev));
                                            set_field_errors.update(|e| e.contact_channel = None);
                                        }
                                    >
                                        <option value="">"Select channel..."</option>
                                        <option value="Telegram">"Telegram"</option>
                                        <option value="Line">"Line"</option>
                                        <option value="Facebook">"Facebook"</option>
                                        <option value="X (Twitter)">"X (Twitter)"</option>
                                    </select>
                                    {move || match &field_errors.get().contact_channel {
                                        Some(err) => view! { <span class="pe-field-error">{err.clone()}</span> }.into_any(),
                                        None => view! { <div></div> }.into_any(),
                                    }}
                                </div>
                                // Contact Handle
                                <div class="pe-field" id="pe-field-handle">
                                    <label class="pe-field-label">
                                        "Contact Username / Profile Link"
                                        {if require_contact {
                                            view! { <span class="pe-required">" *"</span> }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </label>
                                    <input
                                        type="text"
                                        placeholder="Username or profile link"
                                        class="pe-input"
                                        prop:class=move || if field_errors.get().contact_handle.is_some() { "pe-input--error" } else { "" }
                                        prop:value=move || reg_contact_handle.get()
                                        on:input=move |ev| {
                                            set_reg_contact_handle.set(event_target_value(&ev));
                                            set_field_errors.update(|e| e.contact_handle = None);
                                        }
                                    />
                                    {move || match &field_errors.get().contact_handle {
                                        Some(err) => view! { <span class="pe-field-error">{err.clone()}</span> }.into_any(),
                                        None => view! { <div></div> }.into_any(),
                                    }}
                                </div>

                                // Dynamic Developer Profile Section (Issue #049 Phase 2)
                                {
                                    let sl = Arc::clone(&section_label);
                                    let ff = Arc::clone(&form_fields);
                                    move || {
                                    if dev_profile_enabled {
                                        let label = sl.as_ref().clone();
                                        let fields = ff.as_ref().clone();
                                        view! {
                                            <div class="pe-dev-profile-section">
                                                <label class="pe-label">{label}</label>
                                                <For
                                                    each=move || fields.clone()
                                                    key=|field| field.key.clone()
                                                    children=move |field: FormFieldConfig| {
                                                        render_dynamic_field(
                                                            field,
                                                            dynamic_field_values,
                                                            set_dynamic_field_values,
                                                        )
                                                    }
                                                />
                                            </div>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }
                                }}

                                // Single unified consent checkbox
                                {move || {
                                    let is_online_track = is_hybrid && reg_participation.get().to_lowercase().contains("online");
                                    let show_deposit = has_deposit && !is_online_track;
                                    let dep_label = dep_label.clone();
                                    view! {
                                        <div id="pe-field-consent">
                                            <label class="pe-checkbox-label">
                                                <input
                                                    type="checkbox"
                                                    class="pe-checkbox"
                                                    checked=move || reg_consent_given.get()
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        set_reg_consent_given.set(checked);
                                                        set_reg_deposit_agreed.set(checked);
                                                        set_reg_photo_consent_given.set(checked);
                                                        set_reg_consent_marketing.set(checked);
                                                        set_field_errors.update(|e| {
                                                            e.consent_given = None;
                                                            e.deposit_agreed = None;
                                                            e.photo_consent_given = None;
                                                        });
                                                    }
                                                />
                                                <span>
                                                    "I agree to the "
                                                    <a href="/privacy" target="_blank" class="pe-ext-link">"Privacy Policy"</a>
                                                    {if show_deposit {
                                                        format!(" and authorize the {} commitment deposit (refunded upon check-in).", dep_label).into_any()
                                                    } else {
                                                        " for registration, check-in, and NFT issuance.".into_any()
                                                    }}
                                                </span>
                                            </label>
                                            {move || match (&field_errors.get().consent_given, &field_errors.get().deposit_agreed) {
                                                (Some(err), _) | (_, Some(err)) => view! { <span class="pe-field-error pe-field-error-indent">{err.clone()}</span> }.into_any(),
                                                _ => view! { <div></div> }.into_any(),
                                            }}
                                        </div>
                                    }.into_any()
                                }}
                                // Submit button
                                {
                                    let slug = slug.clone();
                                    let email_sub = email_for_submit.clone();
                                    let submit_fields = form_fields.as_ref().clone();
                                    view! {
                                        <button
                                            class="pe-submit-btn"
                                            on:click=move |_| {
                                                let name_val = reg_name.get();
                                                let part_val = reg_participation.get();
                                                let channel_val = reg_contact_channel.get();
                                                let handle_val = reg_contact_handle.get();
                                                let deposit_val = reg_deposit_agreed.get();
                                                // Wallet-only sessions submit the typed email; Google sessions the locked one.
                                                let email_val = if wallet_only { reg_email.get() } else { email_sub.clone() };

                                                let mut errors = FieldErrors::default();
                                                if name_val.trim().is_empty() {
                                                    errors.name = Some("Name is required".to_string());
                                                }
                                                if wallet_only && !email_looks_valid(email_val.trim()) {
                                                    errors.email = Some("Please enter a valid email".to_string());
                                                }
                                                if require_contact && channel_val.trim().is_empty() {
                                                    errors.contact_channel = Some("Please select a channel".to_string());
                                                }
                                                if require_contact && handle_val.trim().is_empty() {
                                                    errors.contact_handle = Some("Please provide your contact info".to_string());
                                                }
                                                let consent_val = reg_consent_given.get();
                                                if !consent_val {
                                                    errors.consent_given = Some("You must agree to continue".to_string());
                                                }
                                                let photo_consent_val = reg_photo_consent_given.get();
                                                let is_online_track = is_hybrid && part_val.to_lowercase().contains("online");
                                                if has_deposit && !is_online_track && !deposit_val {
                                                    errors.deposit_agreed = Some("You must agree to the deposit".to_string());
                                                }

                                                let has_errors = errors.name.is_some()
                                                    || errors.email.is_some()
                                                    || errors.contact_channel.is_some()
                                                    || errors.contact_handle.is_some()
                                                    || errors.consent_given.is_some()
                                                    || errors.deposit_agreed.is_some();

                                                // Determine scroll target before moving errors
                                                let scroll_target = errors.name.as_ref()
                                                    .map(|_| "pe-field-name")
                                                    .or(errors.email.as_ref().map(|_| "pe-field-email"))
                                                    .or(errors.contact_channel.as_ref().map(|_| "pe-field-channel"))
                                                    .or(errors.contact_handle.as_ref().map(|_| "pe-field-handle"))
                                                    .or(errors.consent_given.as_ref().map(|_| "pe-field-consent"))
                                                    .or(errors.deposit_agreed.as_ref().map(|_| "pe-field-deposit"));

                                                set_field_errors.set(errors);

                                                if has_errors {
                                                    if let Some(id) = scroll_target {
                                                        scroll_to_element(id);
                                                    }
                                                    return;
                                                }

                                                // Extract dynamic field values for submission
                                                let dynamic_vals = dynamic_field_values.get();
                                                let (experience_level, tech_stack, interests) =
                                                    extract_profile_fields(&dynamic_vals, &submit_fields);

                                                // Build dynamic profile_fields map (Issue #049 Phase 2)
                                                let profile_fields: std::collections::HashMap<String, String> = submit_fields
                                                    .iter()
                                                    .filter(|f| f.profile_field)
                                                    .filter_map(|f| {
                                                        dynamic_vals.get(&f.key).cloned()
                                                            .filter(|v| !v.is_empty())
                                                            .map(|v| (f.key.clone(), v))
                                                    })
                                                    .collect();

                                                set_reg_state.set(RegState::Submitting);
                                                let body = RegisterBody {
                                                    slug: slug.clone(),
                                                    name: name_val.trim().to_string(),
                                                    email: email_val.trim().to_lowercase(),
                                                    participation_type: if part_val.is_empty() { None } else { Some(part_val.clone()) },
                                                    contact_channel: if channel_val.trim().is_empty() { None } else { Some(channel_val.trim().to_string()) },
                                                    contact_handle: if handle_val.trim().is_empty() { None } else { Some(handle_val.trim().to_string()) },
                                                    deposit_agreed: if deposit_val { Some(true) } else { None },
                                                    consent_given: if consent_val { Some(true) } else { None },
                                                    photo_consent_given: if photo_consent_val { Some(true) } else { None },
                                                    consent_marketing: if reg_consent_marketing.get() { Some(true) } else { None },
                                                    experience_level,
                                                    tech_stack,
                                                    interests,
                                                    profile_fields: if profile_fields.is_empty() { None } else { Some(profile_fields) },
                                                };

                                                leptos::task::spawn_local(async move {
                                                    let window = web_sys::window().expect("no window");
                                                    let origin = window.location().origin().unwrap_or_else(|_| "http://localhost:8787".to_string());
                                                    let url = format!("{origin}/api/public/register");

                                                    match crate::api::fetch::post(&url, &[("Content-Type", "application/json")], Some(serde_json::to_string(&body).unwrap_or_default())).await
                                                    {
                                                        Ok(resp) => {
                                                            if resp.status() == 401 {
                                                                set_reg_state.set(RegState::Error(
                                                                    "Session expired. Please sign in again.".to_string()
                                                                ));
                                                                return;
                                                            }
                                                            match crate::api::fetch::response_text(&resp).await {
                                                                Ok(text) => {
                                                                    match serde_json::from_str::<RegisterResponse>(&text) {
                                                                        Ok(api_resp) => {
                                                                            if api_resp.success {
                                                                                if let Some(data) = api_resp.data {
                                                                                    set_reg_state.set(RegState::Success(data));
                                                                                } else {
                                                                                    set_reg_state.set(RegState::Error("No data returned".to_string()));
                                                                                }
                                                                            } else {
                                                                                set_reg_state.set(RegState::Error(
                                                                                    api_resp.error.unwrap_or_else(|| "Registration failed".to_string())
                                                                                ));
                                                                            }
                                                                        }
                                                                        Err(e) => set_reg_state.set(RegState::Error(format!("Parse error: {e}"))),
                                                                    }
                                                                }
                                                                Err(e) => set_reg_state.set(RegState::Error(format!("Read error: {e}"))),
                                                            }
                                                        }
                                                        Err(e) => set_reg_state.set(RegState::Error(format!("Network error: {e}"))),
                                                    }
                                                });
                                            }
                                        >
                                            "Reserve My Spot"
                                        </button>
                                    }
                                }
                            </div>
                        </div>
                    }.into_any()
                }
            }
        }}
    }.into_any()
}

/// Render a single dynamic form field based on its config.
fn render_dynamic_field(
    field: FormFieldConfig,
    values: ReadSignal<HashMap<String, String>>,
    set_values: WriteSignal<HashMap<String, String>>,
) -> AnyView {
    match field.field_type {
        FormFieldType::Text => render_text_field(field, values, set_values),
        FormFieldType::Textarea => render_textarea_field(field, values, set_values),
        FormFieldType::Select => render_select_field(field, values, set_values),
        FormFieldType::Multiselect => render_multiselect_field(field, values, set_values),
    }
}

fn render_text_field(
    field: FormFieldConfig,
    values: ReadSignal<HashMap<String, String>>,
    set_values: WriteSignal<HashMap<String, String>>,
) -> AnyView {
    let key = field.key.clone();
    let placeholder = field.label.clone();
    let key_for_read = key.clone();
    view! {
        <input
            type="text"
            placeholder=placeholder
            class="pe-input"
            prop:value=move || values.get().get(&key_for_read).cloned().unwrap_or_default()
            on:input=move |ev| {
                let val = event_target_value(&ev);
                set_values.update(|m| { m.insert(key.clone(), val); });
            }
        />
    }
    .into_any()
}

fn render_textarea_field(
    field: FormFieldConfig,
    values: ReadSignal<HashMap<String, String>>,
    set_values: WriteSignal<HashMap<String, String>>,
) -> AnyView {
    let key = field.key.clone();
    let placeholder = field.label.clone();
    let key_for_read = key.clone();
    view! {
        <textarea
            placeholder=placeholder
            class="pe-input"
            rows="3"
            prop:value=move || values.get().get(&key_for_read).cloned().unwrap_or_default()
            on:input=move |ev| {
                let val = event_target_value(&ev);
                set_values.update(|m| { m.insert(key.clone(), val); });
            }
        ></textarea>
    }
    .into_any()
}

fn render_select_field(
    field: FormFieldConfig,
    values: ReadSignal<HashMap<String, String>>,
    set_values: WriteSignal<HashMap<String, String>>,
) -> AnyView {
    let key = field.key.clone();
    let label = field.label.clone();
    let options = field.options.unwrap_or_default();
    let key_for_read = key.clone();
    view! {
        <div class="pe-multiselect-group">
            <span class="pe-multiselect-label">{label}</span>
            <select
                class="pe-input"
                prop:value=move || values.get().get(&key_for_read).cloned().unwrap_or_default()
                on:change=move |ev| {
                    let val = event_target_value(&ev);
                    set_values.update(|m| { m.insert(key.clone(), val); });
                }
            >
                <option value="">"Select..."</option>
                {options.iter().map(|opt| {
                    let opt = opt.clone();
                    view! { <option value=opt.clone()>{opt.clone()}</option> }
                }).collect::<Vec<_>>()}
            </select>
        </div>
    }
    .into_any()
}

fn render_multiselect_field(
    field: FormFieldConfig,
    values: ReadSignal<HashMap<String, String>>,
    set_values: WriteSignal<HashMap<String, String>>,
) -> AnyView {
    let key = field.key.clone();
    let label = field.label.clone();
    let options = field.options.unwrap_or_default();

    view! {
        <div class="pe-multiselect-group">
            <span class="pe-multiselect-label">{label}</span>
            <div class="pe-multiselect-options">
                {options.iter().map(|opt| {
                    let opt = opt.clone();
                    let field_key = key.clone();
                    let opt_for_display = opt.clone();
                    let field_key_for_change = field_key.clone();
                    let opt_for_change = opt.clone();
                    view! {
                        <label class="pe-multiselect-item">
                            <input
                                type="checkbox"
                                class="pe-checkbox"
                                checked=move || {
                                    let raw = values.get().get(&field_key).cloned().unwrap_or_default();
                                    let selected: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
                                    selected.contains(&opt)
                                }
                                on:change=move |ev| {
                                    let checked = event_target_checked(&ev);
                                    set_values.update(|m| {
                                        let raw = m.get(&field_key_for_change).cloned().unwrap_or_default();
                                        let mut selected: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
                                        if checked {
                                            if !selected.contains(&opt_for_change) {
                                                selected.push(opt_for_change.clone());
                                            }
                                        } else {
                                            selected.retain(|s| s != &opt_for_change);
                                        }
                                        m.insert(field_key_for_change.clone(), serde_json::to_string(&selected).unwrap_or_default());
                                    });
                                }
                            />
                            <span>{opt_for_display}</span>
                        </label>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }.into_any()
}

/// Extract the known profile fields (experience_level, tech_stack, interests)
/// from the dynamic field values map for backward-compatible submission.
fn extract_profile_fields(
    values: &HashMap<String, String>,
    fields: &[FormFieldConfig],
) -> (Option<String>, Option<String>, Option<String>) {
    let profile_keys: Vec<&str> = fields
        .iter()
        .filter(|f| f.profile_field)
        .map(|f| f.key.as_str())
        .collect();

    let get = |key: &str| -> Option<String> { values.get(key).cloned().filter(|v| !v.is_empty()) };

    // Map known keys to the RegisterBody fields
    let experience_level = if profile_keys.contains(&"experience_level") {
        get("experience_level")
    } else {
        None
    };

    let tech_stack = if profile_keys.contains(&"tech_stack") {
        get("tech_stack")
    } else {
        None
    };

    let interests = if profile_keys.contains(&"interests") {
        get("interests")
    } else {
        None
    };

    (experience_level, tech_stack, interests)
}
