//! "Try it on your event" (.plans/045, prototype organizers `plan-tile`): an
//! organizer types how many registered and sees how many would come if it
//! went like RTM #1, as a room of chairs, and how many meals that over-orders.
//! The rate is `domain::models::facts::expected_came`: one meetup's guide,
//! not a promise.

use leptos::prelude::*;

use crate::i18n::td_string;
use crate::locale::{fill, tr};
use crate::pages::landing::hall::CHAIR;
use event_checkin_domain::models::facts::{ROOM, expected_came, room_total};

/// The input's range and starting value, as in the prototype.
pub const PLAN_MIN: u32 = 1;
pub const PLAN_MAX: u32 = 200;
pub const PLAN_DEFAULT: u32 = 60;
const DX: f32 = 30.0;
const DY: f32 = 40.0;
const X0: f32 = 3.0;
const Y0: f32 = 6.0;

/// One chair of the planning room: where it sits, and whether that seat came.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanSeat {
    pub x: f32,
    pub y: f32,
    pub came: bool,
}

/// `registered` chairs, the first `expected_came` of them solid, 15 to a row
/// up to 60 and 20 above. Returns the seats and the drawing's size.
pub fn plan_seats(registered: u32) -> (Vec<PlanSeat>, f32, f32) {
    let came = expected_came(registered);
    let cols: u32 = if registered <= 60 { 15 } else { 20 };
    let seats = (0..registered)
        .map(|i| PlanSeat {
            x: X0 + (i % cols) as f32 * DX,
            y: Y0 + (i / cols) as f32 * DY,
            came: i < came,
        })
        .collect();
    let rows = registered.div_ceil(cols).max(1);
    (seats, cols as f32 * DX, Y0 + rows as f32 * DY)
}

/// The value the tile should show for what was typed: in range, or `None`
/// (the tile keeps its last good value).
pub fn parse_registered(raw: &str) -> Option<u32> {
    raw.trim()
        .parse::<f64>()
        .ok()
        .map(f64::round)
        .filter(|v| (f64::from(PLAN_MIN)..=f64::from(PLAN_MAX)).contains(v))
        .map(|v| v as u32)
}

#[component]
pub fn PlanTile() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let registered = RwSignal::new(PLAN_DEFAULT);
    let came = move || expected_came(registered.get());
    let over_line = move || {
        let v = registered.get();
        fill(
            td_string!(i18n.get_locale(), landing.site.plan_over),
            &[("v", &v.to_string()), ("over", &(v - came()).to_string())],
        )
    };
    let aria = move || {
        let v = registered.get();
        fill(
            td_string!(i18n.get_locale(), landing.site.plan_aria),
            &[("v", &v.to_string()), ("came", &came().to_string())],
        )
    };
    let fine = move || {
        let (reg, _) = room_total();
        let conf = ROOM[0].n + ROOM[1].n;
        fill(
            td_string!(i18n.get_locale(), landing.site.plan_fine),
            &[
                ("conf", &conf.to_string()),
                ("paid", &ROOM[0].n.to_string()),
                ("reg", &reg.to_string()),
            ],
        )
    };
    let room = move || {
        let (seats, w, h) = plan_seats(registered.get());
        let chairs = seats
            .into_iter()
            .map(|s| {
                let class = if s.came { "lp-hall-came" } else { "lp-hall-gone" };
                view! { <path class=class transform=format!("translate({} {})", s.x, s.y) d=CHAIR /> }
            })
            .collect::<Vec<_>>();
        view! {
            <svg class="lp-plan-room" role="img" aria-label=aria viewBox=format!("0 0 {w} {h}")>
                {chairs}
            </svg>
        }
    };
    view! {
        <div class="lp-card lp-plan">
            <p class="lp-kicker">{tr(|l| td_string!(l, landing.site.plan_kicker))}</p>
            <label class="lp-plan-in">
                <span>{tr(|l| td_string!(l, landing.site.plan_registered))}</span>
                <input
                    type="number"
                    min=PLAN_MIN
                    max=PLAN_MAX
                    step="1"
                    inputmode="numeric"
                    prop:value=move || registered.get().to_string()
                    on:input=move |ev| {
                        if let Some(v) = parse_registered(&event_target_value(&ev)) {
                            registered.set(v);
                        }
                    }
                />
            </label>
            <div aria-live="polite">
                <p class="lp-plan-big">
                    <b>{move || format!("≈{}", came())}</b>
                    <span>{tr(|l| td_string!(l, landing.site.plan_big))}</span>
                </p>
                <p class="lp-plan-over">{over_line}</p>
                {room}
                <p class="lp-plan-legend" aria-hidden="true">
                    <svg viewBox="0 0 24 30"><path class="lp-hall-came" d=CHAIR /></svg>
                    <span>{tr(|l| td_string!(l, landing.site.plan_came))}</span>
                    <svg viewBox="0 0 24 30"><path class="lp-hall-gone" d=CHAIR /></svg>
                    <span>{tr(|l| td_string!(l, landing.site.plan_gone))}</span>
                </p>
            </div>
            <p class="lp-plan-note">{tr(|l| td_string!(l, landing.site.plan_note))}</p>
            <p class="lp-plan-fine">{fine}</p>
        </div>
    }
}
