//! My Registrations — signed-in attendees see their registered events.

use leptos::prelude::*;
use serde::Deserialize;

use super::notifications::NotificationInbox;
use crate::api::ApiResponse;
use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName};
use crate::pages::public::discover::DateChip;
use crate::pages::ticket::credit_chip::CreditWallet;

/// Past registrations shown before "show all": the most recent ones.
const PAST_PREVIEW: usize = 3;
/// An event with no recorded end counts as over this long after its start.
const ASSUMED_DURATION_MS: i64 = 6 * 60 * 60 * 1000;

/// Response item from GET /api/my-registrations.
#[derive(Clone, Deserialize)]
struct MyRegistrationItem {
    event_id: String,
    event_name: String,
    #[serde(default)]
    event_start_ms: i64,
    /// 0 when unknown (the KV fallback path does not carry it).
    #[serde(default)]
    event_end_ms: i64,
    attendee_id: String,
    /// Human-readable status: "registered", "deposit pending", "deposit confirmed",
    /// "checked in", "nft claimed".
    status: String,
    next_step: NextStepData,
    /// The check-in URL, present when the ticket would show its QR.
    #[serde(default)]
    qr_url: Option<String>,
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
        let signed_in = email.get().is_some();

        match (regs, signed_in) {
            (None, _) | (_, false) => ().into_any(),
            (Some(refs), true) => {
                let now = js_sys::Date::now() as i64;
                let (upcoming, mut past): (Vec<_>, Vec<_>) =
                    refs.into_iter().partition(|r| !is_past(r, now));
                // The API sorts by start ascending; past reads newest first.
                past.reverse();
                let has_regs = !upcoming.is_empty() || !past.is_empty();
                view! {
                    <section class="landing-reg-section">
                        {move || email_verified.get().then(|| view! { <NotificationInbox /> })}

                        // Deposit credit, and the way to ask for it back. The
                        // ticket page only shows this on the event the deposit
                        // was held at, so a holder whose credit has rolled on
                        // to a later event had no reachable exit (issue #120).
                        // Renders nothing when there is no credit and no lock.
                        <CreditWallet />

                        {if has_regs {
                            let upcoming_count = upcoming.len();
                            let past_count = past.len();
                            view! {
                                // One row per registration (F1-b): the row is the
                                // link to the ticket; a button only when the
                                // attendee has something to do.
                                {(upcoming_count > 0).then(|| view! {
                                    <h2 class="landing-reg-title landing-reg-group-title">
                                        {t!(i18n, landing.reg.upcoming, count = upcoming_count)}
                                    </h2>
                                    <div class="landing-reg-list">
                                        {upcoming.into_iter().map(|reg| view! { <RegistrationRow reg past=false /> }).collect::<Vec<_>>()}
                                    </div>
                                })}
                                {(past_count > 0).then(|| view! { <PastRegistrations past /> })}
                            }.into_any()
                        } else {
                            view! {
                                <div class="landing-reg-empty" style="margin-top: 16px;">
                                    <p class="landing-reg-empty-text">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.reg.empty))}
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

/// Whether a registration belongs under "past": its end has gone by, or
/// (end unknown) its start plus a typical duration has. TBA stays upcoming.
fn is_past(reg: &MyRegistrationItem, now: i64) -> bool {
    match (reg.event_end_ms, reg.event_start_ms) {
        (end, _) if end > 0 => end < now,
        (_, start) if start > 0 => start + ASSUMED_DURATION_MS < now,
        _ => false,
    }
}

/// "Past (n)": the most recent few, the rest behind one tap.
#[component]
fn PastRegistrations(past: Vec<MyRegistrationItem>) -> impl IntoView {
    let i18n = use_i18n();
    let (show_all, set_show_all) = signal(false);
    let count = past.len();
    let hidden = count.saturating_sub(PAST_PREVIEW);
    let (preview, rest): (Vec<_>, Vec<_>) = past
        .into_iter()
        .enumerate()
        .partition(|(i, _)| *i < PAST_PREVIEW);
    let rest: Vec<_> = rest.into_iter().map(|(_, r)| r).collect();
    view! {
        <h2 class="landing-reg-title landing-reg-group-title">
            {t!(i18n, landing.reg.past, count)}
        </h2>
        <div class="landing-reg-list">
            {preview.into_iter().map(|(_, reg)| view! { <RegistrationRow reg past=true /> }).collect::<Vec<_>>()}
            <Show when=move || show_all.get() fallback=|| ()>
                {rest.clone().into_iter().map(|reg| view! { <RegistrationRow reg past=true /> }).collect::<Vec<_>>()}
            </Show>
        </div>
        {(hidden > 0).then(|| view! {
            <Show when=move || !show_all.get() fallback=|| ()>
                <button class="btn btn-outline btn-sm landing-reg-more" on:click=move |_| set_show_all.set(true)>
                    {t!(i18n, landing.reg.show_all_past, count = hidden)}
                </button>
            </Show>
        })}
    }
}

/// One registration: date tile, name, status text, chevron — the row links to
/// the ticket. A full-width button follows only when there is an action.
#[component]
fn RegistrationRow(reg: MyRegistrationItem, past: bool) -> impl IntoView {
    let i18n = use_i18n();
    let ticket_url = format!("/ticket/{}?event_id={}", reg.attendee_id, reg.event_id);
    // `step_type` is a server code; only the label is translated.
    let step_type = reg.next_step.step_type.clone();
    let action = match step_type.as_str() {
        "deposit" | "claim" | "quest" => Some(reg.next_step.url.clone()),
        _ => None,
    };
    let step_label = move || match step_type.as_str() {
        "claim" => t_string!(i18n, landing.reg.step.claim),
        "deposit" => t_string!(i18n, landing.reg.step.deposit),
        _ => t_string!(i18n, landing.reg.step.quest),
    };
    let status = reg.status.clone();
    let qr = reg
        .qr_url
        .as_deref()
        .filter(|_| !past)
        .and_then(crate::utils::qr_gen::qr_svg_path);
    view! {
        <div class="landing-reg-item">
            <a class="dv-row" href=ticket_url>
                <DateChip ms=reg.event_start_ms past />
                <div class="dv-row-body">
                    <span class="dv-row-title">{reg.event_name.clone()}</span>
                    <span class="dv-row-meta">{move || crate::locale::status_label(&status)}</span>
                </div>
                <span class="landing-reg-chevron" aria-hidden="true">
                    <Icon icon=IconName::ChevronRight class="icon-sm" />
                </span>
            </a>
            {action.map(|url| view! {
                <a href=url class="btn btn-primary btn-sm btn-block landing-reg-action">{step_label}</a>
            })}
            {qr.map(|qr| view! { <InlineTicketQr qr /> })}
        </div>
    }
}

/// The ticket QR, expanded in place on the landing (.plans/038 P2-a): one tap
/// from the landing to a scannable code, drawn as SVG (no image request). The
/// full ticket page, with deposit status, badge and the fullscreen QR, stays
/// one more tap away through the card's action link.
#[component]
fn InlineTicketQr(qr: (u32, String)) -> impl IntoView {
    let (open, set_open) = signal(false);
    let (side, path) = qr;
    let view_box = format!("0 0 {side} {side}");
    view! {
        <button
            class="btn btn-outline btn-sm landing-reg-qr-toggle"
            aria-expanded=move || open.get().to_string()
            on:click=move |_| set_open.update(|o| *o = !*o)
        >
            {move || match open.get() {
                true => crate::locale::tr(|l| crate::i18n::td_string!(l, landing.reg.hide_ticket)).into_any(),
                false => crate::locale::tr(|l| crate::i18n::td_string!(l, landing.reg.show_ticket)).into_any(),
            }}
        </button>
        <Show when=move || open.get() fallback=|| ()>
            <div class="landing-reg-qr">
                <svg
                    class="landing-reg-qr-svg"
                    viewBox=view_box.clone()
                    role="img"
                    aria-label=crate::locale::tr(|l| crate::i18n::td_string!(l, landing.reg.qr_alt))
                    shape-rendering="crispEdges"
                >
                    <rect width="100%" height="100%" fill="#fff" />
                    <path d=path.clone() fill="#000" />
                </svg>
                <p class="landing-reg-qr-hint">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.reg.qr_hint))}
                </p>
            </div>
        </Show>
    }
}
