//! Shared UI components extracted from page modules.
//!
//! Provides reusable Toast notifications, AppHeader, and ProtectedRoute wrapper
//! to eliminate code duplication between scanner and admin pages.

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;

/// Whether a user has admin privileges.
///
/// Provided via context by `ProtectedRoute`. Child components can access
/// it via `use_context::<ReadSignal<bool>>()` or by reading the `is_admin`
/// prop passed down through page components.
pub fn is_admin_role(role: &str) -> bool {
    role.eq_ignore_ascii_case("admin")
}

// ===== Toast Notification =====

/// Toast notification severity type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastType {
    Success,
    Error,
    Warning,
    Info,
}

/// Toast notification message payload.
#[derive(Clone, Debug)]
pub struct ToastMessage {
    pub text: String,
    pub toast_type: ToastType,
}

/// Show a toast notification that auto-dismisses after 4 seconds.
///
/// Call this from any event handler or async callback to display feedback.
pub fn show_toast(
    set_toast: &WriteSignal<Option<ToastMessage>>,
    text: &str,
    toast_type: ToastType,
) {
    set_toast.set(Some(ToastMessage {
        text: text.to_string(),
        toast_type,
    }));

    let set_toast = *set_toast;
    set_timeout(
        move || {
            set_toast.set(None);
        },
        std::time::Duration::from_secs(4),
    );
}

/// Toast notification component.
///
/// Renders a fixed-position notification at top-right that auto-dismisses.
/// Bind to a signal: `<Toast toast_signal=toast />`
#[component]
pub fn Toast(toast_signal: ReadSignal<Option<ToastMessage>>) -> impl IntoView {
    view! {
        <Show
            when=move || toast_signal.get().is_some()
            fallback=|| view! { <div></div> }
        >
            {move || {
                let msg = toast_signal.get();
                match msg {
                    Some(m) => {
                        let bg_style = match m.toast_type {
                            ToastType::Success => "background:rgba(34,197,94,0.15);border:1px solid rgba(34,197,94,0.4);color:#22c55e;",
                            ToastType::Error => "background:rgba(239,68,68,0.15);border:1px solid rgba(239,68,68,0.4);color:#ef4444;",
                            ToastType::Warning => "background:rgba(245,158,11,0.15);border:1px solid rgba(245,158,11,0.4);color:#f59e0b;",
                            ToastType::Info => "background:rgba(59,130,246,0.15);border:1px solid rgba(59,130,246,0.4);color:#3b82f6;",
                        };
                        let full_style = format!(
                            "position:fixed;top:1rem;right:1rem;padding:0.85rem 1.25rem;border-radius:8px;font-size:0.9rem;font-weight:500;z-index:9999;max-width:360px;{bg_style}",
                        );
                        view! {
                            <div style=full_style>
                                {m.text}
                            </div>
                        }
                            .into_any()
                    }
                    None => view! { <div></div> }.into_any(),
                }
            }}
        </Show>
    }
}

// ===== App Header =====

/// Shared application header component.
///
/// Displays a page title, user email, role-based navigation, and sign-out button.
/// Eliminates the duplicated header markup between Scanner and Admin pages.
#[component]
pub fn AppHeader(
    /// Title text displayed in the header (e.g. "🎫 Scanner").
    #[prop(into)]
    title: String,
    /// Reactive signal containing the current user's email.
    user_email: ReadSignal<String>,
    /// Reactive signal containing the current user's role ("admin" or "staff").
    user_role: ReadSignal<String>,
    /// Callback invoked when the user clicks "Sign Out".
    on_sign_out: impl Fn(web_sys::MouseEvent) + 'static,
) -> impl IntoView {
    view! {
        <header class="header">
            <div class="header-inner">
                <span class="header-title">{title}</span>
                <div style="display:flex;align-items:center;gap:0.75rem;">
                    // Staff ↔ Admin navigation — admin link only shown for admin role
                    <nav style="display:flex;gap:0.25rem;">
                        <A href="/staff" attr:class="btn btn-outline btn-sm">
                            "📷 Scanner"
                        </A>
                        {move || {
                            let role = user_role.get();
                            if is_admin_role(&role) {
                                view! {
                                    <A href="/admin" attr:class="btn btn-outline btn-sm">
                                        "📊 Admin"
                                    </A>
                                }
                                    .into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }
                        }}
                    </nav>
                    <span class="header-user">{move || user_email.get()}</span>
                    <button class="btn btn-outline btn-sm" on:click=on_sign_out>
                        "Sign Out"
                    </button>
                </div>
            </div>
        </header>
    }
}

// ===== Protected Route =====

/// Auth guard wrapper component.
///
/// On mount:
/// - Checks `is_authenticated()` and redirects to `/` if not
/// - Loads user email + role via `GET /api/auth/me`
/// - Provides the user email and role to children via context
///
/// Children can access the user email and role via:
/// ```ignore
/// let user_email = use_context::<ReadSignal<String>>().expect("user_email in context");
/// let user_role = use_context::<ReadSignal<String>>().expect("user_role in context");
/// ```
#[component]
pub fn ProtectedRoute(children: Children) -> impl IntoView {
    let navigate = use_navigate();
    let (user_email, set_user_email) = signal(String::new());
    let (user_role, set_user_role) = signal(String::new());

    // On mount: check auth via cookie by calling /api/auth/me
    // Renders children immediately; redirects to / if cookie is invalid.
    Effect::new(move |_| {
        let nav = navigate.clone();
        leptos::task::spawn_local(async move {
            match crate::api::get_me().await {
                Ok(me) => {
                    log::info!(
                        "[protected] authenticated as {} (role: {})",
                        me.email,
                        me.role
                    );
                    set_user_email.set(me.email);
                    set_user_role.set(me.role);
                }
                Err(e) => {
                    log::warn!("[protected] auth check failed: {e}");
                    nav("/login", Default::default());
                }
            }
        });
    });

    // Provide user_email and user_role via context for child components
    provide_context(user_email);
    provide_context(user_role);

    children()
}
