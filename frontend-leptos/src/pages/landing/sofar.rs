//! "So far" on the night field (.plans/043 L5, ASKS-4 §14): what BeThere has
//! carried, every figure from `GET /api/public/stats` with its measured-at
//! time (build plan rule 1). The numbers count up once when they come into
//! view; under reduced motion they are simply there. Without stats the lines
//! are not drawn at all. The goal and its globe (L10) are `goal.rs`, under
//! the strip; the photo reel (L9) is `photos.rs`, above it.

use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::i18n::td_string;

use super::goal::Goal;
use super::hero::MarkupText;
use super::photos::PhotoReel;
use super::stats::{
    count_at, group_thousands, measured_at_label, payers_line_shown, use_landing_stats,
};
use super::story::ladder_total;

const COUNT_UP_MS: f64 = 1200.0;
const FRAME_MS: u64 = 16;
/// Labels per tape: the run loops by half its width, so it must be more than
/// twice the widest screen (32 × ~110 px).
const TAPE_REPEATS: usize = 32;

pub(super) fn reduced_motion() -> bool {
    web_sys::window()
        .and_then(|w| {
            w.match_media("(prefers-reduced-motion: reduce)")
                .ok()
                .flatten()
        })
        .is_some_and(|q| q.matches())
}

/// Sets `seen` the first time `target` scrolls into view, then stops watching.
pub(super) fn watch_first_sight(target: NodeRef<Div>, seen: RwSignal<bool>) {
    let observer = StoredValue::new_local(None::<web_sys::IntersectionObserver>);
    Effect::new(move |_| {
        let Some(el) = target.get() else {
            return;
        };
        let callback = Closure::<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>::new(
            move |entries: js_sys::Array, obs: web_sys::IntersectionObserver| {
                let visible = entries.iter().any(|e| {
                    e.dyn_into::<web_sys::IntersectionObserverEntry>()
                        .is_ok_and(|e| e.is_intersecting())
                });
                if visible {
                    seen.set(true);
                    obs.disconnect();
                }
            },
        );
        match web_sys::IntersectionObserver::new(callback.as_ref().unchecked_ref()) {
            Ok(obs) => {
                obs.observe(&el);
                observer.set_value(Some(obs));
            }
            // No observer (very old browser): show the numbers.
            Err(_) => seen.set(true),
        }
        // Lives until the observer fires once or the page goes away (on_cleanup
        // below disconnects it); one small closure per landing mount.
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

#[component]
pub fn SoFar() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let stats = use_landing_stats();
    let strip = NodeRef::<Div>::new();
    let seen = RwSignal::new(false);
    let progress = RwSignal::new(0.0_f64);
    watch_first_sight(strip, seen);

    // Run the count once: when the strip has been seen and the stats are in.
    let interval = StoredValue::new(None::<IntervalHandle>);
    Effect::new(move |_| {
        if stats.with(Option::is_none) || interval.with_value(Option::is_some) {
            return;
        }
        // Nothing moves under reduced motion, so there is nothing to wait for:
        // the final figures, whether or not the strip has been seen.
        if reduced_motion() {
            progress.set(1.0);
            return;
        }
        if !seen.get() {
            return;
        }
        let start = js_sys::Date::now();
        let handle = set_interval_with_handle(
            move || {
                let t = (js_sys::Date::now() - start) / COUNT_UP_MS;
                progress.set(t.min(1.0));
                if t >= 1.0 {
                    interval.with_value(|h| {
                        if let Some(h) = h {
                            h.clear();
                        }
                    });
                }
            },
            std::time::Duration::from_millis(FRAME_MS),
        );
        interval.set_value(handle.ok());
    });
    on_cleanup(move || {
        interval.with_value(|h| {
            if let Some(h) = h {
                h.clear();
            }
        });
    });

    // `t` is how far through the count-up: 1.0 is the final figures, which a
    // screen reader gets at once (the counting copy is aria-hidden).
    let done_at = move |t: f64| {
        let Some(s) = stats.get() else {
            return String::new();
        };
        let n = |v: u64| group_thousands(count_at(v, t));
        crate::locale::fill(
            td_string!(i18n.get_locale(), landing.sofar.done),
            &[
                ("events", &n(u64::from(s.events_held))),
                ("onsite", &n(u64::from(s.onsite_registrations))),
                ("door", &n(u64::from(s.door_scans))),
                ("online", &n(u64::from(s.online_registrations))),
                ("thb", &n(s.deposits_handled_thb)),
            ],
        )
    };
    let payers_at = move |t: f64| {
        let Some(s) = stats.get().filter(payers_line_shown) else {
            return String::new();
        };
        crate::locale::fill(
            td_string!(i18n.get_locale(), landing.sofar.payers),
            &[
                (
                    "came",
                    &group_thousands(count_at(u64::from(s.deposit_payers_came), t)),
                ),
                ("paid", &group_thousands(u64::from(s.deposit_payers))),
            ],
        )
    };
    let payers_total = move || {
        let (paid, came) = ladder_total();
        crate::locale::fill(
            td_string!(i18n.get_locale(), landing.sofar.payers_total),
            &[("came", &came.to_string()), ("paid", &paid.to_string())],
        )
    };
    let done = Signal::derive(move || done_at(progress.get()));
    let payers = Signal::derive(move || payers_at(progress.get()));
    let read_out = move || {
        let strip = |s: String| s.replace("**", "");
        format!("{} {}", strip(done_at(1.0)), strip(payers_at(1.0)))
            .trim_end()
            .to_string()
    };
    let livebar = move || {
        stats.get().map(|s| {
            crate::locale::fill(
                td_string!(i18n.get_locale(), landing.sofar.livebar),
                &[("at", &measured_at_label(&s.measured_at))],
            )
        })
    };
    let tape = move || {
        (0..TAPE_REPEATS)
            .map(|_| view! { <span>{crate::locale::tr(|l| td_string!(l, landing.sofar.tape))}</span><i></i> })
            .collect::<Vec<_>>()
    };

    view! {
        <section id="goal" class="lp-goal">
            <div class="lp-tape" aria-hidden="true"><div class="lp-tape-run">{tape()}</div></div>
            <PhotoReel />
            <div class="lp-wrap" node_ref=strip>
                {move || stats.with(Option::is_some).then(|| view! {
                    <p class="lp-livebar"><i></i><span>{livebar}</span></p>
                    <p class="lp-sr">{read_out}</p>
                    <p class="lp-done" aria-hidden="true"><MarkupText text=done /></p>
                })}
                {move || stats.with(|s| s.as_ref().is_some_and(payers_line_shown)).then(|| view! {
                    <p class="lp-done lp-payers" aria-hidden="true"><MarkupText text=payers /></p>
                    <p class="lp-fineprint lp-center">{crate::locale::tr(|l| td_string!(l, landing.sofar.payers_note))}</p>
                    // The ladder's total (RTM #1–#6, hand records included), under
                    // the system-only figure above: both true, both labelled.
                    <p class="lp-fineprint lp-center">{payers_total}</p>
                })}
            </div>
            <div class="lp-tape lp-t2" aria-hidden="true"><div class="lp-tape-run">{tape()}</div></div>
            <Goal />
        </section>
    }
}
