//! The site header (`bethere-ux/site`, nav): a light bar with the wordmark,
//! the page links (.plans/045 R4.0), the language and theme switches inline,
//! and sign-in on the right. (The landing's side index of dots went with the
//! home trim of .plans/045: three sections need no index.)
//!
//! `/discover` and `/feedback` keep `SiteHeader` (app palette, hamburger).
//! The global language bar is not drawn on framed pages
//! (`locale::shows_lang_bar`); the switch lives here instead.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::i18n::td_string;
use crate::locale::{LanguageSwitch, tr};
use crate::pages::site::doors::SitePage;

use super::auth::{AuthState, trigger_landing_oauth, trigger_landing_signout};
use super::theme::{Theme, ThemeToggle};

#[component]
pub fn LandingHeader(auth_state: ReadSignal<AuthState>, theme: RwSignal<Theme>) -> impl IntoView {
    let nav_label = tr(|l| td_string!(l, landing.site.nav_label));
    // `A` marks the current page with aria-current="page".
    let links = SitePage::ALL
        .into_iter()
        .filter_map(|page| page.nav_label().map(|label| (page, label)))
        .map(|(page, label)| view! { <A href=page.path()>{tr(label)}</A> })
        .collect::<Vec<_>>();
    view! {
        <nav class="lp-nav" aria-label=move || nav_label.get()>
            <div class="lp-wrap lp-nav-row">
                <a class="lp-logo" href="/">"BeThere"</a>
                <div class="lp-nav-links">{links}</div>
                <div class="lp-lang"><LanguageSwitch /></div>
                <ThemeToggle theme=theme />
                {move || match auth_state.get() {
                    AuthState::NotSignedIn => view! {
                        <button class="lp-btn lp-nav-btn" type="button" on:click=move |_| trigger_landing_oauth()>
                            {tr(|l| td_string!(l, landing.nav.sign_in))}
                        </button>
                    }.into_any(),
                    AuthState::SignedIn(email) => {
                        let initial = email.chars().next().unwrap_or('?').to_uppercase().to_string();
                        view! {
                            <A href="/profile" attr:class="lp-btn lp-nav-btn lp-nav-me" attr:title=email>
                                {initial}
                            </A>
                            <button class="lp-btn lp-nav-btn lp-nav-out" type="button" on:click=move |_| trigger_landing_signout()>
                                {tr(|l| td_string!(l, landing.nav.sign_out))}
                            </button>
                        }.into_any()
                    }
                    AuthState::Checking => ().into_any(),
                }}
            </div>
        </nav>
    }
}
