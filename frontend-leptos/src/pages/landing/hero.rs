//! The landing hero (build plan 0.5, ASKS-4 §11, §14, §17): the promise, the
//! sub-line whose keywords link to where each is explained, today's rail and
//! the coming one, beside the payers' hall (R4.2), all over the lit room
//! (R4.1, `room.rs`): reduced motion keeps the room dim and still.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::is_admin_role;
use crate::utils::copy_markup::{Segment, parse};

use super::auth::AuthState;

/// Catalog text with the copy markup rendered: keyword links, highlights,
/// and newlines left for `white-space: pre-line`. `MarkupText` takes text
/// built at run time (a catalog string with numbers filled in).
#[component]
pub fn Markup(text: Signal<&'static str>) -> impl IntoView {
    view! { <MarkupText text=Signal::derive(move || text.get().to_string()) /> }
}

#[component]
pub fn MarkupText(text: Signal<String>) -> impl IntoView {
    move || {
        let text = text.get();
        parse(&text)
            .into_iter()
            .map(|seg| match seg {
                Segment::Text(t) => t.to_string().into_any(),
                Segment::Link { text, href } => {
                    let (text, href) = (text.to_string(), href.to_string());
                    view! { <a class="lp-kw" href=href>{text}</a> }.into_any()
                }
                Segment::Highlight(t) => {
                    let t = t.to_string();
                    view! { <mark class="lp-hl">{t}</mark> }.into_any()
                }
            })
            .collect::<Vec<_>>()
    }
}

/// Chairs to a row of the hero's band: wide on a desktop (a few rows across the
/// page), narrower on a phone so the chairs stay chairs. Read once when the page
/// mounts.
fn hall_band_cols() -> usize {
    let width = web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0);
    match width < 768.0 {
        true => 14,
        false => 24,
    }
}

#[component]
pub fn Hero(auth_state: ReadSignal<AuthState>, user_role: ReadSignal<String>) -> impl IntoView {
    let tr = crate::locale::tr;
    let area = NodeRef::<leptos::html::Header>::new();
    let canvas = NodeRef::<leptos::html::Canvas>::new();
    super::room::use_lit_room(area, canvas);
    // The sentence's numbers are the hall's own: paid, came, and who did not.
    let (paid, came) = event_checkin_domain::models::facts::ladder_total();
    let i18n = crate::i18n::use_i18n();
    let lead = move || {
        crate::locale::fill(
            crate::i18n::td_string!(i18n.get_locale(), landing.hero.c_lead),
            &[
                ("came", &came.to_string()),
                ("paid", &paid.to_string()),
                ("gone", &paid.saturating_sub(came).to_string()),
            ],
        )
    };
    view! {
        <header class="lp-hero lp-hero-c" id="top" node_ref=area>
            // The lit room (.plans/045 R4.1), drawn by room/room.js.
            <canvas class="lp-room" width="320" height="180" aria-hidden="true" node_ref=canvas></canvas>
            <div class="lp-wrap lp-hero-c-grid">
                // The payers' hall as a band across the top (.plans/049, owner pick C):
                // the evidence first, then the words.
                <div class="lp-hero-c-hall">
                    <super::hall::Hall cols=hall_band_cols() show_fine=false />
                </div>
                <div class="lp-hero-text">
                  <div class="lp-hero-c-cols">
                    <h1 class="lp-hero-h1">
                        <span>{tr(|l| crate::i18n::td_string!(l, landing.hero.headline_1))}</span>
                        <br />
                        <span class="lp-hero-h1-2">{tr(|l| crate::i18n::td_string!(l, landing.hero.headline_2))}</span>
                    </h1>
                    // Wide screens: the words on the left, the sentence and the action
                    // on the right, level with the headline's last line.
                    <div class="lp-hero-c-side">
                    // The proof in one sentence, with the hall's own numbers.
                    <p class="lp-hero-sub">{lead}</p>
                    <div class="lp-actions">
                        {move || {
                            let role = user_role.get();
                            match auth_state.get() {
                                AuthState::SignedIn(_) if is_admin_role(&role) || role == "organizer" => view! {
                                    <A href="/admin" attr:class="lp-btn lp-btn-primary">
                                        {tr(|l| crate::i18n::td_string!(l, landing.cta.dashboard))}
                                    </A>
                                }.into_any(),
                                AuthState::SignedIn(_) if role == "staff" => view! {
                                    <A href="/staff" attr:class="lp-btn lp-btn-primary">
                                        {tr(|l| crate::i18n::td_string!(l, landing.cta.scanner))}
                                    </A>
                                }.into_any(),
                                // A signed-in attendee has nothing to wait for.
                                AuthState::SignedIn(_) => view! {
                                    <A href="/events" attr:class="lp-btn lp-btn-primary">
                                        {tr(|l| crate::i18n::td_string!(l, landing.hero.b_events_link))}
                                    </A>
                                }.into_any(),
                                // Organizer access is by invitation: the honest action
                                // is the waitlist, not "host an event".
                                _ => view! {
                                    <A href="/organizers#join" attr:class="lp-btn lp-btn-primary">
                                        {tr(|l| crate::i18n::td_string!(l, landing.join.waitlist_cta))}
                                    </A>
                                }.into_any(),
                            }
                        }}
                    </div>
                    // The one-minute film is a quiet link, not a second button.
                    <div class="lp-hero-more"><WhyFilm /></div>
                    // What the numbers count: kept, small, under the action.
                    <p class="lp-hero-scope">{tr(|l| crate::i18n::td_string!(l, landing.hall.fine))}</p>
                    </div>
                  </div>
                </div>
            </div>
        </header>
    }
}

/// The films' revision, in each URL: the files keep their names and are
/// cached for a day, so a re-render bumps this together with
/// `CACHE_KEY_VERSION` in worker/src/media.rs (which ignores the query).
const FILM_REV: &str = "v=5";

/// "Why a deposit · 1 min": the one-minute film (the commitment ladder) in a
/// dialog, with captions. The file is fetched only on the first click
/// (3.5 MB, `media/why-{lang}.mp4`), in the language the page is in then.
#[component]
fn WhyFilm() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let dialog = NodeRef::<leptos::html::Dialog>::new();
    let video = NodeRef::<leptos::html::Video>::new();
    let captions = NodeRef::<leptos::html::Track>::new();
    let open = move |_| {
        let (Some(dialog), Some(video), Some(captions)) =
            (dialog.get(), video.get(), captions.get())
        else {
            return;
        };
        let lang = leptos_i18n::Locale::as_str(i18n.get_locale_untracked());
        let src = format!("/media/why-{lang}.mp4?{FILM_REV}");
        if video.get_attribute("src").as_deref() != Some(src.as_str()) {
            video.set_poster(&format!("/media/why-{lang}.jpg?{FILM_REV}"));
            captions.set_src(&format!("/media/why-{lang}.vtt?{FILM_REV}"));
            captions.set_srclang(lang);
            video.set_src(&src);
        }
        // play() rejects (AbortError) when the dialog closes before playback
        // starts; await it so the rejection is handled, not logged as uncaught.
        if dialog.show_modal().is_ok()
            && let Ok(promise) = video.play()
        {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            });
        }
    };
    // A click on the backdrop lands on the dialog itself, not the video.
    let backdrop = move |ev: leptos::ev::MouseEvent| {
        if let Some(dialog) = dialog.get()
            && ev.target().as_ref() == Some(dialog.as_ref())
        {
            dialog.close();
        }
    };
    let pause = move |_| {
        if let Some(video) = video.get() {
            let _ = video.pause();
        }
    };
    view! {
        <button class="lp-link-btn" type="button" on:click=open>
            {crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.why))}
        </button>
        // Esc closes a modal dialog natively; so do a tap on the backdrop and
        // the close button, which a phone has no other way to find.
        <dialog class="lp-film" node_ref=dialog on:click=backdrop on:close=pause>
            <button
                class="lp-film-close"
                type="button"
                aria-label=crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.film_close))
                on:click=move |_| {
                    if let Some(dialog) = dialog.get() {
                        dialog.close();
                    }
                }
            >
                "×"
            </button>
            <video node_ref=video controls playsinline preload="none">
                <track
                    node_ref=captions
                    kind="captions"
                    label=crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.film_captions))
                />
            </video>
        </dialog>
    }
}
