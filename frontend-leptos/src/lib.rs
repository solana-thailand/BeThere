pub mod api;
pub mod auth;
pub mod components;
pub mod pages;
pub mod utils;

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::ProtectedRoute;
use crate::pages::{admin::Admin, claim::Claim, landing::Landing, login::Login, scanner::Scanner};

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
    view! {
        <Router>
            <Title text="BeThere — Event Check-In" />
            <main>
                <Routes fallback=|| {
                    view! {
                        <div class="center-page">
                            <div class="container" style="display:flex;flex-direction:column;align-items:center;">
                                <div class="logo">"🔍"</div>
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
                    <Route path=path!("/staff") view=ProtectedScanner />
                    <Route path=path!("/admin") view=ProtectedAdmin />
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
