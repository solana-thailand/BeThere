//! My Registrations — signed-in attendees see their registered events.

use leptos::prelude::*;
use leptos_router::components::A;
use serde::Deserialize;

use super::notifications::NotificationInbox;
use crate::api::ApiResponse;
use crate::components::{StatusBadge, StatusTone};
use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName};
use crate::pages::ticket::credit_chip::CreditWallet;

/// Response item from GET /api/my-registrations.
#[derive(Clone, Deserialize)]
struct MyRegistrationItem {
    event_name: String,
    event_slug: String,
    #[serde(default)]
    event_start_ms: i64,
    #[allow(dead_code)]
    attendee_id: String,
    /// Human-readable status: "registered", "deposit pending", "deposit confirmed",
    /// "checked in", "nft claimed".
    status: String,
    next_step: NextStepData,
}

#[derive(Clone, Deserialize)]
struct NextStepData {
    #[serde(rename = "type")]
    step_type: String,
    url: String,
}

/// Component that shows the user's event registrations when signed in.
/// If not signed in, renders nothing.
#[component]
pub(super) fn MyRegistrations() -> impl IntoView {
    let i18n = use_i18n();
    let (registrations, set_registrations) = signal(None::<Vec<MyRegistrationItem>>);
    let (email, set_email) = signal(None::<String>);
    let (email_verified, set_email_verified) = signal(false);

    // Check auth and fetch registrations on mount
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or_else(|_| "http://localhost:8787".to_string());

            // Check auth status
            let auth_url = format!("{origin}/api/auth/me");
            let auth_resp = match crate::api::fetch::get(&auth_url, &[]).await {
                Ok(r) => r,
                Err(_) => return,
            };

            if auth_resp.status() != 200 {
                return;
            }

            let auth_data: serde_json::Value =
                match crate::api::fetch::response_json(&auth_resp).await {
                    Ok(d) => d,
                    Err(_) => return,
                };
            let user_email = auth_data["data"]["email"]
                .as_str()
                .unwrap_or("")
                .to_string();
            if user_email.is_empty() {
                return;
            }
            set_email.set(Some(user_email));
            set_email_verified.set(
                auth_data["data"]["email_verified"]
                    .as_bool()
                    .unwrap_or(false),
            );

            // Fetch my registrations
            let regs_url = format!("{origin}/api/my-registrations");
            match crate::api::fetch::get(&regs_url, &[]).await {
                Ok(resp) if resp.status() == 200 => {
                    if let Ok(data) = crate::api::fetch::response_json::<
                        ApiResponse<Vec<MyRegistrationItem>>,
                    >(&resp)
                    .await
                    {
                        set_registrations.set(Some(data.data.unwrap_or_default()));
                    }
                }
                _ => {
                    set_registrations.set(Some(vec![]));
                }
            }
        });
    });

    move || {
        let regs = registrations.get();
        let user_email = email.get();

        match (regs, user_email) {
            (None, _) | (_, None) => ().into_any(),
            (Some(refs), Some(user)) => {
                let user_email = user.clone();
                let has_regs = !refs.is_empty();
                view! {
                    <section class="landing-reg-section">
                        // Developer Passport Card (Always shown for logged-in users)
                        <div class="landing-dev-passport">
                            <div class="landing-passport-left">
                                <div class="landing-passport-avatar">
                                    <Icon icon=IconName::Crab class="icon-lg" />
                                </div>
                                <div class="landing-passport-info">
                                    <div class="landing-passport-title-row">
                                        <span class="landing-passport-name">{user_email.clone()}</span>
                                        <span class="landing-passport-verified-badge">{t!(i18n, landing.reg.verified)}</span>
                                    </div>
                                    <div class="landing-passport-sub">
                                        {t!(i18n, landing.reg.member)}
                                    </div>
                                </div>
                            </div>
                            <div class="landing-passport-actions">
                                <A href="/profile" attr:class="btn btn-primary btn-sm landing-passport-btn">
                                    <Icon icon=IconName::Settings class="icon-sm" />
                                    " "{t!(i18n, landing.reg.edit_profile)}
                                </A>
                                <button
                                    class="btn btn-outline btn-xs"
                                    on:click=move |_| {
                                        leptos::task::spawn_local(async move {
                                            let _ = crate::api::fetch::post("/api/auth/logout", &[], None).await;
                                            let window = web_sys::window().expect("no window");
                                            let _ = window.location().reload();
                                        });
                                    }
                                >
                                    {t!(i18n, landing.reg.sign_out)}
                                </button>
                            </div>
                        </div>

                        {move || email_verified.get().then(|| view! { <NotificationInbox /> })}

                        // Deposit credit, and the way to ask for it back. The
                        // ticket page only shows this on the event the deposit
                        // was held at, so a holder whose credit has rolled on
                        // to a later event had no reachable exit (issue #120).
                        // Renders nothing when there is no credit and no lock.
                        <CreditWallet />

                        {if has_regs {
                            view! {
                                <div class="landing-reg-header" style="margin-top: 24px;">
                                    <h2 class="landing-reg-title">
                                        {t!(i18n, landing.reg.your_events)}
                                    </h2>
                                </div>
                                <div class="landing-reg-grid">
                                    {refs.into_iter().map(|reg| {
                                        let event_url = format!("/e/{}", reg.event_slug);
                                        // `step_type` is a server code; only the
                                        // label is translated.
                                        let step_type = reg.next_step.step_type.clone();
                                        let step_label = move || match step_type.as_str() {
                                            "claim" => t_string!(i18n, landing.reg.step.claim),
                                            "deposit" => t_string!(i18n, landing.reg.step.deposit),
                                            "quest" => t_string!(i18n, landing.reg.step.quest),
                                            "ticket" => t_string!(i18n, landing.reg.step.ticket),
                                            _ => t_string!(i18n, landing.reg.step.view),
                                        };
                                        // Third copy of this, and the third to
                                        // be wrong. `to_locale_string` with no
                                        // options renders the browser default —
                                        // `8/24/2026, 1:00:00 PM`: seconds on an
                                        // event date, and a month/day order that
                                        // is ambiguous to a Thai-majority
                                        // audience. Shared helper (.issues/104).
                                        // A closure so the date follows a language
                                        // switch.
                                        let start_ms = reg.event_start_ms;
                                        let date_str = move || match start_ms > 0 {
                                            true => crate::utils::format_event_datetime(start_ms),
                                            false => t_string!(i18n, landing.reg.tba).to_string(),
                                        };
                                        let next_url = reg.next_step.url.clone();
                                        let status_tone = match reg.status.as_str() {
                                            "nft claimed" | "checked in" | "deposit confirmed" => StatusTone::Confirmed,
                                            "deposit pending" => StatusTone::Pending,
                                            _ => StatusTone::Neutral,
                                        };
                                        view! {
                                            <div class="landing-reg-card">
                                                <div class="landing-reg-info">
                                                    <a href=event_url class="landing-reg-event-name">
                                                        {reg.event_name}
                                                    </a>
                                                    <p class="landing-reg-event-date">{date_str}</p>
                                                </div>
                                                <div class="landing-reg-identity">
                                                    <span class="landing-reg-identity-label">{user.clone()}</span>
                                                </div>
                                                // The status is a server code: compared
                                                // above, labelled here in the reader's
                                                // language.
                                                {
                                                    let status = reg.status.clone();
                                                    move || view! {
                                                        <StatusBadge tone=status_tone label=crate::locale::status_label(&status) />
                                                    }
                                                }
                                                <a href=next_url class="btn btn-primary btn-sm landing-reg-action">
                                                    {step_label}" →"
                                                </a>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="landing-reg-empty" style="margin-top: 16px;">
                                    <p class="landing-reg-empty-text">
                                        {t!(i18n, landing.reg.empty)}
                                    </p>
                                </div>
                            }.into_any()
                        }}
                    </section>
                }.into_any()
            }
        }
    }
}
