//! Landing-page auth state (same pattern as public_event.rs).



/// Tracks whether the user is signed in on the landing page.
#[derive(Clone, Debug)]
pub(super) enum AuthState {
    Checking,
    SignedIn(String),
    NotSignedIn,
}

/// Navigates to the unified login page (/login) for Google or Solana Wallet authentication.
pub(super) fn trigger_landing_oauth() {
    let window = web_sys::window().expect("no window");
    let _ = window.location().set_href("/login");
}

/// Sign out: clear cookie + reload.
pub(super) fn trigger_landing_signout() {
    leptos::task::spawn_local(async move {
        let _ = crate::api::fetch::post("/api/auth/logout", &[], None).await;
        let window = web_sys::window().expect("no window");
        let _ = window.location().reload();
    });
}
