//! "Marketing emails" on the profile (.plans/038 P2-e): the current state,
//! that it can be withdrawn at any time, and a withdrawal that clears every
//! store (`POST /api/privacy/unsubscribe-marketing`, #117). Opting back in
//! happens at registration, where the consent has its event context.

use leptos::prelude::*;

use crate::api;

#[derive(Clone, Copy, PartialEq)]
enum Pref {
    Loading,
    On,
    Off,
    Failed,
}

#[component]
pub fn MarketingPreference() -> impl IntoView {
    let (pref, set_pref) = signal(Pref::Loading);
    let (busy, set_busy) = signal(false);
    leptos::task::spawn_local(async move {
        set_pref.set(match api::get_marketing_consent().await {
            Ok(true) => Pref::On,
            Ok(false) => Pref::Off,
            Err(_) => Pref::Failed,
        });
    });
    let withdraw = move |_| {
        set_busy.set(true);
        leptos::task::spawn_local(async move {
            let next = match api::unsubscribe_marketing().await {
                Ok(_) => Pref::Off,
                Err(_) => Pref::Failed,
            };
            set_pref.set(next);
            set_busy.set(false);
        });
    };
    view! {
        <section class="marketing-pref" aria-live="polite">
            <h2 class="marketing-pref-title">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.pref_title))}
            </h2>
            {move || match pref.get() {
                Pref::Loading => ().into_any(),
                Pref::On => view! {
                    <p class="marketing-pref-state">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.pref_on))}
                        " "
                        <strong>{crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.pref_anytime))}</strong>
                    </p>
                    <button class="btn btn-outline btn-sm" disabled=move || busy.get() on:click=withdraw>
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.pref_withdraw))}
                    </button>
                }.into_any(),
                Pref::Off => view! {
                    <p class="marketing-pref-state">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.pref_off))}
                    </p>
                }.into_any(),
                Pref::Failed => view! {
                    <p class="marketing-pref-state">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.pref_failed))}
                    </p>
                }.into_any(),
            }}
            <a href="/data-privacy" class="marketing-pref-more">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, privacy.pref_more))}
            </a>
        </section>
    }
}
