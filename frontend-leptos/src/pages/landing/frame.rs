//! The chrome every site page shares (.plans/045 R4.0): the light header with
//! page links, the doors at the foot, and the footer. It was inline in
//! `page.rs`; the landing, `/organizers` and `/sponsors` now wrap their
//! sections in [`SiteFrame`].

use leptos::prelude::*;
use leptos_router::components::A;

use crate::pages::site::doors::{Doors, SitePage};

use super::auth::AuthState;
use super::header::LandingHeader;
use super::stats::provide_landing_stats;
use super::theme::initial_theme;

/// The session as the header and the hero see it: who is signed in, and
/// their role. Starts the `/api/auth/me` check once.
pub fn use_site_auth() -> (ReadSignal<AuthState>, ReadSignal<String>) {
    let (auth_state, set_auth_state) = signal(AuthState::Checking);
    let (user_role, set_user_role) = signal(String::new());
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or_else(|_| "http://localhost:8787".to_string());
            let url = format!("{origin}/api/auth/me");
            match crate::api::fetch::get(&url, &[]).await {
                Ok(resp) if resp.status() == 200 => {
                    if let Ok(data) =
                        crate::api::fetch::response_json::<serde_json::Value>(&resp).await
                    {
                        let email = data["data"]["email"].as_str().unwrap_or("").to_string();
                        let role = data["data"]["role"]
                            .as_str()
                            .unwrap_or("attendee")
                            .to_string();
                        if !email.is_empty() {
                            log::info!("[landing] user signed in: {email} ({role})");
                            set_auth_state.set(AuthState::SignedIn(email));
                            set_user_role.set(role);
                        } else {
                            set_auth_state.set(AuthState::NotSignedIn);
                        }
                    } else {
                        set_auth_state.set(AuthState::NotSignedIn);
                    }
                }
                _ => {
                    set_auth_state.set(AuthState::NotSignedIn);
                }
            }
        });
    });
    (auth_state, user_role)
}

/// Header, the page's sections, the doors, the footer.
#[component]
pub fn SiteFrame(
    here: SitePage,
    auth_state: ReadSignal<AuthState>,
    /// The page places the doors itself (the home, right under the hero).
    #[prop(optional)]
    own_doors: bool,
    children: Children,
) -> impl IntoView {
    let theme = RwSignal::new(initial_theme());
    // One stats fetch for every section that shows a number (rule 1).
    provide_landing_stats();
    scroll_to_hash_after_mount();
    view! {
        <div class="landing-page lp" data-theme=move || theme.get().as_str()>
            <LandingHeader auth_state=auth_state theme=theme />
            {children()}
            {(!own_doors).then(|| view! { <Doors here=here /> })}
            <SiteFooter />
        </div>
    }
}

/// One row of links, one of fine print (.plans/043).
#[component]
fn SiteFooter() -> impl IntoView {
    view! {
        <footer class="lp-footer">
            <div class="lp-wrap">
                <div class="lp-frow">
                    <a class="lp-flogo" href="/">"BeThere"</a>
                    <nav class="lp-flinks">
                        // The swimlane lives on /organizers.
                        <A href="/organizers#how">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.how))}</A>
                        <a href="/faq">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.faq))}</a>
                        <A href="/login">{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.staff_portal))}</A>
                        <a href="https://discord.gg/PGbUgNmsns" target="_blank" rel="noopener noreferrer">"Discord"</a>
                        <a href="https://x.com/ozoneRatchapon" target="_blank" rel="noopener noreferrer">"X"</a>
                        <a href="https://github.com/solana-thailand/BeThere" target="_blank" rel="noopener noreferrer">"GitHub"</a>
                    </nav>
                </div>
                <div class="lp-fine">
                    <span>
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.copyright))}
                        " · "
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.partner))}
                    </span>
                    // A status line, not a promise: real deposits are THB the
                    // organizer holds; the escrow runs on devnet only (0.2).
                    <span>{crate::locale::tr(|l| crate::i18n::td_string!(l, landing.footer.deposit_status))}</span>
                    // Which build is live (.plans/038 P2-f): set by build.sh,
                    // absent from builds that do not know their commit.
                    {option_env!("BETHERE_GIT_SHA").filter(|sha| !sha.is_empty()).map(|sha| view! {
                        <span class="lp-version">{format!("v{} · {sha}", env!("CARGO_PKG_VERSION"))}</span>
                    })}
                </div>
            </div>
        </footer>
    }
}

/// A link from another page (`/organizers#how` in the hero) is followed by
/// the router, which renders the new page before it writes the URL to the
/// address bar and then scrolls: to the `#id` if it can find it then, else
/// to the top, which is where it lands. So the hash is read from the
/// router's location (already the new one) and, one task later, after the
/// router's own scroll, the `#id` is brought into view.
fn scroll_to_hash_after_mount() {
    let hash = leptos_router::hooks::use_location().hash.get_untracked();
    let Some(id) = hash
        .strip_prefix('#')
        .filter(|id| !id.is_empty())
        .map(str::to_string)
    else {
        return;
    };
    Effect::new(move |_| {
        let id = id.clone();
        let scroll = wasm_bindgen::closure::Closure::once_into_js(move || {
            if let Some(el) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id(&id))
            {
                el.scroll_into_view();
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback(wasm_bindgen::JsCast::unchecked_ref(&scroll));
        }
    });
}
