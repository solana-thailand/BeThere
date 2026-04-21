pub mod api;
pub mod auth;
pub mod pages;

use leptos::prelude::*;
use leptos_meta::{MetaTags, Title};
use leptos_router::components::{Route, Router, Routes};

use crate::pages::{admin::Admin, login::Login, scanner::Scanner};

/// Main application component.
///
/// Sets up the Leptos router with three routes:
/// - `/` — Login page (Google OAuth sign-in)
/// - `/staff` — Staff scanner page (QR code scanning + manual check-in)
/// - `/admin` — Admin dashboard (stats, attendee list, QR generation)
#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <Title text="Event Check-In" />
            <main>
                <Routes fallback=|| {
                    view! {
                        <div class="center-page">
                            <div class="container" style="display:flex;flex-direction:column;align-items:center;">
                                <div class="logo">"🔍"</div>
                                <h1>"Page Not Found"</h1>
                                <p class="subtitle">"The page you're looking for doesn't exist."</p>
                                <a href="/" class="btn btn-primary">"Go to Login"</a>
                            </div>
                        </div>
                    }
                }>
                    <Route path="/" view=Login />
                    <Route path="/staff" view=Scanner />
                    <Route path="/admin" view=Admin />
                </Routes>
            </main>
        </Router>
    }
}
