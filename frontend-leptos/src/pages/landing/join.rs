//! The organizer card (.plans/043 L7; on `/organizers` since .plans/045,
//! beside the planning tile, as in the prototype). The logic is the one
//! `page.rs` had (ASKS-4 "the organiser line stays"): an admin or organizer
//! goes to `/admin`; anyone signed in who is not one yet is asked to message
//! us; everyone else gets the waitlist, because `/admin` checks sign-in, not
//! role, and would be a dead end.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::is_admin_role;
use crate::i18n::td_string;

use super::auth::AuthState;
use super::hero::Markup;
use super::waitlist::WaitlistForm;

const DM_URL: &str = "https://x.com/ozoneRatchapon";

#[component]
pub fn OrganizerCard(
    auth_state: ReadSignal<AuthState>,
    user_role: ReadSignal<String>,
) -> impl IntoView {
    let tr = crate::locale::tr;
    view! {
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
            <p class="lp-plan-fine">{tr(|l| td_string!(l, landing.site.org_now))}</p>
        </div>
    }
}
