//! The payers' hall (.plans/045 R4.2, owner's hero of 7 Oct, prototype
//! `bethere-ux/site/site.js` `hall_by_event`): one chair per deposit payer of
//! RTM #1–#6, solid for those who came, an outline for those who did not;
//! each event's payers sit together with its no-shows at the end of its
//! block. Point at, tap or tab to a block and the line under it names the
//! event. The rows are the hand-recorded facts (`domain::models::facts`,
//! R4.7), so the hall, the ladder and the totals have one source.

use leptos::prelude::*;

use crate::i18n::{Locale, td_string, use_i18n};
use crate::locale::tr;
use event_checkin_domain::models::facts::{LADDER, LadderRow, ladder_total};

/// The brand chair (the logo's mark), as in the prototype.
const CHAIR: &str = "M2 0h4v14h14v4H8v12H4V18H2z M18 18h4v12h-4z";
const COLS: usize = 16;
const DX: f32 = 29.0;
const DY: f32 = 37.5;
const X0: f32 = 4.0;
const Y0: f32 = 4.0;

/// One chair of the hall: where it sits, whether its payer came, and the
/// index of its event in [`LADDER`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HallSeat {
    pub x: f32,
    pub y: f32,
    pub came: bool,
    pub group: usize,
}

/// Seat `rows` in order, one block per row, each block's no-shows at its
/// end, `COLS` to a row of chairs. Returns the seats and the drawing's size.
pub fn hall_seats(rows: &[LadderRow]) -> (Vec<HallSeat>, f32, f32) {
    let mut seats = Vec::new();
    let mut i = 0usize;
    for (group, row) in rows.iter().enumerate() {
        for k in 0..row.paid {
            seats.push(HallSeat {
                x: X0 + (i % COLS) as f32 * DX,
                y: Y0 + (i / COLS) as f32 * DY,
                came: k < row.came,
                group,
            });
            i += 1;
        }
    }
    let height = i.div_ceil(COLS) as f32 * DY;
    (seats, COLS as f32 * DX, height)
}

/// "RTM #4 · 16 paid · 14 came", in the reader's language.
fn event_line(row: &LadderRow, locale: Locale) -> String {
    format!(
        "{} · {} {} · {} {}",
        row.event,
        row.paid,
        td_string!(locale, landing.hall.paid),
        row.came,
        td_string!(locale, landing.hall.came)
    )
}

#[component]
pub fn Hall() -> impl IntoView {
    let i18n = use_i18n();
    let (seats, w, h) = hall_seats(&LADDER);
    let (paid, came) = ladder_total();
    // The event under the pointer or focus; `None` shows the hint.
    let picked = RwSignal::new(None::<usize>);
    let groups =
        LADDER
            .iter()
            .enumerate()
            .map(|(g, row)| {
                let chairs = seats
                .iter()
                .filter(|s| s.group == g)
                .map(|s| {
                    let class = if s.came { "lp-hall-came" } else { "lp-hall-gone" };
                    view! {
                        <rect class="lp-hall-hit" x=s.x - 4.0 y=s.y - 4.0 width=DX height=DY />
                        <path class=class transform=format!("translate({} {})", s.x, s.y) d=CHAIR />
                    }
                })
                .collect::<Vec<_>>();
                let row = *row;
                let label = move || event_line(&row, i18n.get_locale());
                view! {
                    <g
                        tabindex="0"
                        role="button"
                        aria-label=label
                        class:lp-hall-on=move || picked.get() == Some(g)
                        on:pointerenter=move |_| picked.set(Some(g))
                        on:focus=move |_| picked.set(Some(g))
                        on:click=move |_| picked.set(Some(g))
                    >
                        {chairs}
                    </g>
                }
            })
            .collect::<Vec<_>>();
    view! {
        <div class="lp-hall-box">
            <svg
                class="lp-hall"
                class:lp-hall-picking=move || picked.get().is_some()
                role="group"
                viewBox=format!("0 0 {w} {h}")
                aria-label=tr(|l| td_string!(l, landing.hall.label))
                on:pointerleave=move |_| picked.set(None)
            >
                {groups}
            </svg>
            <p class="lp-hall-big">
                <b>{format!("{came}/{paid}")}</b>
                <span>{tr(|l| td_string!(l, landing.hall.big))}</span>
            </p>
            <p class="lp-hall-say" aria-live="polite">
                {move || match picked.get() {
                    Some(g) => event_line(&LADDER[g], i18n.get_locale()),
                    None => td_string!(i18n.get_locale(), landing.hall.point).to_string(),
                }}
            </p>
            <p class="lp-hall-fine">{tr(|l| td_string!(l, landing.hall.fine))}</p>
        </div>
    }
}
