//! Light (paper) or dark (the site's navy) for the landing page (ASKS-4 §11,
//! §17). It follows the system setting until the visitor picks, then
//! remembers the pick. Only the landing root carries `data-theme`; the rest
//! of the app keeps its own palette until build plan 1.9.

use leptos::prelude::*;

/// localStorage key for an explicit pick; absent = follow the system.
const STORAGE_KEY: &str = "bethere.theme";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn parse(value: &str) -> Option<Theme> {
        match value {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        }
    }

    pub fn toggled(self) -> Theme {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }

    /// The stored pick wins; without one, the system setting.
    pub fn resolve(stored: Option<Theme>, system_dark: bool) -> Theme {
        match (stored, system_dark) {
            (Some(pick), _) => pick,
            (None, true) => Theme::Dark,
            (None, false) => Theme::Light,
        }
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn system_dark() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok().flatten())
        .is_some_and(|q| q.matches())
}

/// The theme to render with on first paint.
pub fn initial_theme() -> Theme {
    let stored = storage()
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
        .and_then(|v| Theme::parse(&v));
    Theme::resolve(stored, system_dark())
}

/// The nav button: flips the theme and stores the pick.
#[component]
pub fn ThemeToggle(theme: RwSignal<Theme>) -> impl IntoView {
    let on_click = move |_| {
        let next = theme.get_untracked().toggled();
        theme.set(next);
        if let Some(s) = storage() {
            let _ = s.set_item(STORAGE_KEY, next.as_str());
        }
    };
    view! {
        <button
            class="lp-theme"
            type="button"
            on:click=on_click
            aria-label=crate::locale::tr(|l| crate::i18n::td_string!(l, landing.nav.theme))
            aria-pressed=move || (theme.get() == Theme::Dark).to_string()
        >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <circle cx="12" cy="12" r="8" />
                <path d="M12 4a8 8 0 0 1 0 16z" fill="currentColor" />
            </svg>
        </button>
    }
}
