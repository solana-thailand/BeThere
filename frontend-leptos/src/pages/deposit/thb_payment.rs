//! THB payment flow views (ThbUploading, ThbUploaded).

use leptos::prelude::*;

use crate::icons::{Icon, IconName};

use super::js_interop;

/// THB uploading spinner view.
pub fn thb_uploading_view() -> AnyView {
    view! {
        <div class="loading visible loading-top">
            <span class="spinner spinner-lg"></span>
            " Uploading slip..."
        </div>
    }
        .into_any()
}

/// THB uploaded successfully view — auto-redirects to ticket page.
pub fn thb_uploaded_view(
    attendee_id: &str,
    event_id: &str,
) -> AnyView {
    let aid = attendee_id.to_string();
    let eid = event_id.to_string();
    leptos::task::spawn_local(async move {
        gloo::timers::future::TimeoutFuture::new(1500).await;
        js_interop::navigate_to(&format!("/ticket/{aid}?event_id={eid}"));
    });
    view! {
        <div class="card dep-card-error">
            <div class="card-header">
                <h2 class="card-title"><Icon icon=IconName::Check class="icon-sm icon-success" />" Slip Uploaded"</h2>
            </div>
            <p class="hint-desc">
                "Your payment slip has been submitted for verification. You'll be notified once it's confirmed."
            </p>
            <span class="badge badge-warning"><Icon icon=IconName::Hourglass class="icon-sm icon-warning" />" Pending Verification"</span>
            <p style="color:var(--text-secondary);font-size:0.8rem;margin-top:0.75rem;">
                "Redirecting to your ticket..."
            </p>
        </div>
    }
        .into_any()
}
