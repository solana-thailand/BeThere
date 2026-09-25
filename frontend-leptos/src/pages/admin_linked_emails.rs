//! Super-admin "Link one person's emails" panel (plan 025 §6.1 / §7.4,
//! `.issues/122`).
//!
//! A returner who registers with a different email from the one holding their
//! rolling credit cannot use it. Self-service linking needs two Google
//! sign-ins; this panel lets a super admin link the emails instead, so they
//! share one balance. Every link or unlink needs a reason, which the server
//! keeps in the global audit log. Rendered only for super admins: the endpoints
//! refuse everyone else.

use leptos::prelude::*;

use crate::api::{self, AdminLinkEmailsRequest, AdminUnlinkEmailRequest};
use crate::components::{self, ToastMessage, ToastType};

#[component]
pub fn AdminLinkedEmails(set_toast: WriteSignal<Option<ToastMessage>>) -> impl IntoView {
    let (is_super_admin, set_is_super_admin) = signal(false);
    leptos::task::spawn_local(async move {
        if let Ok(me) = api::get_me().await {
            set_is_super_admin.set(me.role == "super_admin");
        }
    });

    let (email, set_email) = signal(String::new());
    let (other_email, set_other_email) = signal(String::new());
    let (reason, set_reason) = signal(String::new());
    // The person's emails as last looked up; `None` before any lookup.
    let (linked, set_linked) = signal(None::<Vec<String>>);
    let (pending, set_pending) = signal(false);

    let lookup = move || {
        let who = email.get_untracked().trim().to_lowercase();
        if who.is_empty() {
            return;
        }
        set_pending.set(true);
        leptos::task::spawn_local(async move {
            match api::get_person_emails(&who).await {
                Ok(found) => set_linked.set(Some(found.emails)),
                Err(e) => {
                    set_linked.set(None);
                    components::show_toast(
                        &set_toast,
                        &format!("Lookup failed: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_pending.set(false);
        });
    };

    let link = move |_| {
        let body = AdminLinkEmailsRequest {
            email: email.get_untracked().trim().to_lowercase(),
            other_email: other_email.get_untracked().trim().to_lowercase(),
            reason: reason.get_untracked().trim().to_string(),
        };
        set_pending.set(true);
        leptos::task::spawn_local(async move {
            match api::admin_link_emails(&body).await {
                Ok(result) => {
                    let message = match result.status.as_str() {
                        "already" => "These emails were already linked",
                        _ => "Linked — they now share one credit balance",
                    };
                    components::show_toast(&set_toast, message, ToastType::Success);
                    set_linked.set(Some(result.emails));
                    set_other_email.set(String::new());
                    set_reason.set(String::new());
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Link failed: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_pending.set(false);
        });
    };

    let unlink = move |target: String| {
        let body = AdminUnlinkEmailRequest {
            email: target,
            reason: reason.get_untracked().trim().to_string(),
        };
        set_pending.set(true);
        leptos::task::spawn_local(async move {
            match api::admin_unlink_email(&body).await {
                Ok(_) => {
                    components::show_toast(&set_toast, "Unlinked", ToastType::Success);
                    set_reason.set(String::new());
                    lookup();
                }
                Err(e) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Unlink failed: {e}"),
                        ToastType::Error,
                    );
                    set_pending.set(false);
                }
            }
        });
    };

    let no_reason = move || reason.get().trim().is_empty();

    view! {
        <Show when=move || is_super_admin.get() fallback=|| view! { <span></span> }>
            <div class="card admin-linked-emails">
                <h4>"Link one person's emails"</h4>
                <p class="panel-hint">
                    "Linked emails share one rolling-credit balance. Use this when someone \
                     registered with a different email from the one holding their credit. \
                     Super admin only; every change needs a reason and is kept in the audit log."
                </p>
                <div class="admin-dep-confirm-row">
                    <input
                        type="email"
                        class="form-input dep-input"
                        placeholder="Email to look up"
                        prop:value=move || email.get()
                        on:input=move |ev| {
                            set_email.set(event_target_value(&ev));
                            set_linked.set(None);
                        }
                    />
                    <button
                        class="btn btn-outline btn-sm"
                        disabled=move || pending.get() || email.get().trim().is_empty()
                        on:click=move |_| lookup()
                    >
                        "Look up"
                    </button>
                </div>

                {move || linked.get().map(|emails| {
                    let lone = emails.len() < 2;
                    view! {
                        <div class="panel-hint">
                            {match lone {
                                true => "Not linked to any other email".to_string(),
                                false => format!("Linked emails ({}):", emails.len()),
                            }}
                        </div>
                        {(!lone).then(|| emails.into_iter().map(|e| {
                            let target = e.clone();
                            view! {
                                <div class="admin-dep-confirm-row">
                                    <span>{e}</span>
                                    <button
                                        class="btn btn-danger btn-xs"
                                        disabled=move || pending.get() || no_reason()
                                        title="Needs a reason in the field below"
                                        on:click=move |_| unlink(target.clone())
                                    >
                                        "Unlink"
                                    </button>
                                </div>
                            }
                        }).collect_view())}
                    }
                })}

                <div style="display:flex;flex-direction:column;gap:0.25rem">
                    <input
                        type="email"
                        class="form-input dep-input"
                        placeholder="Other email of the same person"
                        prop:value=move || other_email.get()
                        on:input=move |ev| set_other_email.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class="form-input dep-input"
                        placeholder="Reason (required, kept in the audit log)"
                        prop:value=move || reason.get()
                        on:input=move |ev| set_reason.set(event_target_value(&ev))
                    />
                    <div class="admin-dep-confirm-row">
                        <button
                            class="btn btn-primary btn-sm"
                            disabled=move || {
                                pending.get()
                                    || email.get().trim().is_empty()
                                    || other_email.get().trim().is_empty()
                                    || no_reason()
                            }
                            on:click=link
                        >
                            "Link emails"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}
