//! Join (.plans/043 L7): share the page, and the organizer card. The
//! organizer logic is the one `page.rs` had (ASKS-4 "the organiser line
//! stays"): an admin or organizer goes to `/admin`; anyone signed in who is
//! not one yet is asked to message us; everyone else gets the waitlist,
//! because `/admin` checks sign-in, not role, and would be a dead end.

use leptos::prelude::*;
use leptos_router::components::A;
use wasm_bindgen::JsCast;

use crate::components::is_admin_role;
use crate::i18n::td_string;

use super::auth::AuthState;
use super::hero::Markup;
use super::waitlist::WaitlistForm;

const DM_URL: &str = "https://x.com/ozoneRatchapon";

/// The native share sheet where there is one, else copy the link. Returns
/// whether the link was copied (so the button can say so).
fn share_page() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let url = window.location().origin().unwrap_or_default();
    let navigator = window.navigator();
    let share = js_sys::Reflect::get(&navigator, &"share".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
    match share {
        Some(share) => {
            let data = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&data, &"title".into(), &"BeThere".into());
            let _ = js_sys::Reflect::set(&data, &"url".into(), &url.as_str().into());
            // A cancelled sheet rejects the promise; nothing to do about it.
            let _ = share.call1(&navigator, &data);
            false
        }
        None => {
            let _ = navigator.clipboard().write_text(&url);
            true
        }
    }
}

#[component]
pub fn Join(auth_state: ReadSignal<AuthState>, user_role: ReadSignal<String>) -> impl IntoView {
    let tr = crate::locale::tr;
    let copied = RwSignal::new(false);
    let share_label = move || match copied.get() {
        true => tr(|l| td_string!(l, landing.join.copied)).get(),
        false => tr(|l| td_string!(l, landing.join.share)).get(),
    };
    view! {
        <section id="join" class="lp-join">
            <div class="lp-wrap">
                <h2 class="lp-h2">{tr(|l| td_string!(l, landing.join.title))}</h2>
                <p class="lp-lede lp-join-lede">
                    <span>{tr(|l| td_string!(l, landing.join.lede))}</span>
                    <button class="lp-btn" type="button" on:click=move |_| copied.set(share_page())>
                        {share_label}
                    </button>
                </p>
                <div class="lp-card lp-join-card">
                    <h3>{tr(|l| td_string!(l, landing.join.org_title))}</h3>
                    <p><Markup text=tr(|l| td_string!(l, landing.join.org_body)) /></p>
                    {move || {
                        let role = user_role.get();
                        match auth_state.get() {
                            AuthState::SignedIn(_) if is_admin_role(&role) || role == "organizer" => view! {
                                <A href="/admin" attr:class="lp-btn lp-btn-primary">
                                    {tr(|l| td_string!(l, landing.cta.dashboard))}
                                </A>
                            }.into_any(),
                            AuthState::SignedIn(_) => view! {
                                <p class="lp-rule">{tr(|l| td_string!(l, landing.waitlist.signed_in))}</p>
                                <a class="lp-btn" href=DM_URL target="_blank" rel="noopener noreferrer">
                                    {tr(|l| td_string!(l, landing.waitlist.dm))}
                                </a>
                            }.into_any(),
                            _ => view! {
                                <details class="lp-join-details">
                                    <summary class="lp-btn lp-btn-primary">
                                        {tr(|l| td_string!(l, landing.join.waitlist_cta))}
                                    </summary>
                                    <div class="lp-join-form"><WaitlistForm /></div>
                                </details>
                            }.into_any(),
                        }
                    }}
                </div>
            </div>
        </section>
    }
}
