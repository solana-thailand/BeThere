//! Route views for the staff/organizer pages (.issues/169).
//!
//! The frontend ships as two Trunk builds of this crate:
//! - the **attendee shell** (`dist/index.html`, default features), which every
//!   attendee downloads;
//! - the **staff shell** (`dist/staff-app.html`, `--features staff`), the full
//!   app including the scanner, admin and organizer pages.
//!
//! The route list in `lib.rs` is the same in both. In the staff build these
//! views wrap the real pages in `ProtectedRoute`. In the attendee build they
//! all resolve to [`StaffShellHandoff`], so the linker drops the staff pages
//! from the attendee wasm. The edge serves the staff shell for these paths via
//! the `_redirects` 200 rewrites; keep that file in step with [`STAFF_PATHS`].

/// Path patterns served by the staff shell, in `_redirects` placeholder syntax.
/// `tests/staff_shell_split.rs` pins `_redirects` to this list.
pub const STAFF_PATHS: [&str; 5] = [
    "/staff",
    "/admin",
    "/dashboard/live",
    "/events/:id/summary",
    "/events/:id/pr-pack",
];

#[cfg(feature = "staff")]
pub use staff::{
    ProtectedAdmin, ProtectedEventSummary, ProtectedLiveDashboard, ProtectedPrPack,
    ProtectedScanner,
};

#[cfg(not(feature = "staff"))]
pub use attendee::{
    StaffShellHandoff as ProtectedAdmin, StaffShellHandoff as ProtectedEventSummary,
    StaffShellHandoff as ProtectedLiveDashboard, StaffShellHandoff as ProtectedPrPack,
    StaffShellHandoff as ProtectedScanner,
};

/// Records the path the page booted on. Call once, before the router mounts.
pub fn record_boot_path() {
    #[cfg(not(feature = "staff"))]
    attendee::record_boot_path();
}

#[cfg(feature = "staff")]
mod staff {
    use crate::components::ProtectedRoute;
    use crate::pages::{
        admin::Admin, dashboard_live::DashboardLive, event_summary::EventSummary, pr_pack::PrPack,
        scanner::Scanner,
    };
    use leptos::prelude::*;

    /// Staff scanner. `ProtectedRoute` captures OAuth tokens from the URL,
    /// redirects to `/login` when signed out and provides the user email.
    #[component]
    pub fn ProtectedScanner() -> impl IntoView {
        view! {
            <ProtectedRoute>
                <Scanner />
            </ProtectedRoute>
        }
    }

    /// Admin dashboard, behind the same auth guard as the scanner.
    #[component]
    pub fn ProtectedAdmin() -> impl IntoView {
        view! {
            <ProtectedRoute>
                <Admin />
            </ProtectedRoute>
        }
    }

    /// Big-screen live dashboard for the in-room demo. The staff JWT is
    /// enforced before mount so a projector mishap can't leak attendee data.
    #[component]
    pub fn ProtectedLiveDashboard() -> impl IntoView {
        view! {
            <ProtectedRoute>
                <DashboardLive />
            </ProtectedRoute>
        }
    }

    /// Organizer-only post-event summary. The freeze mutation is organizer+
    /// server-side too; the guard keeps Staff from loading the page at all.
    #[component]
    pub fn ProtectedEventSummary() -> impl IntoView {
        view! {
            <ProtectedRoute>
                <EventSummary />
            </ProtectedRoute>
        }
    }

    /// Organizer-only PR Pack (Plan 008 Phase 4). The backend enforces the
    /// role gate; this only stops Staff from loading the page UI first.
    #[component]
    pub fn ProtectedPrPack() -> impl IntoView {
        view! {
            <ProtectedRoute>
                <PrPack />
            </ProtectedRoute>
        }
    }
}

#[cfg(not(feature = "staff"))]
mod attendee {
    use leptos::prelude::*;
    use leptos_router::hooks::use_location;
    use std::cell::OnceCell;

    thread_local! {
        static BOOT_PATH: OnceCell<String> = const { OnceCell::new() };
    }

    pub fn record_boot_path() {
        let path = web_sys::window()
            .and_then(|w| w.location().pathname().ok())
            .unwrap_or_default();
        BOOT_PATH.with(|p| {
            let _ = p.set(path);
        });
    }

    /// A staff path reached inside the attendee shell.
    ///
    /// After a client-side navigation (an `<A>` or `use_navigate`) a full
    /// page load of the same URL asks the edge again and gets the staff
    /// shell. The router renders the route before it updates
    /// `window.location`, so the target comes from the router, not the window.
    /// If the page *booted* on this path, the edge served the attendee shell
    /// for it (a missing `_redirects` rule, or the service worker's offline
    /// fallback); navigating again would loop, so say so instead.
    #[component]
    pub fn StaffShellHandoff() -> impl IntoView {
        let location = use_location();
        let path = location.pathname.get_untracked();
        let booted_here = BOOT_PATH.with(|p| p.get().is_some_and(|b| *b == path));
        if !booted_here && let Some(w) = web_sys::window() {
            // The router keeps `search` without its `?`, and `hash` with its `#`.
            let search = location.search.get_untracked();
            let query = if search.is_empty() {
                String::new()
            } else {
                format!("?{search}")
            };
            let hash = location.hash.get_untracked();
            let target = format!("{path}{query}{hash}");
            let _ = w.location().set_href(&target);
        }
        view! {
            <div class="center-page">
                <div class="container layout-col-center">
                    <p class="subtitle">
                        {if booted_here { "The staff app could not be loaded. Check your connection and try again." } else { "Opening the staff app…" }}
                    </p>
                </div>
            </div>
        }
    }
}
