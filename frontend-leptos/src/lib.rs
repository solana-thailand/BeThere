pub mod api;
pub mod auth;
pub mod components;
pub mod icons;
pub mod pages;
pub mod utils;
pub mod wallet;
pub mod wallet_error;

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::ProtectedRoute;
use crate::icons::{Icon, IconName};

/// Build marker exposed to JS and logged to the console once at app boot.
///
/// Deliberately declared as a wasm-bindgen `inline_js` import rather than a
/// plain Rust const. Two purposes:
/// 1. **Diagnostics** — confirms *which* frontend bundle is actually live in a
///    given browser, invaluable when chasing stale-cache / CDN issues.
/// 2. **Cache-bust that reaches the JS glue** — Cloudflare Workers Assets
///    dedupes by *content* hash of each file. The wasm-bindgen JS glue
///    (`event-checkin-frontend-<hash>.js`) changes ONLY when the bindgen
///    interface (an import/export) changes — bumping a plain Rust const changes
///    the WASM but leaves the glue byte-identical, so a poisoned glue object on
///    the CDN keeps being served. Declaring the tag as an `inline_js` import
///    adds a binding to the glue, so bumping `BUILD_TAG` below yields a NEW glue
///    content hash that was never uploaded (and so never poisoned). See the
///    2026-07-26 octet-stream incident.
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function __bethere_build_tag() { return '2026-07-26-1'; }")]
extern "C" {
    fn __bethere_build_tag() -> String;
}
use crate::pages::{
    admin::Admin, adventure::page::Adventure, claim::Claim, dashboard_live::DashboardLive,
    data_privacy::DataPrivacy, deposit::Deposit, dev_dashboard::DevDashboard,
    dev_profile::DevProfile, event_summary::EventSummary, landing::Landing, login::Login,
    pr_pack::PrPack, privacy::Privacy, public_event::PublicEvent, scanner::Scanner,
    ticket::page::Ticket, EventRecap, PastEvents, PostEventRegister,
};

/// Main application component.
///
/// Sets up the Leptos router with routes:
/// - `/` — Landing page (public marketing page)
/// - `/login` — Login page (Google OAuth sign-in)
/// - `/claim/:token` — NFT claim page for attendees
/// - `/staff` — Staff scanner page (QR code scanning + manual check-in)
/// - `/admin` — Admin dashboard (stats, attendee list, QR generation)
///
/// Protected routes (`/staff`, `/admin`) are wrapped in `ProtectedRoute`,
/// which handles auth checking, token capture from URL, and user email loading.
#[component]
pub fn App() -> impl IntoView {
    // Register Mobile Wallet Adapter once at app boot.
    // No-op on non-Android platforms. After registration, MWA wallets
    // (Phantom, Solflare, Seed Vault) appear in the Wallet Standard registry
    // and are picked up by existing deposit/claim/escrow wallet detection.
    // See `.plans/011_solana_mobile_demo_day.md`.
    wallet::init_mobile_wallet_adapter();

    // Log the live build tag to the console (side effect — never optimized out;
    // the __bethere_build_tag import also forces a fresh JS-glue content hash).
    leptos::logging::log!("[bethere] frontend build {}", __bethere_build_tag());

    view! {
        <Router>
            <Title text="BeThere — Event Check-In" />
            <main>
                <Routes fallback=|| {
                    view! {
                        <div class="center-page">
                            <div class="container layout-col-center">
                                <div class="logo"><Icon icon=IconName::Search class="icon-xl" /></div>
                                <h1>"Page Not Found"</h1>
                                <p class="subtitle">"The page you're looking for doesn't exist."</p>
                                <a href="/" class="btn btn-primary">"Go Home"</a>
                            </div>
                        </div>
                    }
                }>
                    <Route path=path!("/") view=Landing />
                    <Route path=path!("/login") view=Login />
                    <Route path=path!("/claim/:token") view=Claim />
                    <Route path=path!("/deposit/:attendee_id") view=Deposit />
                    <Route path=path!("/ticket/:attendee_id") view=Ticket />
                    <Route path=path!("/e/:slug") view=PublicEvent />
                    // Public past-events feed + recap pages (Plan 008 — Phase 2).
                    // `/past-events` lists completed events with a published recap;
                    // `/events/:slug/recap` renders one published recap.
                    <Route path=path!("/past-events") view=PastEvents />
                    <Route path=path!("/events/:slug/recap") view=EventRecap />
                    // Post-event lead-capture form (Plan 008 — Phase 3).
                    // JWT-gated inside the component (redirects to /login if no
                    // session) — same self-gate pattern as the dev-profile page.
                    <Route path=path!("/events/:slug/post-event-register") view=PostEventRegister />
                    <Route path=path!("/privacy") view=Privacy />
                    <Route path=path!("/data-privacy") view=DataPrivacy />
                    <Route path=path!("/adventure") view=Adventure />
                    <Route path=path!("/dashboard") view=DevDashboard />
                    <Route path=path!("/profile") view=DevProfile />
                    <Route path=path!("/staff") view=ProtectedScanner />
                    <Route path=path!("/admin") view=ProtectedAdmin />
                    <Route path=path!("/dashboard/live") view=ProtectedLiveDashboard />
                    <Route path=path!("/events/:id/summary") view=ProtectedEventSummary />
                    <Route path=path!("/events/:id/pr-pack") view=ProtectedPrPack />
                </Routes>
            </main>
        </Router>
    }
}

/// Protected wrapper for the Scanner page.
///
/// Nests the Scanner component inside `ProtectedRoute`, which handles:
/// - Capturing OAuth tokens from URL params
/// - Redirecting to `/login` if not authenticated
/// - Loading user email via `GET /api/auth/me`
/// - Providing `ReadSignal<String>` via context
#[component]
fn ProtectedScanner() -> impl IntoView {
    view! {
        <ProtectedRoute>
            <Scanner />
        </ProtectedRoute>
    }
}

/// Protected wrapper for the Admin page.
///
/// Same auth guard as `ProtectedScanner`, but for the Admin dashboard.
#[component]
fn ProtectedAdmin() -> impl IntoView {
    view! {
        <ProtectedRoute>
            <Admin />
        </ProtectedRoute>
    }
}

/// Protected wrapper for the live aggregate dashboard.
///
/// Same auth guard as `ProtectedAdmin`. The live dashboard is the big-screen
/// view for the in-room demo — staff JWT is enforced before mount so a
/// projector mishap can't leak attendee data to the room.
#[component]
fn ProtectedLiveDashboard() -> impl IntoView {
    view! {
        <ProtectedRoute>
            <DashboardLive />
        </ProtectedRoute>
    }
}

/// Protected wrapper for the post-event summary page.
///
/// Same auth guard as `ProtectedAdmin`. Organizer-only view of the frozen
/// funnel + financials snapshot; the freeze mutation is also organizer+
/// (enforced server-side), so wrapping in `ProtectedRoute` prevents a Staff
/// user from even loading the page and seeing the summary data.
#[component]
fn ProtectedEventSummary() -> impl IntoView {
    view! {
        <ProtectedRoute>
            <EventSummary />
        </ProtectedRoute>
    }
}

/// Protected wrapper for the PR Pack page (Plan 008 Phase 4).
///
/// Organizer-only view of generated marketing copy. The backend enforces the
/// role gate too, so this wrapper just prevents a Staff user from loading the
/// page UI before the API rejects them.
#[component]
fn ProtectedPrPack() -> impl IntoView {
    view! {
        <ProtectedRoute>
            <PrPack />
        </ProtectedRoute>
    }
}
