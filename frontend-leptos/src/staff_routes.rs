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
//!
//! The attendee pages are the mirror case: the edge always serves them from
//! the attendee shell, so the staff shell only reaches one by a client-side
//! navigation (a "Home" link, a ticket link). The staff build hands every
//! attendee route except `/login` back with a full page load, and the linker
//! drops those pages from the staff wasm, which sits near its size ceiling
//! (`scripts/verify/frontend_size_budget.sh`). `/login` stays real because
//! `ProtectedRoute` sends a signed-out visitor there inside the staff shell.
//! The devnet sandbox (`/sandbox`, [`SandboxRoute`]) is one of them.
//! So are the site pages (`/events`, `/organizers`, `/sponsors`) and the
//! `/discover` redirect (`.plans/045` R4.0).

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

#[cfg(feature = "staff")]
pub use staff::{
    AttendeeShellHandoff as Adventure, AttendeeShellHandoff as Claim,
    AttendeeShellHandoff as DataPrivacy, AttendeeShellHandoff as Deposit,
    AttendeeShellHandoff as DevDashboard, AttendeeShellHandoff as DevProfile,
    AttendeeShellHandoff as EventRecap, AttendeeShellHandoff as Faq,
    AttendeeShellHandoff as Feedback, AttendeeShellHandoff as HomeRoute,
    AttendeeShellHandoff as NfcCheckin, AttendeeShellHandoff as PastEvents,
    AttendeeShellHandoff as PostEventRegister, AttendeeShellHandoff as Privacy,
    AttendeeShellHandoff as PublicEvent, AttendeeShellHandoff as SandboxRoute,
    AttendeeShellHandoff as Ticket, AttendeeShellHandoff as DiscoverRedirect,
    AttendeeShellHandoff as EventsRoute, AttendeeShellHandoff as OrganizersRoute,
    AttendeeShellHandoff as SponsorsRoute, AttendeeShellHandoff as UnsubscribeRoute,
    AttendeeShellHandoff as CourseRoute,
};

#[cfg(not(feature = "staff"))]
pub use attendee::{
    StaffShellHandoff as ProtectedAdmin, StaffShellHandoff as ProtectedEventSummary,
    StaffShellHandoff as ProtectedLiveDashboard, StaffShellHandoff as ProtectedPrPack,
    StaffShellHandoff as ProtectedScanner,
};

#[cfg(not(feature = "staff"))]
pub use crate::pages::{
    EventRecap, Feedback, NfcCheckin, PastEvents, PostEventRegister, adventure::page::Adventure,
    claim::Claim, data_privacy::DataPrivacy, deposit::Deposit, dev_dashboard::DevDashboard,
    dev_profile::DevProfile, faq::Faq, landing::Landing as HomeRoute, privacy::Privacy,
    public_event::PublicEvent, ticket::page::Ticket,
};

#[cfg(not(feature = "staff"))]
pub use crate::pages::sandbox::Sandbox as SandboxRoute;

// The Release 4 site pages (.plans/045 R4.0) take the landing's path: the
// attendee shell renders them, the staff shell hands them back.
#[cfg(not(feature = "staff"))]
pub use crate::pages::site::EventsPage as EventsRoute;
#[cfg(not(feature = "staff"))]
pub use crate::pages::site::{
    CoursePage as CourseRoute, DiscoverRedirect, Organizers as OrganizersRoute,
    SponsorsPage as SponsorsRoute, UnsubscribePage as UnsubscribeRoute,
};

pub use handoff::record_boot_path;

#[cfg(feature = "staff")]
mod staff {
    use crate::components::ProtectedRoute;
    use crate::pages::{
        admin::Admin, dashboard_live::DashboardLive, event_summary::EventSummary, pr_pack::PrPack,
        scanner::Scanner,
    };
    use leptos::prelude::*;

    /// An attendee page reached inside the staff shell (a "Home" link, a ticket
    /// link). A full page load of the same URL gets the attendee shell from the
    /// edge.
    #[component]
    pub fn AttendeeShellHandoff() -> impl IntoView {
        super::handoff::shell_handoff(
            "Opening BeThere…",
            "BeThere could not be loaded. Check your connection and try again.",
        )
    }

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

    /// A staff path reached inside the attendee shell.
    #[component]
    pub fn StaffShellHandoff() -> impl IntoView {
        super::handoff::shell_handoff(
            "Opening the staff app…",
            "The staff app could not be loaded. Check your connection and try again.",
        )
    }
}

/// The hand-off between the two shells, shared by both builds.
mod handoff {
    use leptos::prelude::*;
    use leptos_router::hooks::use_location;
    use std::cell::OnceCell;

    thread_local! {
        static BOOT_PATH: OnceCell<String> = const { OnceCell::new() };
    }

    /// Records the path the page booted on. Call once, before the router mounts.
    pub fn record_boot_path() {
        let path = web_sys::window()
            .and_then(|w| w.location().pathname().ok())
            .unwrap_or_default();
        BOOT_PATH.with(|p| {
            let _ = p.set(path);
        });
    }

    /// A path that belongs to the other shell.
    ///
    /// After a client-side navigation (an `<A>` or `use_navigate`) a full
    /// page load of the same URL asks the edge again and gets the right
    /// shell. The router renders the route before it updates
    /// `window.location`, so the target comes from the router, not the window.
    /// If the page *booted* on this path, the edge served this shell for it
    /// (a missing `_redirects` rule, or the service worker's offline
    /// fallback); navigating again would loop, so say so instead.
    pub fn shell_handoff(opening: &'static str, failed: &'static str) -> impl IntoView {
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
                    <p class="subtitle">{if booted_here { failed } else { opening }}</p>
                </div>
            </div>
        }
    }
}
