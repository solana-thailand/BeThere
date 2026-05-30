use super::types::*;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn registration_form(
    slug_for_reg: String,
    locked_email: String,
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
    _reg_email: ReadSignal<String>,
    set_reg_email: WriteSignal<String>,
    reg_participation: ReadSignal<String>,
    set_reg_participation: WriteSignal<String>,
    reg_contact_channel: ReadSignal<String>,
    set_reg_contact_channel: WriteSignal<String>,
    reg_contact_handle: ReadSignal<String>,
    set_reg_contact_handle: WriteSignal<String>,
    reg_deposit_agreed: ReadSignal<bool>,
    set_reg_deposit_agreed: WriteSignal<bool>,
    reg_state: ReadSignal<RegState>,
    set_reg_state: WriteSignal<RegState>,
) -> AnyView {
    // Pre-fill email from JWT
    set_reg_email.set(locked_email.clone());

    let (field_errors, set_field_errors) = signal(FieldErrors::default());

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

                    let redirect_url = next_url.clone();
                    leptos::task::spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(800).await;
                        navigateTo(&redirect_url);
                    });

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
                                <p class="pe-detail-secondary">"Redirecting..."</p>
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
                    let email_display = locked_email.clone();
                    let email_for_display = locked_email.clone();
                    let email_for_submit = locked_email.clone();
                    view! {
                        <div class="pe-card">
                            <h2 class="pe-section-title">
                                <Icon icon=IconName::Ticket class="icon-md" />" Reserve Your Spot"
                            </h2>
                            // Signed-in indicator
                            <div class="pe-signed-in-row">
                                <span class="pe-checkmark">"✓"</span>
                                <span class="pe-detail-secondary">
                                    {format!("Signed in as {email_display}")}
                                </span>
                            </div>
                            <div class="pe-flex-col-gap-md">
                                // Name
                                <div class="pe-field" id="pe-field-name">
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
                                // Email — locked
                                <input
                                    type="email"
                                    value=email_for_display
                                    readonly
                                    class="pe-input pe-input--locked"
                                />
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
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }
                                }}
                                // Contact Channel
                                <div class="pe-field" id="pe-field-channel">
                                    <label class="pe-label">
                                        "Preferred Contact Channel / ช่องทางที่สะดวกให้ทีมงานติดต่อกลับเพื่อยืนยันสิทธิ์ (Confirm Seat)"
                                        {if require_contact {
                                            view! { <span class="pe-required">" *"</span> }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </label>
                                    <select
                                        class="pe-input"
                                        prop:class=move || if field_errors.get().contact_channel.is_some() { "pe-input--error" } else { "" }
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
                                    <input
                                        type="text"
                                        placeholder="Username or profile link / โปรดระบุ Username หรือลิงก์โปรไฟล์"
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
                                // Deposit Agreement
                                {move || {
                                    let is_online_track = is_hybrid && reg_participation.get().to_lowercase().contains("online");
                                    if has_deposit && !is_online_track {
                                        let dep_label = dep_label.clone();
                                        view! {
                                            <div id="pe-field-deposit">
                                            <label class="pe-checkbox-label">
                                                <input
                                                    type="checkbox"
                                                    class="pe-checkbox"
                                                    checked=move || reg_deposit_agreed.get()
                                                    on:change=move |ev| {
                                                        set_reg_deposit_agreed.set(event_target_checked(&ev));
                                                        set_field_errors.update(|e| e.deposit_agreed = None);
                                                    }
                                                />
                                                <span>{format!("ยอมรับการจ่ายมัดจำ {} (จะได้รับคืนภายในงาน) / I agree to pay a {} commitment deposit to secure my seat and understand I will receive a refund upon check-in at the venue.", dep_label, dep_label)}</span>
                                            </label>
                                            {move || match &field_errors.get().deposit_agreed {
                                                Some(err) => view! { <span class="pe-field-error pe-field-error-indent">{err.clone()}</span> }.into_any(),
                                                None => view! { <div></div> }.into_any(),
                                            }}
                                            </div>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }
                                }}
                                // Submit button
                                {
                                    let slug = slug.clone();
                                    let email_sub = email_for_submit.clone();
                                    view! {
                                        <button
                                            class="pe-submit-btn"
                                            on:click=move |_| {
                                                let name_val = reg_name.get();
                                                let part_val = reg_participation.get();
                                                let channel_val = reg_contact_channel.get();
                                                let handle_val = reg_contact_handle.get();
                                                let deposit_val = reg_deposit_agreed.get();
                                                let email_val = email_sub.clone();

                                                let mut errors = FieldErrors::default();
                                                if name_val.trim().is_empty() {
                                                    errors.name = Some("Name is required".to_string());
                                                }
                                                if require_contact && channel_val.trim().is_empty() {
                                                    errors.contact_channel = Some("Please select a channel".to_string());
                                                }
                                                if require_contact && handle_val.trim().is_empty() {
                                                    errors.contact_handle = Some("Please provide your contact info".to_string());
                                                }
                                                let is_online_track = is_hybrid && part_val.to_lowercase().contains("online");
                                                if has_deposit && !is_online_track && !deposit_val {
                                                    errors.deposit_agreed = Some("You must agree to the deposit".to_string());
                                                }

                                                let has_errors = errors.name.is_some()
                                                    || errors.contact_channel.is_some()
                                                    || errors.contact_handle.is_some()
                                                    || errors.deposit_agreed.is_some();

                                                // Determine scroll target before moving errors
                                                let scroll_target = errors.name.as_ref()
                                                    .map(|_| "pe-field-name")
                                                    .or(errors.contact_channel.as_ref().map(|_| "pe-field-channel"))
                                                    .or(errors.contact_handle.as_ref().map(|_| "pe-field-handle"))
                                                    .or(errors.deposit_agreed.as_ref().map(|_| "pe-field-deposit"));

                                                set_field_errors.set(errors);

                                                if has_errors {
                                                    if let Some(id) = scroll_target {
                                                        scroll_to_element(&id);
                                                    }
                                                    return;
                                                }

                                                set_reg_state.set(RegState::Submitting);
                                                let body = RegisterBody {
                                                    slug: slug.clone(),
                                                    name: name_val.trim().to_string(),
                                                    email: email_val.trim().to_lowercase(),
                                                    participation_type: if part_val.is_empty() { None } else { Some(part_val.clone()) },
                                                    contact_channel: if channel_val.trim().is_empty() { None } else { Some(channel_val.trim().to_string()) },
                                                    contact_handle: if handle_val.trim().is_empty() { None } else { Some(handle_val.trim().to_string()) },
                                                    deposit_agreed: if deposit_val { Some(true) } else { None },
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
