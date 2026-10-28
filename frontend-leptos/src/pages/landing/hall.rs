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
pub const CHAIR: &str = "M2 0h4v14h14v4H8v12H4V18H2z M18 18h4v12h-4z";
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
    hall_seats_in(rows, COLS)
}

/// [`hall_seats`] with `cols` chairs to a row: the hero draws the hall wide
/// (a band across the page) or narrow (a phone), the card keeps `COLS`.
pub fn hall_seats_in(rows: &[LadderRow], cols: usize) -> (Vec<HallSeat>, f32, f32) {
    let cols = cols.max(1);
    let mut seats = Vec::new();
    let mut i = 0usize;
    for (group, row) in rows.iter().enumerate() {
        for k in 0..row.paid {
            seats.push(HallSeat {
                x: X0 + (i % cols) as f32 * DX,
                y: Y0 + (i / cols) as f32 * DY,
                came: k < row.came,
                group,
            });
            i += 1;
        }
    }
    let height = i.div_ceil(cols) as f32 * DY;
    (seats, cols as f32 * DX, height)
}

/// How the hall seats its payers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HallLayout {
    /// One block per event, flowing across `cols` chairs to a row (the card).
    #[default]
    Blocks,
    /// One row per event, labelled, its no-shows at the end of the row: the
    /// hall reads as a small chart (the hero on a wide screen, .plans/049).
    ByEvent,
}

/// Room left of a by-event row for its label ("RTM #1"), in drawing units.
pub const ROW_GUTTER: f32 = 64.0;
/// Room right of the widest row for its count ("14/16").
pub const ROW_TAIL: f32 = 56.0;

/// [`HallLayout::ByEvent`]: event `g` is row `g`, its payers left to right
/// with the no-shows last. Returns the seats and the drawing's size.
pub fn hall_seats_by_event(rows: &[LadderRow]) -> (Vec<HallSeat>, f32, f32) {
    let widest = rows.iter().map(|r| r.paid).max().unwrap_or(0) as f32;
    let mut seats = Vec::new();
    for (group, row) in rows.iter().enumerate() {
        for k in 0..row.paid {
            seats.push(HallSeat {
                x: X0 + ROW_GUTTER + k as f32 * DX,
                y: Y0 + group as f32 * DY,
                came: k < row.came,
                group,
            });
        }
    }
    let width = X0 + ROW_GUTTER + widest * DX + ROW_TAIL;
    (seats, width, Y0 + rows.len() as f32 * DY)
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
pub fn Hall(
    #[prop(default = COLS)] cols: usize,
    #[prop(default = HallLayout::Blocks)] layout: HallLayout,
    /// The caveat line under the chairs; the hero draws it beside its words.
    #[prop(default = true)]
    show_fine: bool,
) -> impl IntoView {
    let i18n = use_i18n();
    let (seats, w, h) = match layout {
        HallLayout::Blocks => hall_seats_in(&LADDER, cols),
        HallLayout::ByEvent => hall_seats_by_event(&LADDER),
    };
    let (paid, came) = ladder_total();
    // The widest row ends here; the count of each row sits just after it.
    let widest = LADDER.iter().map(|r| r.paid).max().unwrap_or(0) as f32;
    let tail_x = X0 + ROW_GUTTER + widest * DX + 8.0;
    // The order the chairs fill in when the page opens (.plans/049): those who
    // came, in reading order, then the empty outlines last.
    let came_total = seats.iter().filter(|s| s.came).count();
    let fill_order: Vec<usize> = {
        let (mut next_came, mut next_gone) = (0usize, 0usize);
        seats
            .iter()
            .map(|s| match s.came {
                true => {
                    next_came += 1;
                    next_came - 1
                }
                false => {
                    next_gone += 1;
                    came_total + 8 * (next_gone - 1)
                }
            })
            .collect()
    };
    // The event under the pointer or focus; `None` shows the hint.
    let picked = RwSignal::new(None::<usize>);
    let groups =
        LADDER
            .iter()
            .enumerate()
            .map(|(g, row)| {
                let chairs = seats
                .iter()
                .zip(fill_order.iter())
                .filter(|(s, _)| s.group == g)
                .map(|(s, order)| {
                    let class = if s.came { "lp-hall-came" } else { "lp-hall-gone" };
                    view! {
                        <rect class="lp-hall-hit" x=s.x - 4.0 y=s.y - 4.0 width=DX height=DY />
                        <path
                            class=class
                            style=format!("--i:{order}")
                            transform=format!("translate({} {})", s.x, s.y)
                            d=CHAIR
                        />
                    }
                })
                .collect::<Vec<_>>();
                // By-event rows name themselves and say how many came.
                let marks = (layout == HallLayout::ByEvent).then(|| {
                    let y = Y0 + g as f32 * DY + 21.0;
                    view! {
                        <text class="lp-hall-label" x=X0 y=y>{row.event}</text>
                        <text class="lp-hall-count" x=tail_x y=y>{format!("{}/{}", row.came, row.paid)}</text>
                    }
                });
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
                        {marks}
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
            {show_fine.then(|| view! {
                <p class="lp-hall-fine">{tr(|l| td_string!(l, landing.hall.fine))}</p>
            })}
        </div>
    }
}
