//! Cloudflare Turnstile on the waitlist and register forms (`.issues/170`).
//!
//! Lazy on purpose. Nothing is fetched until the visitor first touches the
//! form ([`BotCheck::activate`]): not the config request (a Worker request
//! against the free quota) and not Cloudflare's script (a third party). When
//! the worker reports the check off, the form behaves exactly as before.

use event_checkin_domain::turnstile::TOKEN_HEADER;
use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::i18n::{Locale, use_i18n};

#[wasm_bindgen(module = "/js/turnstile.js")]
extern "C" {
    #[wasm_bindgen(js_name = "renderTurnstile")]
    async fn render_turnstile(
        container: web_sys::HtmlElement,
        site_key: &str,
        language: &str,
        on_token: &Closure<dyn Fn(String)>,
        on_clear: &Closure<dyn Fn()>,
    ) -> JsValue;

    #[wasm_bindgen(js_name = "resetTurnstile")]
    fn reset_turnstile(widget_id: &str);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Not touched yet: no request made.
    Dormant,
    /// Config requested.
    Loading,
    /// The worker does not check tokens.
    Off,
    /// The widget is on the page.
    On,
}

#[derive(Clone, serde::Deserialize)]
struct TurnstileConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    site_key: String,
}

/// Per-form handle. `Copy`, so submit closures can capture it.
#[derive(Clone, Copy)]
pub struct BotCheck {
    phase: RwSignal<Phase>,
    token: RwSignal<Option<String>>,
    widget_id: StoredValue<String>,
    slot: NodeRef<Div>,
}

impl BotCheck {
    pub fn new() -> Self {
        Self {
            phase: RwSignal::new(Phase::Dormant),
            token: RwSignal::new(None),
            widget_id: StoredValue::new(String::new()),
            slot: NodeRef::new(),
        }
    }

    /// Whether submit may proceed: the check is off, or a token is in hand.
    /// Untouched counts as not ready, so a scripted submit that never focused
    /// the form still waits for the config.
    pub fn ready(&self) -> bool {
        match self.phase.get() {
            Phase::Off => true,
            Phase::On => self.token.with(Option::is_some),
            Phase::Dormant | Phase::Loading => false,
        }
    }

    /// The header to send with the gated request, when there is a token.
    pub fn header(&self) -> Option<(&'static str, String)> {
        self.token
            .get_untracked()
            .map(|token| (TOKEN_HEADER, token))
    }

    /// Spend the token: siteverify accepts each one once.
    pub fn reset(&self) {
        self.token.set(None);
        self.widget_id.with_value(|id| reset_turnstile(id));
    }

    /// First touch of the form: fetch the config, render the widget if on.
    pub fn activate(&self) {
        if self.phase.get_untracked() != Phase::Dormant {
            return;
        }
        self.phase.set(Phase::Loading);
        let check = *self;
        let language = match use_i18n().get_locale_untracked() {
            Locale::th => "th",
            Locale::en => "en",
        };
        let known = CONFIG.with_borrow_mut(|state| match state {
            ConfigState::Idle => {
                *state = ConfigState::InFlight(Vec::new());
                None
            }
            ConfigState::InFlight(waiters) => {
                waiters.push((check, language));
                Some(None)
            }
            ConfigState::Done(config) => Some(Some(config.clone())),
        });
        match known {
            // Another form's request will apply the answer to this one too.
            Some(None) => {}
            Some(Some(config)) => leptos::task::spawn_local(check.apply(config, language)),
            None => leptos::task::spawn_local(async move {
                let config = request_config().await;
                let waiters = CONFIG.with_borrow_mut(|state| {
                    match std::mem::replace(state, ConfigState::Done(config.clone())) {
                        ConfigState::InFlight(waiters) => waiters,
                        _ => Vec::new(),
                    }
                });
                for (waiter, waiter_language) in waiters {
                    leptos::task::spawn_local(waiter.apply(config.clone(), waiter_language));
                }
                check.apply(config, language).await;
            }),
        }
    }

    async fn apply(self, config: Option<TurnstileConfig>, language: &'static str) {
        let check = self;
        let (Some(config), Some(slot)) = (config, check.slot.get_untracked()) else {
            // Fail open on the client only: the worker still decides. An
            // unreachable config endpoint means a submit that the worker will
            // judge, not a form that can never be sent.
            check.phase.set(Phase::Off);
            return;
        };
        if !config.enabled || config.site_key.is_empty() {
            check.phase.set(Phase::Off);
            return;
        }
        check.phase.set(Phase::On);
        let on_token = Closure::<dyn Fn(String)>::new(move |token: String| {
            check.token.set(Some(token));
        });
        let on_clear = Closure::<dyn Fn()>::new(move || check.token.set(None));
        let id = render_turnstile(
            slot.into(),
            &config.site_key,
            language,
            &on_token,
            &on_clear,
        )
        .await;
        // The widget calls these for as long as the page lives.
        on_token.forget();
        on_clear.forget();
        check
            .widget_id
            .set_value(id.as_string().unwrap_or_default());
    }
}

impl Default for BotCheck {
    fn default() -> Self {
        Self::new()
    }
}

/// The config does not change while the page lives, and a form can render
/// more than once (the event page renders it per auth/lookup state), so one
/// request serves every form. A failed request is remembered too: the worker
/// still enforces, and a reload retries.
enum ConfigState {
    Idle,
    InFlight(Vec<(BotCheck, &'static str)>),
    Done(Option<TurnstileConfig>),
}

thread_local! {
    static CONFIG: std::cell::RefCell<ConfigState> =
        const { std::cell::RefCell::new(ConfigState::Idle) };
}

async fn request_config() -> Option<TurnstileConfig> {
    let origin = web_sys::window()?.location().origin().ok()?;
    let url = format!("{origin}/api/public/turnstile/config");
    let response = crate::api::fetch::get(&url, &[]).await.ok()?;
    match response.ok() {
        true => crate::api::fetch::response_json(&response).await.ok(),
        false => None,
    }
}

/// Where the widget renders. Reserves no space until the check is on, so a
/// deployment without Turnstile has no layout shift.
#[component]
pub fn BotCheckSlot(check: BotCheck) -> impl IntoView {
    view! {
        <div
            class="bot-check-slot"
            class:is-on=move || check.phase.get() == Phase::On
            node_ref=check.slot
        ></div>
    }
}
