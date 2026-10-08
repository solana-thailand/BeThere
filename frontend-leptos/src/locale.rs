//! Attendee-page language: EN + TH (.plans/037 §2).
//!
//! The catalog is `locales/{en,th}.json`, compiled into `crate::i18n` by
//! `leptos_i18n::load_locales!()`; keys are checked at compile time by `t!`.
//!
//! Choice order: the attendee's explicit pick (localStorage) → the browser's
//! `navigator.languages` (`th*` → TH, anything else → EN, done by
//! `leptos_i18n`) → EN. Only an explicit pick is stored, so a visitor who
//! never touches the switch keeps following their browser.
//!
//! Staff, admin and scanner pages stay English and do not render the switch.

use leptos::prelude::*;

use crate::i18n::{Locale, t_string, use_i18n};

/// localStorage key for an explicit language pick.
const STORAGE_KEY: &str = "bethere.lang";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// The stored pick, if the attendee ever made one.
fn saved_locale() -> Option<Locale> {
    let value = storage()?.get_item(STORAGE_KEY).ok()??;
    parse_locale(&value)
}

/// `"th"` / `"en"` → `Locale`; anything else is not a pick.
pub fn parse_locale(value: &str) -> Option<Locale> {
    match value {
        "th" => Some(Locale::th),
        "en" => Some(Locale::en),
        _ => None,
    }
}

/// Apply the stored pick. Rendered once, inside `I18nContextProvider` and
/// before the routes.
///
/// An Effect, not a plain call: the provider seeds its locale from
/// `navigator.languages` in its own Effect, which runs after component bodies
/// and overwrote a pick set here directly (a reload came back in EN with `th`
/// stored). This Effect is created after the provider's, so it runs after it
/// in the same flush, before the browser paints.
#[component]
pub fn RestoreSavedLocale() -> impl IntoView {
    let i18n = use_i18n();
    Effect::new(move |_| {
        if let Some(locale) = saved_locale()
            && locale != i18n.get_locale_untracked()
        {
            i18n.set_locale(locale);
        }
    });
}

/// Routes that are attendee-facing and therefore bilingual. Everything else
/// (admin, staff, dashboards) is English-only for now and shows no switch.
const ATTENDEE_PREFIXES: [&str; 11] = [
    "/e/",
    "/deposit/",
    "/ticket/",
    "/claim/",
    "/discover",
    "/feedback",
    "/privacy",
    "/data-privacy",
    "/past-events",
    "/faq",
    "/profile",
];

/// `/events/{slug}/…` pages that attendees see. The rest of `/events/` (the
/// summary and PR pack) is staff-only.
const ATTENDEE_EVENT_SUFFIXES: [&str; 2] = ["/recap", "/post-event-register"];

/// Whether `path` is a bilingual attendee page (`/` and `/login` included).
pub fn is_attendee_path(path: &str) -> bool {
    matches!(path, "/" | "/login")
        || ATTENDEE_PREFIXES.iter().any(|p| path.starts_with(p))
        || (path.starts_with("/events/")
            && ATTENDEE_EVENT_SUFFIXES.iter().any(|s| path.ends_with(s)))
}

/// Whether the language bar above the page is drawn: on attendee pages,
/// except the landing, whose header carries the switch inline.
pub fn shows_lang_bar(path: &str) -> bool {
    path != "/" && is_attendee_path(path)
}

/// The switch, on attendee pages only.
#[component]
pub fn AttendeeLanguageSwitch() -> impl IntoView {
    let location = leptos_router::hooks::use_location();
    view! {
        <Show when=move || shows_lang_bar(&location.pathname.get()) fallback=|| ()>
            <div class="lang-bar">
                <LanguageSwitch />
            </div>
        </Show>
    }
}

/// A catalog string that follows the language switch, as ONE concrete type.
///
/// `t!(i18n, key)` expands to a distinct closure type per call site, and
/// Leptos instantiates its render code for every one: ~1,700 call sites cost
/// ~135 KB brotli of wasm. A `Signal<&'static str>` is a single type, so the
/// render code exists once and each key is only a `fn(Locale) -> &str`.
/// Use `t!` only where a key interpolates.
///
/// ```ignore
/// view! { <h1>{tr(|l| td_string!(l, discover.title))}</h1> }
/// ```
pub fn tr(text: fn(Locale) -> &'static str) -> Signal<&'static str> {
    let i18n = use_i18n();
    Signal::derive(move || text(i18n.get_locale()))
}

/// Fill `{name}` placeholders in a catalog string.
///
/// For catalog keys whose text goes into a `String` (toasts, error states).
/// `td_string!` can only interpolate `{{ name }}` with the crate-wide
/// `interpolate_display` option, which generates a builder per key; single
/// braces are plain text to `leptos_i18n`, so they reach this function as-is.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    args.iter()
        .fold(template.to_string(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
}

/// BCP 47 tag for `Intl` date formatting in `locale`.
///
/// EN keeps `en-GB` (day first, named month; `.issues/104`). TH uses `th-TH`,
/// which renders Thai month names and the Buddhist-era year Thai readers expect.
pub fn date_tag(locale: Locale) -> &'static str {
    match locale {
        Locale::en => "en-GB",
        Locale::th => "th-TH",
    }
}

/// The date tag for `locale` on `path`. English-only pages (admin, staff,
/// dashboards) keep `en-GB` whatever the language: the provider wraps every
/// route, so a Thai browser or a stored `th` pick otherwise put a
/// Buddhist-era year beside English text (`.issues/192`).
pub fn date_tag_on(locale: Locale, path: &str) -> &'static str {
    match is_attendee_path(path) {
        true => date_tag(locale),
        false => date_tag(Locale::en),
    }
}

/// The date tag for the current attendee language. Reactive: read inside a
/// view closure and the date re-renders when the language switches. Outside
/// the i18n provider (tests) and on English-only pages it is EN.
pub fn current_date_tag() -> &'static str {
    let Some(i18n) = use_context::<leptos_i18n::I18nContext<Locale>>() else {
        return date_tag(Locale::en);
    };
    match i18n.get_locale() {
        Locale::en => date_tag(Locale::en),
        locale => {
            let path = web_sys::window()
                .and_then(|w| w.location().pathname().ok())
                .unwrap_or_default();
            date_tag_on(locale, &path)
        }
    }
}

/// A registration status code from `/api/my-registrations` (or `"postponed"`)
/// in the current language. Unknown codes pass through unchanged, so a new
/// server-side status shows up as its code rather than disappearing.
pub fn status_label(code: &str) -> String {
    let i18n = use_i18n();
    let label = match code {
        "postponed" => t_string!(i18n, status.postponed),
        "registered" => t_string!(i18n, status.registered),
        "deposit pending" => t_string!(i18n, status.deposit_pending),
        "deposit confirmed" => t_string!(i18n, status.deposit_confirmed),
        "checked in" => t_string!(i18n, status.checked_in),
        "nft claimed" => t_string!(i18n, status.nft_claimed),
        other => return other.to_string(),
    };
    label.to_string()
}

/// EN ⇄ TH switch. Labelled in the language it switches TO, so a reader who
/// cannot read the current page can still find it.
#[component]
pub fn LanguageSwitch() -> impl IntoView {
    let i18n = use_i18n();
    let toggle = move |_| {
        let next = match i18n.get_locale_untracked() {
            Locale::en => Locale::th,
            Locale::th => Locale::en,
        };
        i18n.set_locale(next);
        if let Some(store) = storage() {
            let _ = store.set_item(STORAGE_KEY, leptos_i18n::Locale::as_str(next));
        }
    };
    view! {
        <button
            type="button"
            class="lang-switch"
            on:click=toggle
            aria-label=crate::locale::tr(|l| crate::i18n::td_string!(l, lang.switch_aria))
        >
            <span class="lang-switch-chip">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, lang.switch_label))}
            </span>
        </button>
    }
}
