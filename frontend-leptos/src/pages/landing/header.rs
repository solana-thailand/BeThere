//! The site header and the landing's side index (`bethere-ux/site`, nav and
//! `nav.side`): a light bar with the wordmark, the page links (.plans/045
//! R4.0), the language and theme switches inline, and sign-in on the right.
//! On wide screens the landing adds a column of dots marking where you are.
//!
//! `/discover` and `/feedback` keep `SiteHeader` (app palette, hamburger).
//! The global language bar is not drawn on framed pages
//! (`locale::shows_lang_bar`); the switch lives here instead.

use leptos::prelude::*;
use leptos_router::components::A;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::i18n::{Locale, td_string};
use crate::locale::{LanguageSwitch, tr};
use crate::pages::site::doors::SitePage;

use super::auth::{AuthState, trigger_landing_oauth, trigger_landing_signout};
use super::theme::{Theme, ThemeToggle};

/// A section the header or the side index can jump to.
pub struct Section {
    /// The element id on the landing page, without `#`.
    pub id: &'static str,
    pub label: fn(Locale) -> &'static str,
}

/// The side index, in page order.
pub const SIDE_SECTIONS: [Section; 7] = [
    Section {
        id: "top",
        label: |l| td_string!(l, landing.nav.top),
    },
    Section {
        id: "events",
        label: |l| td_string!(l, landing.nav.events),
    },
    Section {
        id: "story",
        label: |l| td_string!(l, landing.nav.story),
    },
    Section {
        id: "how",
        label: |l| td_string!(l, landing.nav.how),
    },
    Section {
        id: "goal",
        label: |l| td_string!(l, landing.nav.so_far_goal),
    },
    Section {
        id: "sponsors",
        label: |l| td_string!(l, landing.nav.sponsors),
    },
    Section {
        id: "join",
        label: |l| td_string!(l, landing.nav.join),
    },
];

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

/// Dots on the right edge (wide screens only, CSS). The one for the section
/// in the middle of the viewport is filled.
#[component]
pub fn SideIndex() -> impl IntoView {
    let active = RwSignal::new("top");
    watch_sections(active);
    let dots = SIDE_SECTIONS
        .iter()
        .map(|s| {
            let id = s.id;
            view! {
                <a
                    href=format!("#{id}")
                    class:on=move || active.get() == id
                    aria-current=move || (active.get() == id).then_some("true")
                    data-label=tr(s.label)
                >
                    {tr(s.label)}
                </a>
            }
        })
        .collect::<Vec<_>>();
    view! {
        <nav class="lp-side" aria-label=tr(|l| td_string!(l, landing.nav.sections))>{dots}</nav>
    }
}

/// Marks the section crossing the middle band of the viewport as active.
fn watch_sections(active: RwSignal<&'static str>) {
    let observer = StoredValue::new_local(None::<web_sys::IntersectionObserver>);
    // After mount, so every section is in the document.
    Effect::new(move |_| {
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let callback = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
            let hit = entries
                .iter()
                .filter_map(|e| e.dyn_into::<web_sys::IntersectionObserverEntry>().ok())
                .find(|e| e.is_intersecting())
                .map(|e| e.target().id());
            if let Some(id) = hit
                && let Some(s) = SIDE_SECTIONS.iter().find(|s| s.id == id)
            {
                active.set(s.id);
            }
        });
        let init = web_sys::IntersectionObserverInit::new();
        init.set_root_margin("-45% 0px -50% 0px");
        let Ok(obs) = web_sys::IntersectionObserver::new_with_options(
            callback.as_ref().unchecked_ref(),
            &init,
        ) else {
            return;
        };
        SIDE_SECTIONS
            .iter()
            .filter_map(|s| doc.get_element_by_id(s.id))
            .for_each(|el| obs.observe(&el));
        observer.set_value(Some(obs));
        // One closure per landing mount; on_cleanup disconnects the observer.
        callback.forget();
    });
    on_cleanup(move || {
        observer.with_value(|o| {
            if let Some(o) = o {
                o.disconnect();
            }
        });
    });
}
