//! The landing hero (build plan 0.5, ASKS-4 §11, §14, §17): the promise, the
//! sub-line whose keywords link to where each is explained, today's rail and
//! the coming one, and the 6 s loop that tells the whole story without words.
//! Reduced motion shows the loop's end state, still (`style-23-landing.css`).

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

#[component]
pub fn Hero(auth_state: ReadSignal<AuthState>, user_role: ReadSignal<String>) -> impl IntoView {
    let tr = crate::locale::tr;
    view! {
        <header class="lp-hero" id="top">
            <div class="lp-wrap lp-hero-grid">
                <div>
                    <p class="lp-kicker">{tr(|l| crate::i18n::td_string!(l, landing.hero.kicker))}</p>
                    <h1 class="lp-hero-h1">
                        <span>{tr(|l| crate::i18n::td_string!(l, landing.hero.headline_1))}</span>
                        <br />
                        <span class="lp-hero-h1-2">{tr(|l| crate::i18n::td_string!(l, landing.hero.headline_2))}</span>
                    </h1>
                    <p class="lp-hero-sub">
                        <Markup text=tr(|l| crate::i18n::td_string!(l, landing.hero.sub)) />
                    </p>
                    <p class="lp-hero-now">{tr(|l| crate::i18n::td_string!(l, landing.hero.now))}</p>
                    <p class="lp-quote">{tr(|l| crate::i18n::td_string!(l, landing.hero.quote))}</p>
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
                                _ => view! {
                                    <a href="#events" class="lp-btn lp-btn-primary">
                                        {tr(|l| crate::i18n::td_string!(l, landing.cta.find_events))}
                                    </a>
                                }.into_any(),
                            }
                        }}
                        <WhyFilm />
                    </div>
                </div>
                <div class="lp-loopcard" aria-hidden="true">
                    <HeroLoop />
                </div>
            </div>
        </header>
    }
}

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
        let src = format!("/media/why-{lang}.mp4");
        if video.get_attribute("src").as_deref() != Some(src.as_str()) {
            video.set_poster(&format!("/media/why-{lang}.jpg"));
            captions.set_src(&format!("/media/why-{lang}.vtt"));
            captions.set_srclang(lang);
            video.set_src(&src);
        }
        if dialog.show_modal().is_ok() {
            let _ = video.play();
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
        <button class="lp-btn" type="button" on:click=open>
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

/// Deposit chip drops on the ticket, the QR is scanned and ticked, "after
/// the event", the chip travels to the phone, the badge appears. Keyframes
/// only; the art keeps the paper card's own colours in both themes. The
/// chip label is bold: 20 px bold is large text, and it sits at 4.0:1.
#[component]
fn HeroLoop() -> impl IntoView {
    let after = crate::locale::tr(|l| crate::i18n::td_string!(l, landing.hero.after_event));
    view! {
        <svg class="lp-loop" viewBox="0 0 420 300">
            <defs>
                <linearGradient id="lp-sol" x1="0" y1="0" x2="1" y2="1">
                    <stop offset="0" stop-color="#9945FF" />
                    <stop offset="1" stop-color="#14F195" />
                </linearGradient>
            </defs>
            <rect x="20" y="96" width="250" height="124" rx="16" fill="#f4f0e6" stroke="#121212" stroke-width="3" />
            <line x1="182" y1="108" x2="182" y2="208" stroke="#12152a" stroke-width="3" stroke-dasharray="7 7" />
            <text x="42" y="132" font-family="Inter" font-weight="700" font-size="22" fill="#12152a">"BeThere"</text>
            <rect x="200" y="122" width="52" height="52" rx="5" fill="#12152a" />
            <g fill="#f4f0e6">
                <rect x="206" y="128" width="13" height="13" />
                <rect x="233" y="128" width="13" height="13" />
                <rect x="206" y="155" width="13" height="13" />
                <rect x="226" y="149" width="8" height="8" />
                <rect x="236" y="159" width="10" height="10" />
            </g>
            <rect class="lp-h-scan" x="194" y="122" width="64" height="3" rx="1.5" fill="#6b63f0" />
            <polyline class="lp-h-tick" points="206,196 216,206 240,184" fill="none" stroke="#1f9d63" stroke-width="6" stroke-linecap="round" stroke-linejoin="round" />
            <rect x="320" y="120" width="70" height="128" rx="14" fill="none" stroke="#121212" stroke-width="4" />
            <g class="lp-h-clock">
                <circle cx="355" cy="62" r="22" fill="none" stroke="#121212" stroke-width="3" />
                <path d="M355 49 V62 L364 68" fill="none" stroke="#121212" stroke-width="3" stroke-linecap="round" />
                <text x="320" y="99" font-family="Inter, Anuphan" font-weight="600" font-size="14" fill="#121212">{after}</text>
            </g>
            <g class="lp-h-chip">
                <rect x="42" y="150" width="104" height="38" rx="19" fill="#6b63f0" />
                <text x="94" y="176" text-anchor="middle" font-family="Inter, Anuphan" font-weight="700" font-size="20" fill="#12152a">"฿500"</text>
            </g>
            <g class="lp-h-badge">
                <circle cx="355" cy="229" r="15" fill="#12152a" stroke="url(#lp-sol)" stroke-width="4" />
                <text x="355" y="233" text-anchor="middle" font-family="Inter" font-weight="700" font-size="11" fill="#f4f0e6">"Be"</text>
            </g>
        </svg>
    }
}
