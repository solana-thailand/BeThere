//! The "so far" reel (.plans/043 L9): event photos the owner chose on 6 Oct
//! 2026 (room shots and group photos already posted), from
//! `worker/landing-photos.jsonl` via `GET /api/public/landing-photos`.
//!
//! Nothing here is on the first load: the list is fetched when the reel first
//! comes into view, thumbnails load lazily, and a full-size image is fetched
//! only when tapped. Captions are the event label only. Under the reel, one
//! line says how to have a photo taken down.

use event_checkin_domain::models::landing_photo::LandingPhoto;
use leptos::html::Div;
use leptos::prelude::*;

use crate::api::ApiResponse;
use crate::i18n::{Locale, td_string};
use crate::locale::tr;

use super::sofar::watch_first_sight;

/// Where the worker serves a listed photo or thumbnail.
pub fn photo_url(name: &str) -> String {
    format!("/api/storage/landing-photos/{name}")
}

/// The alt text in the reader's language.
pub fn alt(photo: &LandingPhoto, locale: Locale) -> &str {
    match locale {
        Locale::th => &photo.alt_th,
        Locale::en => &photo.alt_en,
    }
}

async fn fetch_photos() -> Vec<LandingPhoto> {
    let url = format!("{}/public/landing-photos", crate::api::api_base());
    let Ok(resp) = crate::api::fetch::get(&url, &[]).await else {
        return Vec::new();
    };
    if resp.status() != 200 {
        log::warn!("[landing] photos returned {}", resp.status());
        return Vec::new();
    }
    crate::api::fetch::response_json::<ApiResponse<Vec<LandingPhoto>>>(&resp)
        .await
        .ok()
        .and_then(|w| w.data)
        .unwrap_or_default()
}

#[component]
pub fn PhotoReel() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let reel = NodeRef::<Div>::new();
    let seen = RwSignal::new(false);
    let photos = RwSignal::new(Vec::<LandingPhoto>::new());
    let started = StoredValue::new(false);
    watch_first_sight(reel, seen);
    Effect::new(move |_| {
        if !seen.get() || started.get_value() {
            return;
        }
        started.set_value(true);
        leptos::task::spawn_local(async move { photos.set(fetch_photos().await) });
    });

    let dialog = NodeRef::<leptos::html::Dialog>::new();
    let shown = RwSignal::new(None::<LandingPhoto>);
    let open = move |p: LandingPhoto| {
        shown.set(Some(p));
        if let Some(d) = dialog.get() {
            let _ = d.show_modal();
        }
    };
    let close = move || {
        if let Some(d) = dialog.get() {
            d.close();
        }
    };
    // A click on the backdrop lands on the dialog itself, not the image.
    let backdrop = move |ev: leptos::ev::MouseEvent| {
        if let Some(d) = dialog.get()
            && ev.target().as_ref() == Some(d.as_ref())
        {
            d.close();
        }
    };

    let tiles = move || {
        photos
            .get()
            .into_iter()
            .map(|p| {
                let label = p.event.clone();
                let thumb = photo_url(&p.thumb);
                // The full size's ratio: the reel's CSS fixes the height, so
                // the tile has its width before the thumbnail arrives.
                let (w, h) = (p.w, p.h);
                let alt_text = {
                    let p = p.clone();
                    move || alt(&p, i18n.get_locale()).to_string()
                };
                view! {
                    <figure class="lp-moment">
                        <button type="button" class="lp-moment-open" on:click=move |_| open(p.clone())>
                            <img src=thumb alt=alt_text width=w height=h loading="lazy" decoding="async" />
                        </button>
                        <figcaption>{label}</figcaption>
                    </figure>
                }
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="lp-reel" node_ref=reel>
            <div class="lp-reel-track">{tiles}</div>
            {move || photos.with(|p| !p.is_empty()).then(|| view! {
                <p class="lp-reel-remove">
                    {tr(|l| td_string!(l, landing.sofar.photo_remove))}
                    " "
                    <a href="#contact">{tr(|l| td_string!(l, landing.sofar.photo_remove_link))}</a>
                </p>
            })}
        </div>
        <dialog class="lp-photo" node_ref=dialog on:click=backdrop>
            <button class="lp-film-close" type="button"
                aria-label=tr(|l| td_string!(l, landing.hero.film_close))
                on:click=move |_| close()>
                "×"
            </button>
            {move || shown.get().map(|p| {
                let alt_text = alt(&p, i18n.get_locale()).to_string();
                view! {
                    <img src=photo_url(&p.file) alt=alt_text width=p.w height=p.h decoding="async" />
                    <p class="lp-photo-cap">{p.event.clone()}</p>
                }
            })}
        </dialog>
    }
}
