//! SVG icon components — replaces all emoji usage with crisp, theme-aware SVG icons.
//!
//! Each icon is a small inline SVG rendered via Leptos `view!`.
//! Benefits over emoji:
//! - Consistent rendering across OS/browser
//! - Respects dark theme (no invisible emoji on dark bg)
//! - Pixel-perfect at any size
//! - No external dependencies or network requests

use leptos::prelude::*;

/// Render a small SVG icon as an `<span>` element with the given CSS class.
///
/// Usage in view! macro:
/// ```ignore
/// view! { <Icon icon=IconName::Check class="icon-success" /> }
/// ```
#[component]
pub fn Icon(
    /// Which icon to render.
    icon: IconName,
    /// Optional CSS class for sizing/coloring.
    #[prop(optional, into)]
    class: Option<String>,
) -> impl IntoView {
    let svg = icon.to_svg();
    let class_val = class.unwrap_or_default();
    view! {
        <span class=format!("icon {class_val}") inner_html=svg />
    }
}

/// All available icon names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IconName {
    // Status
    Check,     // ✅
    Cross,     // ❌
    Warning,   // ⚠️
    Hourglass, // ⏳
    Info,      // ℹ️
    Lock,      // 🔒
    Pause,     // ⏸
    Denied,    // ⛔
    Circle,    // ⚪

    // Finance
    Coin,       // 🪙 / 💰
    MoneyWings, // 💸
    Baht,       // ฿
    Recycle,    // ♻️

    // Wallet (official brand logos)
    Wallet,   // 💼 Generic
    Ghost,    // Phantom (official purple logomark)
    Backpack, // Backpack (official red logomark)
    Sun,      // Solflare (official yellow insignia)
    Planet,   // Jupiter (official gradient logomark)

    // Actions
    Link,     // 🔗
    Clip,     // 📎
    Camera,   // 📷
    Copy,     // 📋 Copy
    Flash,    // ⚡
    Sound,    // 🔊
    Globe,    // 🌐
    Chain,    // ⛓
    Save,     // 💾
    Refresh,  // 🔄
    Settings, // ⚙️
    SignOut,  // ↗

    // Objects
    Ticket,        // 🎫
    Trophy,        // 🏆
    Gift,          // 🎁
    Search,        // 🔍
    Chart,         // 📊
    Phone,         // 📱
    Lightbulb,     // 💡
    Calendar,      // 📅
    Pin,           // 📍
    Timer,         // ⏱️
    Map,           // 🗺️
    Star,          // ⭐
    CreditCard,    // 💳
    Target,        // 🎯
    Palette,       // 🎨
    Puzzle,        // 🧩
    Brain,         // 🧠
    TicketFree,    // 🎟️
    QrCode,        // 📱
    Expand,        // ⤢
    AlertTriangle, // ⚠️
    Clock,         // 🕐

    // Nature/Mascot
    Crab, // 🦀

    // Party
    Party, // 🎉

    // Brand
    Solana,
}

impl IconName {
    /// Return the raw SVG markup for this icon.
    ///
    /// All SVGs use `currentColor` so they inherit text color from CSS.
    /// Uses `fill="none" stroke="currentColor"` for outlined icons.
    pub fn to_svg(self) -> &'static str {
        match self {
            // ── Status ────────────────────────────────────────────
            IconName::Check => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6L9 17l-5-5"/></svg>"#
            }
            IconName::Cross => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>"#
            }
            IconName::Warning => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>"#
            }
            IconName::Hourglass => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 3h14M5 21h14M7 3v3a8 8 0 004 7 8 8 0 00-4 7v1M17 3v3a8 8 0 01-4 7 8 8 0 014 7v1"/></svg>"#
            }
            IconName::Info => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>"#
            }
            IconName::Lock => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0110 0v4"/></svg>"#
            }
            IconName::Pause => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>"#
            }
            IconName::Denied => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/></svg>"#
            }
            IconName::Circle => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="9"/></svg>"#
            }

            // ── Finance ────────────────────────────────────────────
            IconName::Coin => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M14.5 9a3.5 3.5 0 00-5 0M9.5 15a3.5 3.5 0 005 0"/><line x1="12" y1="6" x2="12" y2="8"/><line x1="12" y1="16" x2="12" y2="18"/></svg>"#
            }
            IconName::MoneyWings => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 7c4-2 8-2 10 0s6 2 10 0M2 12c4-2 8-2 10 0s6 2 10 0M2 17c4-2 8-2 10 0s6 2 10 0"/><circle cx="12" cy="12" r="3"/></svg>"#
            }
            IconName::Baht => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="9" y1="2" x2="15" y2="2"/><line x1="12" y1="2" x2="12" y2="22"/><path d="M7 7h6a4 4 0 010 8H7"/><path d="M7 15h6a4 4 0 010 8H7"/></svg>"#
            }
            IconName::Recycle => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7 19H4.815a1.83 1.83 0 01-1.57-.881 1.785 1.785 0 01-.004-1.784L7 11m0 8v-8m0 8h5m-5-8l2.5-4.5m0 0L12 2l2.5 4.5m0 0L17 11M7 7l5-5 5 5"/><path d="M17 11l3.76 6.335a1.785 1.785 0 01-.004 1.784 1.83 1.83 0 01-1.57.881H17m0-9v9m0 0h-5"/></svg>"#
            }

            // ── Wallet ─────────────────────────────────────────────
            IconName::Wallet => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="5" width="20" height="14" rx="2"/><path d="M16 12h.01"/><path d="M2 10h20"/></svg>"#
            }
            IconName::Ghost => {
                // Official Phantom logomark — purple #AB9FF2 on transparent
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 593 493"><path d="M70.055 493C145.604 493 202.38 427.297 236.263 375.378C232.142 386.865 229.852 398.351 229.852 409.378C229.852 439.703 247.252 461.297 281.592 461.297C328.753 461.297 379.119 419.946 405.218 375.378C403.386 381.811 402.471 387.784 402.471 393.297C402.471 414.432 414.375 427.757 438.643 427.757C515.108 427.757 592.03 292.216 592.03 173.676C592.03 81.324 545.327 0 428.112 0C222.069 0 0 251.784 0 414.432C0 478.297 34.34 493 70.055 493ZM357.141 163.568C357.141 140.595 369.962 124.514 388.734 124.514C407.049 124.514 419.87 140.595 419.87 163.568C419.87 186.541 407.049 203.081 388.734 203.081C369.962 203.081 357.141 186.541 357.141 163.568ZM455.126 163.568C455.126 140.595 467.947 124.514 486.719 124.514C505.034 124.514 517.855 140.595 517.855 163.568C517.855 186.541 505.034 203.081 486.719 203.081C467.947 203.081 455.126 186.541 455.126 163.568Z" fill="#AB9FF2"/></svg>"##
            }
            IconName::Backpack => {
                // Official Backpack logomark — red #E33E3F on transparent
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 145 120"><path d="M91.8388 50.2642C94.2901 50.2642 95.5157 50.2642 96.277 51.0257C97.0386 51.787 97.0386 53.0127 97.0386 55.4639V58.9304C97.0386 63.8327 97.0386 66.2839 95.5157 67.807C93.9926 69.3299 91.5413 69.3299 86.6391 69.3299H59.7737C54.8714 69.3299 52.4202 69.3299 50.8972 67.807C49.3742 66.2839 49.3743 63.8327 49.3743 58.9304V55.4639C49.3743 53.0127 49.3743 51.787 50.1357 51.0257C50.8972 50.2642 52.1228 50.2642 54.574 50.2642H91.8388Z" fill="#E33E3F"/><path fill-rule="evenodd" clip-rule="evenodd" d="M73.2064 10.1829C97.3855 10.1829 97.0386 26.3018 97.0386 34.015C97.0386 41.7282 97.0386 42.6812 97.0386 42.6812C97.0386 44.117 95.8745 45.2811 94.4387 45.2811H51.9742C50.5383 45.2811 49.3743 44.117 49.3743 42.6812C49.3743 42.6812 49.3743 41.7282 49.3743 34.015C49.3743 26.3018 49.0274 10.1829 73.2064 10.1829ZM73.2064 16.325C69.1381 16.325 65.8401 19.623 65.8401 23.6914C65.8402 27.7595 69.1382 31.0577 73.2064 31.0577C77.2748 31.0577 80.5727 27.7595 80.5727 23.6914C80.5727 19.623 77.2748 16.325 73.2064 16.325Z" fill="#E33E3F"/><path d="M73.1995 0.190674C78.4167 0.190674 82.9839 2.01423 84.1917 4.98397C84.3737 5.49636 84.4645 5.7526 84.3029 5.94285C84.141 6.13309 83.8282 6.07639 83.2022 5.96295C81.5847 5.66977 79.2946 5.46818 77.2784 5.40862C75.9919 5.33948 74.6348 5.30367 73.2068 5.30367C71.7795 5.30367 70.423 5.33955 69.1371 5.40862C67.1113 5.46815 64.8131 5.66971 63.1896 5.95301C62.5771 6.05991 62.2707 6.11327 62.1102 5.92359C61.9497 5.73386 62.0384 5.4823 62.216 4.97931C63.4141 2.01893 67.9837 0.19069 73.1995 0.190674Z" fill="#E33E3F"/></svg>"##
            }
            IconName::Sun => {
                // Official Solflare insignia — yellow #FFEF46 on transparent.
                // viewBox cropped to the path's actual bounds (X:195-595, Y:187-612)
                // to match Phantom's visual weight; the original 0 0 800 800 left
                // ~50% empty padding, making the logo render ~half-size.
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="185 177 420 445"><path d="M195 558.869L216.829 582.205L252.063 549.951L427.263 612C536.44 539.524 595 490.361 595 429.81C595 389.587 571.171 367.258 518.566 349.879L478.874 336.468L587.549 232.196L565.72 208.861L533.469 237.147L381.103 187C333.96 202.378 274.394 247.597 274.394 292.758C274.352 297.952 275.025 303.126 276.394 308.136C237.194 330.477 221.286 351.343 221.286 377.148C221.286 401.478 234.189 425.82 275.389 439.197L308.143 450.127L195 558.869ZM271.434 351.32C271.434 341.899 276.394 332.947 284.84 325.492C293.766 338.4 309.16 349.822 333.48 357.745L386.051 375.124L356.76 403.422L305.16 386.546C281.331 378.543 271.411 366.675 271.411 351.297M314.074 495.357L347.823 463.092L411.343 483.935C444.589 494.854 455.994 509.26 452.486 545.492L314.074 495.357ZM323 243.092L471.914 292.736L439.674 323.525L362.246 297.743C335.457 288.814 326.509 274.408 323.046 244.121L323 243.092ZM396.406 416.307L425.743 388.067L480.337 405.937C516.074 417.793 533.949 439.642 533.949 470.387C533.949 493.722 525.023 509.111 507.16 528.983L501.697 534.939L503.686 521.025C511.64 470.387 496.714 448.595 447.571 432.657L396.509 416.307H396.406Z" fill="#FFEF46"/></svg>"##
            }
            IconName::Planet => {
                // Official Jupiter logomark — gradient green-cyan on transparent
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><path d="M26.1464 199.614C36.7084 214.305 50.2565 226.598 65.9023 235.686C81.5482 244.774 98.9383 250.452 116.932 252.348C107.675 238.417 94.218 225.6 77.4183 215.842C60.6186 206.084 42.8275 200.754 26.1464 199.614Z" fill="url(#jupiter-grad0)"/><path d="M99.9925 176.988C67.6261 158.185 32.5947 153.393 7.52861 161.858C9.94843 169.856 13.1398 177.599 17.0576 184.979C38.8358 184.475 62.6138 190.39 84.7394 203.241C106.865 216.092 123.787 233.828 134.143 253C142.499 252.743 150.811 251.679 158.963 249.822C153.896 223.858 132.35 195.796 99.9925 176.988Z" fill="url(#jupiter-grad1)"/><path d="M254.229 100.663C250.113 83.925 242.668 68.1877 232.336 54.3909C222.004 40.5942 208.997 29.0209 194.093 20.3621C179.189 11.7034 162.693 6.13675 145.59 3.99461C128.487 1.85247 111.128 3.17875 94.5489 7.89426C122.246 11.2839 152.99 21.6814 183.14 39.1972C213.291 56.7129 237.573 78.2665 254.229 100.663Z" fill="url(#jupiter-grad2)"/><path d="M213.93 162.049C199.753 138.505 175.467 115.959 145.55 98.5793C115.632 81.1992 84.0242 71.2719 56.5727 70.6152C32.4219 70.0432 14.296 77.0639 6.85584 89.8723C6.81347 89.9486 6.75415 90.0206 6.70754 90.0969C6.0381 92.4992 5.46187 94.9058 4.93648 97.3209C15.3256 93.2195 27.3629 90.9358 40.7476 90.6816C70.5124 90.1223 103.824 99.6428 134.563 117.502C165.302 135.361 190.093 159.592 204.346 185.717C210.736 197.488 214.723 209.076 216.303 220.147C218.142 218.503 219.947 216.804 221.697 215.037C221.743 214.957 221.773 214.872 221.82 214.787C229.26 201.966 226.383 182.747 213.93 162.049Z" fill="url(#jupiter-grad3)"/><path d="M122.788 137.754C76.9736 111.137 26.3458 106.968 2 125.543C2.04782 131.357 2.49231 137.161 3.3304 142.915C10.4921 140.744 17.8732 139.377 25.3373 138.839C52.5431 136.792 82.5368 144.372 109.755 160.193C136.974 176.014 158.43 198.326 170.132 222.956C173.367 229.703 175.834 236.792 177.488 244.09C182.902 241.967 188.167 239.48 193.245 236.646C197.321 206.292 168.616 164.375 122.788 137.754Z" fill="url(#jupiter-grad4)"/><path d="M237.496 122.641C223.158 99.1218 198.659 76.5132 168.53 59.0187C138.401 41.5241 106.67 31.4401 79.1295 30.6308C58.1352 30.0249 41.8736 35.1135 33.4419 44.7273C68.4522 38.7955 114.631 48.7651 159.391 74.7676C204.15 100.77 235.699 135.954 247.876 169.303C252.05 157.224 248.419 140.577 237.496 122.641Z" fill="url(#jupiter-grad5)"/><defs><linearGradient id="jupiter-grad0" x1="169.969" y1="53.7813" x2="54.0834" y2="253" gradientUnits="userSpaceOnUse"><stop offset="0.0001" stop-color="#C7F284"/><stop offset="1" stop-color="#00BEF0"/></linearGradient><linearGradient id="jupiter-grad1" x1="169.969" y1="53.7813" x2="54.0834" y2="253" gradientUnits="userSpaceOnUse"><stop offset="0.0001" stop-color="#C7F284"/><stop offset="1" stop-color="#00BEF0"/></linearGradient><linearGradient id="jupiter-grad2" x1="169.969" y1="53.7813" x2="54.0834" y2="253" gradientUnits="userSpaceOnUse"><stop offset="0.0001" stop-color="#C7F284"/><stop offset="1" stop-color="#00BEF0"/></linearGradient><linearGradient id="jupiter-grad3" x1="169.969" y1="53.7813" x2="54.0834" y2="253" gradientUnits="userSpaceOnUse"><stop offset="0.0001" stop-color="#C7F284"/><stop offset="1" stop-color="#00BEF0"/></linearGradient><linearGradient id="jupiter-grad4" x1="169.969" y1="53.7813" x2="54.0834" y2="253" gradientUnits="userSpaceOnUse"><stop offset="0.0001" stop-color="#C7F284"/><stop offset="1" stop-color="#00BEF0"/></linearGradient><linearGradient id="jupiter-grad5" x1="169.969" y1="53.7813" x2="54.0834" y2="253" gradientUnits="userSpaceOnUse"><stop offset="0.0001" stop-color="#C7F284"/><stop offset="1" stop-color="#00BEF0"/></linearGradient></defs></svg>"##
            }

            // ── Actions ────────────────────────────────────────────
            IconName::Link => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71"/></svg>"#
            }
            IconName::Clip => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48"/></svg>"#
            }
            IconName::Camera => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M23 19a2 2 0 01-2 2H3a2 2 0 01-2-2V8a2 2 0 012-2h4l2-3h6l2 3h4a2 2 0 012 2z"/><circle cx="12" cy="13" r="4"/></svg>"#
            }
            IconName::Copy => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>"#
            }
            IconName::Flash => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>"#
            }
            IconName::Sound => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07"/></svg>"#
            }
            IconName::Globe => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z"/></svg>"#
            }
            IconName::Chain => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 18L18 6M6 6l12 12"/></svg>"#
            }
            IconName::Save => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>"#
            }
            IconName::Refresh => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>"#
            }
            IconName::Settings => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/></svg>"#
            }
            IconName::SignOut => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>"#
            }

            // ── Objects ────────────────────────────────────────────
            IconName::Ticket => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 9a3 3 0 013-3h14a3 3 0 013 3v0a3 3 0 01-3 3v0a3 3 0 013 3v0a3 3 0 01-3 3H5a3 3 0 01-3-3v0a3 3 0 013-3v0a3 3 0 01-3-3z"/><line x1="13" y1="6" x2="13" y2="18"/></svg>"#
            }
            IconName::Trophy => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9H4.5a2.5 2.5 0 010-5H6"/><path d="M18 9h1.5a2.5 2.5 0 000-5H18"/><path d="M4 22h16"/><path d="M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20 7 22"/><path d="M14 14.66V17c0 .55.47.98.97 1.21C16.15 18.75 17 20 17 22"/><path d="M18 2H6v7a6 6 0 0012 0V2z"/></svg>"#
            }
            IconName::Gift => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 12 20 22 4 22 4 12"/><rect x="2" y="7" width="20" height="5"/><line x1="12" y1="22" x2="12" y2="7"/><path d="M12 7H7.5a2.5 2.5 0 010-5C11 2 12 7 12 7z"/><path d="M12 7h4.5a2.5 2.5 0 000-5C13 2 12 7 12 7z"/></svg>"#
            }
            IconName::Search => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>"#
            }
            IconName::Chart => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/></svg>"#
            }
            IconName::Phone => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="2" width="14" height="20" rx="2" ry="2"/><line x1="12" y1="18" x2="12.01" y2="18"/></svg>"#
            }
            IconName::Lightbulb => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18h6M10 22h4"/><path d="M15.09 14c.18-.98.65-1.74 1.41-2.5A4.65 4.65 0 0018 8 6 6 0 006 8c0 1 .23 2.23 1.5 3.5A4.61 4.61 0 019 14"/></svg>"#
            }
            IconName::Calendar => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg>"#
            }
            IconName::Pin => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0118 0z"/><circle cx="12" cy="10" r="3"/></svg>"#
            }
            IconName::Timer => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="13" r="8"/><path d="M12 9v4l2 2"/><path d="M5 3L2 6"/><path d="M22 6l-3-3"/><line x1="12" y1="2" x2="12" y2="5"/></svg>"#
            }
            IconName::Map => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="1 6 1 22 8 18 16 22 23 18 23 2 16 6 8 2 1 6"/><line x1="8" y1="2" x2="8" y2="18"/><line x1="16" y1="6" x2="16" y2="22"/></svg>"#
            }
            IconName::Star => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>"#
            }
            IconName::CreditCard => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="1" y="4" width="22" height="16" rx="2" ry="2"/><line x1="1" y1="10" x2="23" y2="10"/></svg>"#
            }
            IconName::Target => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="6"/><circle cx="12" cy="12" r="2"/></svg>"#
            }
            IconName::Palette => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r=".5" fill="currentColor"/><circle cx="17.5" cy="10.5" r=".5" fill="currentColor"/><circle cx="8.5" cy="7.5" r=".5" fill="currentColor"/><circle cx="6.5" cy="12.5" r=".5" fill="currentColor"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 011.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z"/></svg>"#
            }
            IconName::Puzzle => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19.439 7.85c-.049.322.059.648.289.878l1.568 1.568c.47.47.706 1.087.706 1.704s-.235 1.233-.706 1.704l-1.611 1.611a.98.98 0 01-.837.276c-.47-.07-.802-.48-.968-.925a2.501 2.501 0 10-3.214 3.214c.446.166.855.497.925.968a.979.979 0 01-.276.837l-1.61 1.61a2.404 2.404 0 01-1.705.707 2.403 2.403 0 01-1.704-.706l-1.568-1.568a1.026 1.026 0 00-.877-.29c-.493.074-.84.504-1.02.968a2.5 2.5 0 11-3.237-3.237c.464-.18.894-.527.967-1.02a1.026 1.026 0 00-.289-.877l-1.568-1.568A2.402 2.402 0 011.198 12c0-.617.236-1.234.706-1.704L3.515 8.685a.98.98 0 01.837-.276c.47.07.802.48.968.925a2.501 2.501 0 103.214-3.214c-.446-.166-.855-.497-.925-.968a.979.979 0 01.276-.837l1.61-1.61a2.404 2.404 0 011.705-.707c.618 0 1.234.236 1.704.706l1.568 1.568c.23.23.556.338.877.29.493-.074.84-.504 1.02-.968a2.5 2.5 0 113.237 3.237c-.464.18-.894.527-.967 1.02z"/></svg>"#
            }
            IconName::Brain => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a7 7 0 017 7c0 2.38-1.19 4.47-3 5.74V17a2 2 0 01-2 2h-4a2 2 0 01-2-2v-2.26C6.19 13.47 5 11.38 5 9a7 7 0 017-7z"/><line x1="9" y1="21" x2="15" y2="21"/></svg>"#
            }
            IconName::TicketFree => {
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 9a3 3 0 013-3h14a3 3 0 013 3v0"/><path d="M2 9v6a3 3 0 003 3h14a3 3 0 003-3V9"/><line x1="9" y1="6" x2="9" y2="18"/><text x="14" y="14" font-size="7" fill="currentColor" stroke="none" font-weight="bold" font-family="sans-serif">FREE</text></svg>"##
            }
            IconName::QrCode => {
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="8" height="8" rx="1"/><rect x="14" y="2" width="8" height="8" rx="1"/><rect x="2" y="14" width="8" height="8" rx="1"/><rect x="14" y="14" width="4" height="4" rx="0.5"/><rect x="20" y="14" width="2" height="2"/><rect x="14" y="20" width="2" height="2"/><rect x="20" y="20" width="2" height="2"/></svg>"##
            }
            IconName::Expand => {
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 3 21 3 21 9"/><polyline points="9 21 3 21 3 15"/><line x1="21" y1="3" x2="14" y2="10"/><line x1="3" y1="21" x2="10" y2="14"/></svg>"##
            }
            IconName::AlertTriangle => {
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>"##
            }
            IconName::Clock => {
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>"##
            }

            // ── Mascot ─────────────────────────────────────────────
            IconName::Crab => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 9V4M8 6l4 3M16 6l-4 3"/><path d="M7 12h-3a1 1 0 01-1-1v-1M17 12h3a1 1 0 001-1v-1"/><path d="M9 15l-2 3M15 15l2 3"/><circle cx="9" cy="11" r=".5" fill="currentColor"/><circle cx="15" cy="11" r=".5" fill="currentColor"/></svg>"#
            }

            // ── Party ──────────────────────────────────────────────
            IconName::Party => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>"#
            }

            // ── Brand ──────────────────────────────────────────────
            IconName::Solana => {
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 397 311"><path d="M64.6 237.9c2.4-2.4 5.7-3.8 9.2-3.8h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1l62.7-62.7z" fill="currentColor"/><path d="M64.6 3.8C67.1 1.4 70.4 0 73.8 0h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1L64.6 3.8z" fill="currentColor"/><path d="M333.1 120.1c-2.4-2.4-5.7-3.8-9.2-3.8H6.5c-5.8 0-8.7 7-4.6 11.1l62.7 62.7c2.4 2.4 5.7 3.8 9.2 3.8h317.4c5.8 0 8.7-7 4.6-11.1l-62.7-62.7z" fill="currentColor"/></svg>"##
            }
        }
    }
}

/// Convenience: get wallet icon name from wallet name string.
pub fn wallet_icon_name(wallet_name: &str) -> IconName {
    match wallet_name {
        "Phantom" => IconName::Ghost,
        "Backpack" => IconName::Backpack,
        "Solflare" => IconName::Sun,
        "Jupiter" => IconName::Planet,
        "Coinbase" => IconName::Coin,
        _ => IconName::Wallet,
    }
}
