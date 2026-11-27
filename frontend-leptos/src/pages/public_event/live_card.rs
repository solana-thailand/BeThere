//! "On now / Next" while the show runs, from the event's Ontime room (plan 050).
//!
//! The room's attendee feed needs no token and carries no notes. Its socket
//! sends a packet only when something changes; between packets the card
//! counts down by itself. Nothing is stored: when the organiser edits the show
//! in Ontime, the room sends the new agenda and the card follows.
//!
//! The socket opens only around the event (`WINDOW_MS` either side), so a page
//! read weeks before costs the room nothing, and closes with the page: one
//! loop owns it and ends when its signals are gone, like `SessionTimer`.

use event_checkin_domain::models::event::{OntimeFeed, safe_ontime_feed};
use event_checkin_domain::models::live_agenda::{
    Finish, LiveAgenda, NowCard, Pace, clock_text, time_of_day,
};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{CloseEvent, MessageEvent, WebSocket};

use crate::i18n::{Locale, t_string, use_i18n};
use crate::locale::fill;

/// Listen from this long before the start until this long after the end.
const WINDOW_MS: i64 = 3 * 3_600_000;
const TICK_MS: u32 = 1_000;
const FIRST_RETRY_MS: i64 = 1_000;
const MAX_RETRY_MS: i64 = 30_000;

/// A socket and the handlers it calls, dropped together.
struct Connection {
    socket: WebSocket,
    _on_message: Closure<dyn Fn(MessageEvent)>,
    _on_close: Closure<dyn Fn(CloseEvent)>,
}

impl Connection {
    fn open(feed: &OntimeFeed, set_live: WriteSignal<LiveAgenda>) -> Option<Self> {
        let socket = WebSocket::new(&feed.socket_url()).ok()?;
        let on_message = Closure::<dyn Fn(MessageEvent)>::new(move |event: MessageEvent| {
            if let Some(text) = event.data().as_string() {
                let now = js_sys::Date::now() as i64;
                let _ = set_live.try_update(|live| live.apply(&text, now));
            }
        });
        let on_close = Closure::<dyn Fn(CloseEvent)>::new(move |event: CloseEvent| {
            // the room turned its feed off: show nothing of the show
            if event.code() == 1000 && !event.reason().is_empty() {
                let _ = set_live.try_update(LiveAgenda::clear);
            }
        });
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        Some(Self {
            socket,
            _on_message: on_message,
            _on_close: on_close,
        })
    }

    fn is_closed(&self) -> bool {
        self.socket.ready_state() == WebSocket::CLOSED
    }

    fn is_open(&self) -> bool {
        self.socket.ready_state() == WebSocket::OPEN
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // detach first: the handlers are freed with this value
        self.socket.set_onmessage(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
}

/// The live card, or nothing when the event has no Ontime room, the show is
/// not running, or the room is not reachable.
#[component]
pub fn LiveCard(feed_url: Option<String>, start_ms: i64, end_ms: i64) -> impl IntoView {
    let Some(feed) = feed_url.as_deref().and_then(safe_ontime_feed) else {
        return ().into_any();
    };
    let (live, set_live) = signal(LiveAgenda::default());
    let (now, set_now) = signal(js_sys::Date::now() as i64);

    let loop_feed = feed.clone();
    leptos::task::spawn_local(async move {
        let mut connection: Option<Connection> = None;
        let mut retry_ms = FIRST_RETRY_MS;
        let mut retry_at = 0_i64;
        loop {
            let now_ms = js_sys::Date::now() as i64;
            // unmounted: stop, and `connection` closes as it drops
            if set_now.try_set(now_ms).is_some() {
                break;
            }
            let around = now_ms >= start_ms - WINDOW_MS && now_ms <= end_ms + WINDOW_MS;
            match (&connection, around) {
                // the window closed: drop the socket and forget the show, or
                // a card missed by its last packet would stay "on now"
                (Some(_), false) => {
                    connection = None;
                    let _ = set_live.try_update(LiveAgenda::clear);
                }
                (Some(open), true) if open.is_closed() => {
                    connection = None;
                    retry_at = now_ms + retry_ms;
                    retry_ms = (retry_ms * 2).min(MAX_RETRY_MS);
                }
                (Some(open), true) if open.is_open() => retry_ms = FIRST_RETRY_MS,
                (None, true) if now_ms >= retry_at => {
                    connection = Connection::open(&loop_feed, set_live);
                    // the browser refused the address: back off like a drop
                    if connection.is_none() {
                        retry_at = now_ms + retry_ms;
                        retry_ms = (retry_ms * 2).min(MAX_RETRY_MS);
                    }
                }
                _ => {}
            }
            gloo_timers::future::TimeoutFuture::new(TICK_MS).await;
        }
    });

    let i18n = use_i18n();
    // `card` moves every second; `shape` (the card without its clock) changes
    // only when the session, the pace or the finish does. The section is built
    // from `shape`, so it is not replaced each second: focus on the agenda
    // link survives, a tap is not lost, and `aria-live` sits on a node that
    // stays. Only the countdown and the bar read `card`.
    let card = Memo::new(move |_| live.with(|l| l.card(now.get())));
    let shape = Memo::new(move |_| card.get().map(without_clock));
    view! {
        {move || shape.get().map(|shape| {
            let locale = i18n.get_locale();
            let agenda_href = feed.agenda_url(match locale {
                Locale::th => "th",
                Locale::en => "en",
            });
            card_view(shape, card, locale, agenda_href)
        })}
    }
    .into_any()
}

/// The card with its clock zeroed: whether there is a countdown stays, its
/// value does not.
fn without_clock(card: NowCard) -> NowCard {
    NowCard {
        left_ms: card.left_ms.map(|_| 0),
        progress: 0.0,
        ..card
    }
}

/// `5 min`, `2 h`, `7 h 32 min`.
fn span(minutes: u32, locale: Locale) -> String {
    let (h, m) = ((minutes / 60).to_string(), (minutes % 60).to_string());
    let words = |key: fn(Locale) -> &'static str| key(locale);
    match (minutes / 60, minutes % 60) {
        (0, _) => fill(
            words(|l| crate::i18n::td_string!(l, event.live_minutes)),
            &[("m", &m)],
        ),
        (_, 0) => fill(
            words(|l| crate::i18n::td_string!(l, event.live_hours)),
            &[("h", &h)],
        ),
        _ => fill(
            words(|l| crate::i18n::td_string!(l, event.live_hours_minutes)),
            &[("h", &h), ("m", &m)],
        ),
    }
}

fn card_view(
    card: NowCard,
    clock: Memo<Option<NowCard>>,
    locale: Locale,
    agenda_href: String,
) -> impl IntoView {
    let text = |key: fn(Locale) -> &'static str| key(locale);
    let i18n = use_i18n();
    let (state, state_class) = match card.paused {
        true => (
            t_string!(i18n, event.live_paused),
            "pe-live-chip pe-live-chip-paused",
        ),
        false => (
            t_string!(i18n, event.live_now),
            "pe-live-chip pe-live-chip-live",
        ),
    };
    let (pace, pace_class) = match card.pace {
        Pace::OnTime => (
            t_string!(i18n, event.live_on_time).to_string(),
            "pe-live-pace pe-live-pace-ontime",
        ),
        Pace::Late { minutes } => (
            fill(
                text(|l| crate::i18n::td_string!(l, event.live_late)),
                &[("time", &span(minutes, locale))],
            ),
            "pe-live-pace pe-live-pace-late",
        ),
        Pace::Ahead { minutes } => (
            fill(
                text(|l| crate::i18n::td_string!(l, event.live_ahead)),
                &[("time", &span(minutes, locale))],
            ),
            "pe-live-pace pe-live-pace-ahead",
        ),
    };
    let fields = card
        .fields
        .iter()
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
    let left_ms = move || clock.with(|c| c.as_ref().and_then(|c| c.left_ms));
    let has_left = card.left_ms.is_some();
    let next = card.next_title.map(|title| {
        fill(
            text(|l| crate::i18n::td_string!(l, event.live_next)),
            &[("title", &title)],
        )
    });
    let finish = card.finish.map(|finish| match finish {
        Finish::Expected(ms) => fill(
            text(|l| crate::i18n::td_string!(l, event.live_finish)),
            &[("time", &time_of_day(ms))],
        ),
        Finish::Planned(ms) => fill(
            text(|l| crate::i18n::td_string!(l, event.live_planned)),
            &[("time", &time_of_day(ms))],
        ),
    });
    let width = move || {
        let progress = clock.with(|c| c.as_ref().map_or(0.0, |c| c.progress));
        format!("width: {:.1}%", progress * 100.0)
    };
    view! {
        <section class="pe-live" aria-label=t_string!(i18n, event.live_aria) aria-live="polite">
            <div class="pe-live-head">
                <span class=state_class>{state}</span>
                <span class=pace_class>{pace}</span>
            </div>
            <p class="pe-live-label">{t_string!(i18n, event.live_on_now)}</p>
            <h2 class="pe-live-title">{card.title}</h2>
            {(!fields.is_empty()).then(|| view! { <p class="pe-live-fields">{fields}</p> })}
            // `aria-live="off"`: a countdown read out every second is noise
            {has_left.then(|| view! {
                <p
                    class="pe-live-left"
                    class:pe-live-over=move || left_ms().is_some_and(|ms| ms < 0)
                    aria-live="off"
                >
                    {move || left_ms().map(clock_text).unwrap_or_default()}
                </p>
            })}
            <div class="pe-live-bar" aria-hidden="true"><span style=width></span></div>
            {next.map(|next| view! { <p class="pe-live-next">{next}</p> })}
            <div class="pe-live-foot">
                {finish.map(|finish| view! { <span>{finish}</span> })}
                <a href=agenda_href target="_blank" rel="noopener noreferrer">
                    {t_string!(i18n, event.live_full_agenda)}
                </a>
            </div>
        </section>
    }
}
