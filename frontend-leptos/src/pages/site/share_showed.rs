//! "Share that I showed up" (Super GOAT SG4): on `/me/record` for each event
//! the person came to, and on the ticket after check-in. Makes (or reuses)
//! the person's `/showed/{token}` link, hands it to the share sheet or the
//! clipboard, and can turn it off. The worker decides whether they came.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::i18n::td_string;
use crate::locale::tr;

#[wasm_bindgen(module = "/js/clipboard.js")]
extern "C" {
    #[wasm_bindgen(js_name = "shareOrCopy")]
    fn share_or_copy_js(url: &str, title: &str) -> js_sys::Promise;
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct Shared {
    token: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Note {
    None,
    Copied,
    Failed,
    Off,
}

fn link(token: &str) -> String {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    format!("{origin}/showed/{token}")
}

/// The share control for one event. `token` is the live link if the record
/// already has one.
#[component]
pub fn ShareShowed(slug: String, event_name: String, token: Option<String>) -> impl IntoView {
    let token = RwSignal::new(token);
    let note = RwSignal::new(Note::None);
    let busy = RwSignal::new(false);
    let slug = StoredValue::new(slug);
    let title = StoredValue::new(format!("I showed up · {event_name}"));

    let share = move |_| {
        busy.set(true);
        note.set(Note::None);
        leptos::task::spawn_local(async move {
            let live = match token.get_untracked() {
                Some(t) => Some(t),
                None => crate::api::api_post_json::<Shared>(
                    &format!("/me/showed/{}", slug.get_value()),
                    &serde_json::json!({}),
                )
                .await
                .ok()
                .map(|s| s.token)
                .filter(|t| !t.is_empty()),
            };
            match live {
                Some(t) => {
                    token.set(Some(t.clone()));
                    // The share sheet can stay open (or never settle, headless):
                    // the buttons wait only for the network, never for it.
                    busy.set(false);
                    let outcome = JsFuture::from(share_or_copy_js(&link(&t), &title.get_value()))
                        .await
                        .ok()
                        .and_then(|v| v.as_string())
                        .unwrap_or_default();
                    note.set(match outcome.as_str() {
                        "copied" => Note::Copied,
                        "shared" => Note::None,
                        _ => Note::Failed,
                    });
                }
                None => {
                    note.set(Note::Failed);
                    busy.set(false);
                }
            }
        });
    };
    let stop = move |_| {
        let Some(t) = token.get_untracked() else {
            return;
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::api_delete(&format!("/me/showed/{t}")).await {
                Ok(resp) if resp.ok() => {
                    token.set(None);
                    note.set(Note::Off);
                }
                _ => note.set(Note::Failed),
            }
            busy.set(false);
        });
    };

    view! {
        <span class="lp-share">
            <button class="lp-share-btn" type="button" disabled=move || busy.get() on:click=share>
                {tr(|l| td_string!(l, landing.site.showed_share))}
            </button>
            {move || token.get().is_some().then(|| view! {
                <button class="lp-share-off" type="button" disabled=move || busy.get() on:click=stop>
                    {tr(|l| td_string!(l, landing.site.showed_stop))}
                </button>
            })}
            {move || match note.get() {
                Note::Copied => Some(view! { <small role="status">{tr(|l| td_string!(l, landing.site.showed_copied))}</small> }.into_any()),
                Note::Off => Some(view! { <small role="status">{tr(|l| td_string!(l, landing.site.showed_off))}</small> }.into_any()),
                Note::Failed => Some(view! { <small role="alert">{tr(|l| td_string!(l, landing.site.showed_failed))}</small> }.into_any()),
                Note::None => None,
            }}
        </span>
    }
}
