use super::types::*;
use crate::i18n::{Locale, t, t_string, td_string, use_i18n};
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
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
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
    require_photo_consent: bool,
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
    // Bot check (.issues/170): only signed-in visitors who can register see
    // this form, so it starts on render rather than on first touch.
    let bot_check = crate::bot_check::BotCheck::new();
    bot_check.activate();
    if !wallet_only {
        set_reg_email.set(locked_email.clone());
    }

    let i18n = use_i18n();
    let (field_errors, set_field_errors) = signal(FieldErrors::default());
    // One field's inline error, in the current language.
    let field_error = move |pick: fn(&FieldErrors) -> Option<FieldMsg>, class: &'static str| {
        move || match pick(&field_errors.get()) {
            Some(msg) => view! { <span class=class>{msg(i18n.get_locale())}</span> }.into_any(),
            None => view! { <div></div> }.into_any(),
        }
    };

    // Resolve form config: use provided config or defaults
    let resolved_config = match form_config {
        Some(cfg) => cfg.clone(),
        None => RegistrationFormConfig::default(),
    };
    let section_label = Arc::new(resolved_config.section_label.clone());
    let form_fields = Arc::new(resolved_config.fields.clone());

    // Auto-fill from localStorage for returning users
    if let Some(saved_json) = loadDevProfile()
        && let Ok(profile) = serde_json::from_str::<SavedDevProfile>(&saved_json)
    {
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

                    // Wallet possession does not prove ownership of the typed email.
                    // Pause with the verified linking path instead of auto-redirecting.
                    let wallet_not_linked = matches!(data.wallet_linked, Some(false));

                    let redirect_url = next_url.clone();
                    if !wallet_not_linked {
                        leptos::task::spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(800).await;
                            navigateTo(&redirect_url);
                        });
                    }

                    let continue_url = next_url.clone();
                    let name = data.name.clone();
                    view! {
                        <div class="pe-card">
                            <div class="pe-text-center">
                                <div class="pe-success-icon-lg">
                                    <Icon icon=IconName::Check class="icon-2xl icon-success" />
                                </div>
                                <h2 class="pe-section-title pe-title-success">
                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, event.registered_title))}
                                </h2>
                                <p class="pe-detail-secondary pe-mb-1">
                                    {t!(i18n, event.welcome, name)}
                                </p>
                                {if wallet_not_linked {
                                    view! {
                                        <div style="background:rgba(153,69,255,0.08);border:1px solid rgba(153,69,255,0.25);border-radius:8px;padding:10px 12px;margin:12px 0;font-size:0.82rem;line-height:1.45;color:#cbd5e1;text-align:left;">
                                            <strong style="color:#fff;">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.heads_up))}" "</strong>
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, event.wallet_not_linked))}
                                        </div>
                                        <button class="pe-submit-btn" on:click=move |_| navigateTo(&continue_url)>
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, event.continue_cta))}
                                        </button>
                                    }.into_any()
                                } else {
                                    view! { <p class="pe-detail-secondary">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.redirecting))}</p> }.into_any()
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
                                <Icon icon=IconName::Ticket class="icon-md" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.reserve_title))}
                            </h2>
                            <div class="pe-error-box">
                                {msg_clone}
                            </div>
                            <button
                                class="btn btn-outline btn-block"
                                on:click=move |_| set_reg_state.set(RegState::Idle)
                            >
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, event.try_again))}
                            </button>
                        </div>
                    }.into_any()
                }
                RegState::Submitting => {
                    view! {
                        <div class="pe-card pe-text-center">
                            <div class="pe-icon-mb-sm"><Icon icon=IconName::Hourglass class="icon-md" /></div>
                            <p class="pe-detail-secondary">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.registering))}</p>
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
                                <Icon icon=IconName::Ticket class="icon-md" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.reserve_title))}
                            </h2>
                            <div class="pe-flex-col-gap-md">
                                // Name
                                <div class="pe-field" id="pe-field-name">
                                    <label class="pe-field-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.field_name))}<span class="pe-required">" *"</span></label>
                                    <input
                                        type="text"
                                        placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, event.field_name_placeholder))
                                        class="pe-input"
                                        prop:class=move || if field_errors.get().name.is_some() { "pe-input--error" } else { "" }
                                        prop:value=move || reg_name.get()
                                        on:input=move |ev| {
                                            set_reg_name.set(event_target_value(&ev));
                                            set_field_errors.update(|e| e.name = None);
                                        }
                                    />
                                    {field_error(|e| e.name, "pe-field-error")}
                                </div>
                                // Email — locked for Google sessions; editable + required for wallet-only
                                <div class="pe-field" id="pe-field-email">
                                    <label class="pe-field-label">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.field_email))}
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
                                            <span class="pe-field-hint">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.field_email_hint))}</span>
                                            {field_error(|e| e.email, "pe-field-error")}
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
                                        // The option values ("In-Person", "Online") go to the
                                        // server and stay English; only the labels translate.
                                        let ip_label = match in_person_remaining {
                                            Some(count) => view! { {t!(i18n, event.track_in_person_left, count)} }.into_any(),
                                            None => view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, event.track_in_person))} }.into_any(),
                                        };
                                        let on_label = match online_remaining {
                                            Some(count) => view! { {t!(i18n, event.track_online_left, count)} }.into_any(),
                                            None => view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, event.track_online))} }.into_any(),
                                        };
                                        view! {
                                            <div class="pe-field">
                                                <label class="pe-field-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.track_label))}</label>
                                                <select
                                                    class="pe-input"
                                                    on:change=move |ev| set_reg_participation.set(event_target_value(&ev))
                                                >
                                                    <option value="">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.track_placeholder))}</option>
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
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.channel_label))}
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
                                        <option value="">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.channel_placeholder))}</option>
                                        <option value="Telegram">"Telegram"</option>
                                        <option value="Line">"Line"</option>
                                        <option value="Facebook">"Facebook"</option>
                                        <option value="X (Twitter)">"X (Twitter)"</option>
                                    </select>
                                    {field_error(|e| e.contact_channel, "pe-field-error")}
                                </div>
                                // Contact Handle
                                <div class="pe-field" id="pe-field-handle">
                                    <label class="pe-field-label">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, event.handle_label))}
                                        {if require_contact {
                                            view! { <span class="pe-required">" *"</span> }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </label>
                                    <input
                                        type="text"
                                        placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, event.handle_placeholder))
                                        class="pe-input"
                                        prop:class=move || if field_errors.get().contact_handle.is_some() { "pe-input--error" } else { "" }
                                        prop:value=move || reg_contact_handle.get()
                                        on:input=move |ev| {
                                            set_reg_contact_handle.set(event_target_value(&ev));
                                            set_field_errors.update(|e| e.contact_handle = None);
                                        }
                                    />
                                    {field_error(|e| e.contact_handle, "pe-field-error")}
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

                                // Registration consent (privacy + deposit). Photo and
                                // marketing consent are separate boxes below (.issues/161).
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
                                                        set_field_errors.update(|e| {
                                                            e.consent_given = None;
                                                            e.deposit_agreed = None;
                                                        });
                                                    }
                                                />
                                                <span>
                                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, event.consent_agree))}
                                                    <a href="/privacy" target="_blank" class="pe-ext-link">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.privacy_policy))}</a>
                                                    {if show_deposit {
                                                        // `deposit_consent_label` names the currency (.issues/166).
                                                        let amount = dep_label;
                                                        view! { {t!(i18n, event.consent_deposit, amount)} }.into_any()
                                                    } else {
                                                        view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, event.consent_no_deposit))} }.into_any()
                                                    }}
                                                </span>
                                            </label>
                                            {field_error(|e| e.consent_given.or(e.deposit_agreed), "pe-field-error pe-field-error-indent")}
                                        </div>
                                    }.into_any()
                                }}
                                // Photo consent: optional unless the event requires it
                                <div id="pe-field-photo-consent">
                                    <label class="pe-checkbox-label">
                                        <input
                                            type="checkbox"
                                            class="pe-checkbox"
                                            checked=move || reg_photo_consent_given.get()
                                            on:change=move |ev| {
                                                set_reg_photo_consent_given.set(event_target_checked(&ev));
                                                set_field_errors.update(|e| e.photo_consent_given = None);
                                            }
                                        />
                                        <span>
                                            {move || match require_photo_consent {
                                                true => t_string!(i18n, event.photo_consent_required),
                                                false => t_string!(i18n, event.photo_consent_optional),
                                            }}
                                        </span>
                                    </label>
                                    {field_error(|e| e.photo_consent_given, "pe-field-error pe-field-error-indent")}
                                </div>
                                // Marketing consent: always optional, never a condition of registering
                                <div id="pe-field-marketing-consent">
                                    <label class="pe-checkbox-label">
                                        <input
                                            type="checkbox"
                                            class="pe-checkbox"
                                            checked=move || reg_consent_marketing.get()
                                            on:change=move |ev| set_reg_consent_marketing.set(event_target_checked(&ev))
                                        />
                                        <span>{crate::locale::tr(|l| crate::i18n::td_string!(l, event.marketing_consent))}</span>
                                    </label>
                                </div>
                                <crate::bot_check::BotCheckSlot check=bot_check />
                                // Submit button
                                {
                                    let slug = slug.clone();
                                    let email_sub = email_for_submit.clone();
                                    let submit_fields = form_fields.as_ref().clone();
                                    view! {
                                        <button
                                            class="pe-submit-btn"
                                            disabled=move || !bot_check.ready()
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
                                                    errors.name = Some(|l| td_string!(l, event.err_name_required));
                                                }
                                                if wallet_only && !email_looks_valid(email_val.trim()) {
                                                    errors.email = Some(|l| td_string!(l, event.err_email_invalid));
                                                }
                                                if require_contact && channel_val.trim().is_empty() {
                                                    errors.contact_channel = Some(|l| td_string!(l, event.err_channel_required));
                                                }
                                                if require_contact && handle_val.trim().is_empty() {
                                                    errors.contact_handle = Some(|l| td_string!(l, event.err_handle_required));
                                                }
                                                let consent_val = reg_consent_given.get();
                                                if !consent_val {
                                                    errors.consent_given = Some(|l| td_string!(l, event.err_consent_required));
                                                }
                                                let photo_consent_val = reg_photo_consent_given.get();
                                                if require_photo_consent && !photo_consent_val {
                                                    errors.photo_consent_given = Some(|l| td_string!(l, event.err_photo_consent_required));
                                                }
                                                let is_online_track = is_hybrid && part_val.to_lowercase().contains("online");
                                                if has_deposit && !is_online_track && !deposit_val {
                                                    errors.deposit_agreed = Some(|l| td_string!(l, event.err_deposit_required));
                                                }

                                                let has_errors = errors.name.is_some()
                                                    || errors.email.is_some()
                                                    || errors.contact_channel.is_some()
                                                    || errors.contact_handle.is_some()
                                                    || errors.consent_given.is_some()
                                                    || errors.deposit_agreed.is_some()
                                                    || errors.photo_consent_given.is_some();

                                                // Determine scroll target before moving errors
                                                let scroll_target = errors.name.as_ref()
                                                    .map(|_| "pe-field-name")
                                                    .or(errors.email.as_ref().map(|_| "pe-field-email"))
                                                    .or(errors.contact_channel.as_ref().map(|_| "pe-field-channel"))
                                                    .or(errors.contact_handle.as_ref().map(|_| "pe-field-handle"))
                                                    .or(errors.consent_given.as_ref().map(|_| "pe-field-consent"))
                                                    .or(errors.deposit_agreed.as_ref().map(|_| "pe-field-deposit"))
                                                    .or(errors.photo_consent_given.as_ref().map(|_| "pe-field-photo-consent"));

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
                                                    consent_marketing: Some(reg_consent_marketing.get()),
                                                    experience_level,
                                                    tech_stack,
                                                    interests,
                                                    profile_fields: if profile_fields.is_empty() { None } else { Some(profile_fields) },
                                                };

                                                // Client-side messages are rendered in the language
                                                // current at submit time; server errors pass through.
                                                let locale: Locale = i18n.get_locale_untracked();
                                                leptos::task::spawn_local(async move {
                                                    let window = web_sys::window().expect("no window");
                                                    let origin = window.location().origin().unwrap_or_else(|_| "http://localhost:8787".to_string());
                                                    let url = format!("{origin}/api/public/register");
                                                    let fail = |msg: &str| RegState::Error(msg.to_string());
                                                    let fail_with = |msg: &str, e: &dyn std::fmt::Display| RegState::Error(format!("{msg}: {e}"));

                                                    let token_header = bot_check.header();
                                                    let mut hdrs = vec![("Content-Type", "application/json")];
                                                    if let Some((name, token)) = token_header.as_ref() {
                                                        hdrs.push((name, token.as_str()));
                                                    }
                                                    let result = crate::api::fetch::post(&url, &hdrs, Some(serde_json::to_string(&body).unwrap_or_default())).await;
                                                    // The token is spent whatever the answer was.
                                                    bot_check.reset();
                                                    match result
                                                    {
                                                        Ok(resp) => {
                                                            if resp.status() == 401 {
                                                                set_reg_state.set(fail(td_string!(locale, event.err_session_expired)));
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
                                                                                    set_reg_state.set(fail(td_string!(locale, event.err_no_data)));
                                                                                }
                                                                            } else {
                                                                                set_reg_state.set(RegState::Error(match api_resp.error {
                                                                                    Some(msg) if event_checkin_domain::turnstile::is_rejection(&msg) => {
                                                                                        td_string!(locale, event.err_bot_check).to_string()
                                                                                    }
                                                                                    Some(msg) => msg,
                                                                                    None => td_string!(locale, event.err_registration_failed).to_string(),
                                                                                }));
                                                                            }
                                                                        }
                                                                        Err(e) => set_reg_state.set(fail_with(td_string!(locale, event.err_parse), &e)),
                                                                    }
                                                                }
                                                                Err(e) => set_reg_state.set(fail_with(td_string!(locale, event.err_read), &e)),
                                                            }
                                                        }
                                                        Err(e) => set_reg_state.set(fail_with(td_string!(locale, event.err_network), &e)),
                                                    }
                                                });
                                            }
                                        >
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, event.submit))}
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
                <option value="">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.select_placeholder))}</option>
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
