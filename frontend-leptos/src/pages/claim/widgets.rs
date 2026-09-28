//! Interactive widgets (client-side only).

use leptos::prelude::*;

use super::helpers::*;
use crate::i18n::{t_string, use_i18n};

/// `SessionTimer` phase codes. Internal state only, never shown: the view
/// turns them into words in the reader's language.
const PHASE_STARTS: &str = "starts";
const PHASE_LIVE: &str = "live";
const PHASE_ENDED: &str = "ended";

// ---------------------------------------------------------------------------
// Interactive widgets (client-side only)
// ---------------------------------------------------------------------------

/// Floating hearts widget — audience taps to send hearts.
/// Purely cosmetic, client-side only. Hearts float up and fade out.
#[component]
pub(super) fn HeartsWidget() -> impl IntoView {
    let (hearts, set_hearts) = signal(Vec::<u32>::new());
    let (count, set_count) = signal(0u32);
    let heart_id = std::cell::Cell::new(0u32);

    let send_heart = move |_: web_sys::MouseEvent| {
        let id = heart_id.get();
        heart_id.set(id + 1);
        set_hearts.update(|h| h.push(id));
        set_count.update(|c| *c += 1);

        // Remove heart after animation (3 seconds)
        let set_h = set_hearts;
        set_timeout(
            move || {
                set_h.update(|h| h.retain(|&x| x != id));
            },
            std::time::Duration::from_secs(3),
        );
    };

    view! {
        <div class="hearts-widget">
            <button class="heart-btn" on:click=send_heart>
                <svg viewBox="0 0 24 24" width="28" height="28">
                    <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" fill="#ef4444"/>
                </svg>
                <span class="heart-count">{move || count.get()}</span>
            </button>
            <div class="hearts-container">
                {move || hearts.get().iter().map(|&id| {
                    let left = (id % 5) as f64 * 15.0 + 10.0;
                    let delay = (id % 3) as f64 * 0.2;
                    let style = format!(
                        "left:{}%;animation-delay:{}s;",
                        left, delay
                    );
                    view! {
                        <span class="floating-heart" style=style>
                            <svg viewBox="0 0 24 24" width="20" height="20">
                                <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" fill="#ef4444"/>
                            </svg>
                        </span>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

/// Live session timer showing event progress.
/// Shows elapsed time since event start or countdown to start.
#[component]
pub(super) fn SessionTimer(start_ms: i64, end_ms: i64) -> impl IntoView {
    let event_start_ms = start_ms as f64;
    let event_end_ms = end_ms as f64;

    // Seconds to the start (before) or since the start (live).
    let (time_display, set_time_display) = signal(0i64);
    // One of the `PHASE_*` codes; empty until the first tick.
    let (status_label, set_status_label) = signal(String::new());

    Effect::new(move |_| {
        let set_t = set_time_display;
        let set_s = set_status_label;

        leptos::task::spawn_local(async move {
            loop {
                let now = js_sys::Date::now();
                // `try_set` hands the value back once the timer is unmounted;
                // stop then, or every remount stacks another loop until the
                // event ends.
                let (label, value) = if now < event_start_ms {
                    let diff = ((event_start_ms - now) / 1000.0) as i64;
                    (PHASE_STARTS, diff)
                } else if now < event_end_ms {
                    let diff = ((now - event_start_ms) / 1000.0) as i64;
                    (PHASE_LIVE, diff)
                } else {
                    let _ = set_s.try_set(PHASE_ENDED.to_string());
                    break; // stop polling after event ends
                };
                if set_s.try_set(label.to_string()).is_some() || set_t.try_set(value).is_some() {
                    break;
                }
                // 5s interval — reduces re-renders vs 1s; sufficient granularity
                // for countdown/elapsed display on event time scales (hours).
                gloo_timers::future::TimeoutFuture::new(5000).await;
            }
        });
    });

    let i18n = use_i18n();
    let label = move || match status_label.get().as_str() {
        PHASE_STARTS => t_string!(i18n, claim.timer.starts_in),
        PHASE_LIVE => t_string!(i18n, claim.timer.live),
        PHASE_ENDED => t_string!(i18n, claim.timer.ended),
        _ => "",
    };
    let value = move || {
        let secs = time_display.get();
        let locale = i18n.get_locale();
        match status_label.get().as_str() {
            PHASE_STARTS => format_duration(secs, locale),
            PHASE_LIVE => format!("+{}", format_duration(secs, locale)),
            PHASE_ENDED => t_string!(i18n, claim.timer.thanks).to_string(),
            _ => String::new(),
        }
    };
    view! {
        <div class="session-timer">
            <span class="timer-label">{label}</span>
            <span class="timer-value">{value}</span>
        </div>
    }
}

/// Generative pixel art avatar — 8x8 grid with face features.
/// Deterministic from name hash: each person gets a unique cute face.
#[component]
pub(super) fn ParticipantAvatar(name: String) -> impl IntoView {
    let hash = simple_hash(&name);

    let skin_hues = [30, 25, 35, 20, 40, 28];
    let skin_hue = skin_hues[(hash % 6) as usize];
    let skin_lightness = 70 + (hash % 15);
    let face_color = format!("hsl({skin_hue}, 60%, {skin_lightness}%)");

    let eye_style = (hash / 6) % 4;
    let mouth_style = (hash / 24) % 4;
    let has_blush = (hash / 96).is_multiple_of(3);
    let bg_hue = (hash / 288) % 360;
    let bg_color = format!("hsl({bg_hue}, 50%, 25%)");

    let grid = build_face_grid(eye_style, mouth_style, has_blush);

    let svg_cells = grid
        .iter()
        .enumerate()
        .flat_map(|(row, cells)| {
            let fc = face_color.clone();
            cells.iter().enumerate().filter_map(move |(col, &cell)| {
            if cell == 0 { return None; }
            let color = match cell {
                1 => fc.clone(),
                2 => "#1a1a2e".to_string(),
                3 => "#e74c3c".to_string(),
                4 => "rgba(255,150,150,0.6)".to_string(),
                _ => "#333".to_string(),
            };
            Some(format!(
                "<rect x=\"{x}\" y=\"{y}\" width=\"1\" height=\"1\" fill=\"{color}\" rx=\"0.15\"/>",
                x = col,
                y = row,
                color = color
            ))
        }).collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .join("");

    view! {
        <div class="participant-avatar-pixel" style=format!("background:{bg_color};")>
            <svg viewBox="0 0 8 8" width="56" height="56" class="claim-avatar-svg" inner_html=svg_cells></svg>
        </div>
    }
}

/// NFT badge preview placeholder — shows a stylized mystery badge card
/// until real NFT artwork is uploaded. Pure CSS/SVG, no external image.
#[component]
pub(super) fn NftBadgePreview() -> impl IntoView {
    view! {
        <div class="nft-preview-card">
            <div class="nft-preview-badge">
                <svg viewBox="0 0 80 80" width="80" height="80">
                    // Outer hexagon
                    <polygon
                        points="40,4 72,22 72,58 40,76 8,58 8,22"
                        fill="none"
                        stroke="rgba(99,102,241,0.4)"
                        stroke-width="1.5"
                    />
                    // Inner diamond
                    <polygon
                        points="40,16 60,40 40,64 20,40"
                        fill="rgba(99,102,241,0.08)"
                        stroke="rgba(99,102,241,0.25)"
                        stroke-width="1"
                    />
                    // Center star
                    <circle cx="40" cy="40" r="6" fill="rgba(99,102,241,0.5)" />
                    <circle cx="40" cy="40" r="3" fill="rgba(129,140,248,0.8)" />
                </svg>
            </div>
            <div class="nft-preview-info">
                <div class="nft-preview-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.proof_of_attendance))}</div>
                <div class="nft-preview-sub">{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.preview_sub))}</div>
            </div>
        </div>
    }
}

/// Build an 8x8 face grid with symmetric features.
pub(super) fn build_face_grid(eye_style: u32, mouth_style: u32, has_blush: bool) -> [[u8; 8]; 8] {
    let mut grid: [[u8; 8]; 8] = [
        [0, 0, 1, 1, 1, 1, 0, 0],
        [0, 1, 1, 1, 1, 1, 1, 0],
        [1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 1, 1],
        [0, 1, 1, 1, 1, 1, 1, 0],
        [0, 0, 1, 1, 1, 1, 0, 0],
    ];

    // Eyes (symmetric)
    match eye_style {
        0 => {
            grid[3][2] = 2;
            grid[3][5] = 2;
        } // dot eyes
        1 => {
            grid[2][2] = 2;
            grid[2][5] = 2;
            grid[3][2] = 2;
            grid[3][5] = 2;
        } // tall eyes
        2 => {
            grid[3][2] = 2;
            grid[3][3] = 2;
            grid[3][4] = 2;
            grid[3][5] = 2;
        } // wide eyes
        _ => {
            grid[2][2] = 2;
            grid[3][3] = 2;
            grid[2][5] = 2;
            grid[3][4] = 2;
        } // anime eyes
    }

    // Mouth (centered)
    match mouth_style {
        0 => {
            grid[5][3] = 3;
            grid[5][4] = 3;
        } // small smile
        1 => {
            grid[5][2] = 3;
            grid[5][3] = 3;
            grid[5][4] = 3;
            grid[5][5] = 3;
        } // wide smile
        2 => {
            grid[5][3] = 3;
            grid[5][4] = 3;
            grid[6][3] = 3;
            grid[6][4] = 3;
        } // open mouth
        _ => {
            grid[4][4] = 3;
            grid[5][3] = 3;
            grid[5][4] = 3;
        } // smirk
    }

    // Blush
    if has_blush {
        grid[4][1] = 4;
        grid[4][6] = 4;
    }

    grid
}
