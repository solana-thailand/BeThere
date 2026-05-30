use super::types::*;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn share_button(
    current_slug: &str,
    event_name: &str,
    share_copied: ReadSignal<bool>,
    set_share_copied: WriteSignal<bool>,
) -> AnyView {
    let share_slug = current_slug.to_string();
    let share_name = event_name.to_string();
    let set_copied = set_share_copied.clone();

    view! {
        <div class="pe-share-row">
            <button
                class="btn btn-outline btn-sm"
                on:click=move |_| {
                    let share_slug = share_slug.clone();
                    let share_name = share_name.clone();
                    let set_c = set_copied.clone();
                    leptos::task::spawn_local(async move {
                        let window = web_sys::window().expect("no window");
                        let url = format!("{}/e/{share_slug}", window.location().origin().unwrap_or_default());
                        let _ = wasm_bindgen_futures::JsFuture::from(
                            share_event_js(&share_name, &url)
                        ).await;
                        set_c.set(true);
                        gloo_timers::future::TimeoutFuture::new(2000).await;
                        set_c.set(false);
                    });
                }
            >
                <Icon icon=IconName::Link class="icon-sm" />" Share Event ↗"
            </button>
            {move || {
                if share_copied.get() {
                    view! {
                        <span class="pe-share-copied">"Link copied!"</span>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}
        </div>
    }.into_any()
}
