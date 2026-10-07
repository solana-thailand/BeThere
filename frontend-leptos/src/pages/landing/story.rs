//! The commitment ladder (.plans/043 L8): one room of chairs, RTM #1's 45
//! in-person registrants, narrowed in three steps (registered → confirmed →
//! paid a deposit) while the share who came climbs; then RTM #1–#6 per event.
//!
//! These are historical records, not live figures, and several were never in
//! D1 (RTM #1 predates BeThere; RTM #3's deposits were purged before the
//! archive existed; one RTM #2 payer kept a standing balance outside D1). So
//! the table is typed data here, every row with its source and the date it
//! was measured, and the total is summed from the rows, never typed in.
//! Definition (owner, 6 Oct 2026): in-person registrants who paid cash or
//! credit; comp, staff and online payers are out; came = checked in. The live
//! "paid / came" strip (`stats.rs`) uses the same definition on what the
//! system recorded.

use leptos::html::Div;
use leptos::prelude::*;

use crate::i18n::{Locale, td_string};
use crate::locale::tr;

use super::hero::MarkupText;
use super::sofar::{reduced_motion, watch_first_sight};

/// Where a row's numbers come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LadderSource {
    /// The organizer's sheet, before BeThere took deposits (confirmed 3 Oct).
    HandRecord,
    /// BeThere's deposit archive, plus one payer the owner records by hand.
    ArchiveAndOwnerRecord,
    /// The digest-chained deposit statement, paired 1:1 to attendees.
    Statement,
    /// BeThere's live tables.
    System,
}

impl LadderSource {
    pub fn label(self, locale: Locale) -> &'static str {
        match self {
            Self::HandRecord => td_string!(locale, landing.story.src_hand),
            Self::ArchiveAndOwnerRecord => td_string!(locale, landing.story.src_archive_owner),
            Self::Statement => td_string!(locale, landing.story.src_statement),
            Self::System => td_string!(locale, landing.story.src_system),
        }
    }
}

/// One event's payers and how many of them came.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LadderRow {
    pub event: &'static str,
    pub paid: u32,
    pub came: u32,
    pub source: LadderSource,
    /// `YYYY-MM-DD`: when this row was last checked against its source.
    pub measured_at: &'static str,
}

/// RTM #1–#6 under the owner's definition (`.plans/043`, 6 Oct 2026; RTM #6
/// added 8 Oct, `.plans/044` item 4).
pub const LADDER: [LadderRow; 6] = [
    LadderRow {
        event: "RTM #1",
        paid: 16,
        came: 16,
        source: LadderSource::HandRecord,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #2",
        paid: 15,
        came: 15,
        source: LadderSource::ArchiveAndOwnerRecord,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #3",
        paid: 12,
        came: 12,
        source: LadderSource::Statement,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #4",
        paid: 16,
        came: 14,
        source: LadderSource::System,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #5",
        paid: 14,
        came: 13,
        source: LadderSource::System,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #6",
        paid: 21,
        came: 20,
        source: LadderSource::System,
        measured_at: "2026-10-08",
    },
];

/// `(paid, came)` summed over [`LADDER`].
pub fn ladder_total() -> (u32, u32) {
    LADDER
        .iter()
        .fold((0, 0), |(p, c), r| (p + r.paid, c + r.came))
}

/// `came` of `n` as a whole percent, rounded half up; 0 when nobody counted.
pub fn percent(came: u32, n: u32) -> u32 {
    match n {
        0 => 0,
        _ => (200 * came + n) / (2 * n),
    }
}

/// One group of RTM #1's room (hand record, confirmed 3 Oct 2026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomGroup {
    pub n: u32,
    pub came: u32,
}

/// Paid a deposit; confirmed without paying; never confirmed.
pub const ROOM: [RoomGroup; 3] = [
    RoomGroup { n: 16, came: 16 },
    RoomGroup { n: 14, came: 9 },
    RoomGroup { n: 15, came: 0 },
];

/// How far the room is narrowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Registered,
    Confirmed,
    Paid,
}

impl Stage {
    pub const ALL: [Stage; 3] = [Stage::Registered, Stage::Confirmed, Stage::Paid];

    /// How many of [`ROOM`]'s groups are still in the room.
    pub fn groups(self) -> usize {
        match self {
            Self::Registered => 3,
            Self::Confirmed => 2,
            Self::Paid => 1,
        }
    }

    /// `(seats, came)` in the room at this stage.
    pub fn counts(self) -> (u32, u32) {
        ROOM[..self.groups()]
            .iter()
            .fold((0, 0), |(n, c), g| (n + g.n, c + g.came))
    }

    fn next(self) -> Option<Stage> {
        match self {
            Self::Registered => Some(Self::Confirmed),
            Self::Confirmed => Some(Self::Paid),
            Self::Paid => None,
        }
    }

    fn button(self, locale: Locale) -> &'static str {
        match self {
            Self::Registered => td_string!(locale, landing.story.stage_registered),
            Self::Confirmed => td_string!(locale, landing.story.stage_confirmed),
            Self::Paid => td_string!(locale, landing.story.stage_paid),
        }
    }

    fn caption(self, locale: Locale) -> &'static str {
        match self {
            Self::Registered => td_string!(locale, landing.story.cap_registered),
            Self::Confirmed => td_string!(locale, landing.story.cap_confirmed),
            Self::Paid => td_string!(locale, landing.story.cap_paid),
        }
    }
}

/// How long each stage holds before the room narrows on its own.
const STAGE_MS: u64 = 2600;
/// A chair: back, seat, two legs (24 × 40 before scaling).
const CHAIR: &str = "M0 0h6v20h18v6h-3v14h-5v-14h-10v14h-5v-14h-1z";

/// A fixed pseudo-random in [0, 1): the same scatter on every load.
fn jitter(i: usize) -> f64 {
    let v = ((i as f64 + 1.0) * 12.9898).sin() * 43758.5453;
    v - v.floor()
}

/// One chair: which group it belongs to, whether that person came, and where
/// it stands.
struct Seat {
    group: usize,
    came: bool,
    transform: String,
}

fn seats() -> Vec<Seat> {
    let mut people: Vec<(usize, bool)> = ROOM
        .iter()
        .enumerate()
        .flat_map(|(k, g)| (0..g.n).map(move |i| (k, i < g.came)))
        .collect();
    // Mixed, not grouped: the room does not sort itself by who paid.
    let mut order: Vec<usize> = (0..people.len()).collect();
    order.sort_by(|&a, &b| jitter(a + 99).total_cmp(&jitter(b + 99)));
    people = order.into_iter().map(|i| people[i]).collect();
    people
        .into_iter()
        .enumerate()
        .map(|(pos, (group, came))| {
            let x = 16.0 + (pos % 8) as f64 * 62.0 + (jitter(pos) - 0.5) * 16.0;
            let y = 14.0 + (pos / 8) as f64 * 74.0 + (jitter(pos + 7) - 0.5) * 14.0;
            let r = (jitter(pos + 3) - 0.5) * 22.0;
            Seat {
                group,
                came,
                transform: format!("translate({x:.1} {y:.1}) rotate({r:.1} 12 20) scale(1.3)"),
            }
        })
        .collect()
}

#[component]
pub fn Story() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let stage = RwSignal::new(Stage::Registered);
    let room = NodeRef::<Div>::new();
    let seen = RwSignal::new(false);
    let touched = RwSignal::new(false);
    watch_first_sight(room, seen);

    // Narrow the room on its own once it is seen, unless the reader has
    // picked a stage or asked for no motion.
    let timer = StoredValue::new(None::<TimeoutHandle>);
    let schedule = move || {
        let handle = set_timeout_with_handle(
            move || {
                if touched.get_untracked() {
                    return;
                }
                if let Some(next) = stage.get_untracked().next() {
                    stage.set(next);
                }
            },
            std::time::Duration::from_millis(STAGE_MS),
        );
        timer.set_value(handle.ok());
    };
    Effect::new(move |_| {
        let current = stage.get();
        if !seen.get() || touched.get() || reduced_motion() || current.next().is_none() {
            return;
        }
        schedule();
    });
    on_cleanup(move || {
        timer.with_value(|h| {
            if let Some(h) = h {
                h.clear();
            }
        });
    });

    let fill = move |template: &str, n: u32, came: u32| {
        crate::locale::fill(
            template,
            &[("n", &n.to_string()), ("came", &came.to_string())],
        )
    };
    let pct = move || {
        let (n, came) = stage.get().counts();
        percent(came, n).to_string()
    };
    let caption = move || {
        let s = stage.get();
        let (n, came) = s.counts();
        fill(s.caption(i18n.get_locale()), n, came)
    };
    let (paid, came) = ladder_total();
    let all_paid = Signal::derive(move || {
        crate::locale::fill(
            td_string!(i18n.get_locale(), landing.story.all_paid),
            &[("came", &came.to_string()), ("paid", &paid.to_string())],
        )
    });
    let definition = move || td_string!(i18n.get_locale(), landing.story.definition);

    let stage_buttons = Stage::ALL
        .into_iter()
        .map(|s| {
            let (n, came) = s.counts();
            view! {
                <button type="button" class="lp-stage"
                    aria-pressed=move || (stage.get() == s).to_string()
                    on:click=move |_| {
                        touched.set(true);
                        stage.set(s);
                    }>
                    {move || fill(s.button(i18n.get_locale()), n, came)}
                </button>
            }
        })
        .collect::<Vec<_>>();

    let chairs = seats()
        .into_iter()
        .map(|seat| {
            let group = seat.group;
            view! {
                <path class="lp-seat" class:lp-seat-empty=!seat.came
                    class:lp-seat-out=move || { group >= stage.get().groups() }
                    d=CHAIR transform=seat.transform />
            }
        })
        .collect::<Vec<_>>();

    let rows = LADDER
        .into_iter()
        .map(|r| {
            view! {
                <tr>
                    <th scope="row">{r.event}</th>
                    <td>{r.paid}</td>
                    <td>{r.came}</td>
                    <td>{move || format!("{} · {}", r.source.label(i18n.get_locale()), r.measured_at)}</td>
                </tr>
            }
        })
        .collect::<Vec<_>>();

    view! {
        <section id="story" class="lp-story">
            <div class="lp-wrap lp-story-grid">
                <div>
                    <h2>{tr(|l| td_string!(l, landing.story.title))}</h2>
                    <p class="lp-pct" aria-live="polite"><span>{pct}</span>"%"</p>
                    <p class="lp-cap2">{caption}</p>
                    <div class="lp-stages" role="group"
                        aria-label=tr(|l| td_string!(l, landing.story.stages_aria))>
                        {stage_buttons}
                    </div>
                    <p class="lp-fineprint">{tr(|l| td_string!(l, landing.story.room_source))}</p>
                    <p class="lp-allpaid"><MarkupText text=all_paid /></p>
                    <details class="lp-ladder">
                        <summary>{tr(|l| td_string!(l, landing.story.per_event))}</summary>
                        <table>
                            <thead>
                                <tr>
                                    <th scope="col">{tr(|l| td_string!(l, landing.story.col_event))}</th>
                                    <th scope="col">{tr(|l| td_string!(l, landing.story.col_paid))}</th>
                                    <th scope="col">{tr(|l| td_string!(l, landing.story.col_came))}</th>
                                    <th scope="col">{tr(|l| td_string!(l, landing.story.col_source))}</th>
                                </tr>
                            </thead>
                            <tbody>{rows}</tbody>
                        </table>
                        <p class="lp-fineprint">{definition}</p>
                    </details>
                    <p class="lp-fineprint">{tr(|l| td_string!(l, landing.story.caveat))}</p>
                </div>
                <div class="lp-chairs" node_ref=room>
                    <svg viewBox="0 0 520 440" role="img"
                        aria-label=tr(|l| td_string!(l, landing.story.room_aria))>
                        {chairs}
                    </svg>
                </div>
            </div>
        </section>
    }
}
