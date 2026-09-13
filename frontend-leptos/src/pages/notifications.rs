//! On-demand, organizer-only delivery history for an event.
use crate::api;
use leptos::prelude::*;
use serde::Deserialize;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Default, Deserialize)]
struct DeliveryPage {
    items: Vec<Delivery>,
    next_before: Option<i64>,
}
#[derive(Clone, Deserialize)]
struct Delivery {
    id: i64,
    kind: String,
    status: String,
    attempts: i32,
    error_code: Option<String>,
    recipient_name: Option<String>,
    recipient_email: Option<String>,
}

#[component]
pub fn NotificationPanel(event_id: String) -> impl IntoView {
    let event_id = StoredValue::new(event_id);
    let (open, set_open) = signal(false);
    let (busy, set_busy) = signal(false);
    let (page, set_page) = signal(DeliveryPage::default());
    let (error, set_error) = signal(None::<String>);
    let (before, set_before) = signal(None::<i64>);
    let (refresh, set_refresh) = signal(0u32);
    // Invalidates late responses on close, a newer request, or component disposal.
    let epoch = Arc::new(AtomicU64::new(0));
    let cleanup_epoch = epoch.clone();
    on_cleanup(move || {
        cleanup_epoch.fetch_add(1, Ordering::Relaxed);
    });
    let request_epoch = StoredValue::new(epoch);
    Effect::new(move |_| {
        let epoch = request_epoch.get_value();
        let request = epoch.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = refresh.get();
        let cursor = before.get();
        if !open.get() {
            set_busy.set(false);
            return;
        }
        set_busy.set(true);
        set_error.set(None);
        let id = event_id.get_value();
        leptos::task::spawn_local(async move {
            let query = cursor.map(|v| format!("?before={v}")).unwrap_or_default();
            let result = api::api_get_json::<DeliveryPage>(&format!(
                "/events/{}/notifications{query}",
                urlencoding::encode(&id)
            ))
            .await;
            if epoch.load(Ordering::Relaxed) != request {
                return;
            }
            match result {
                Ok(data) => set_page.set(data),
                Err(e) => set_error.set(Some(e.to_string())),
            }
            set_busy.set(false);
        });
    });
    view! {
        <div class="notification-panel">
            <button class="btn btn-outline btn-sm" aria-expanded=move || open.get().to_string()
                on:click=move |_|set_open.update(|v|*v = !*v)>"Notifications"</button>
            <Show when=move ||open.get()>
                <div class="notification-panel-body">
                    <h3>"Attendee notifications"</h3>
                    <p>"Accepted means the email provider accepted the message; inbox delivery is not confirmed. Uncertain attempts need review in the provider logs before any resend."</p>
                    <div class="notification-panel-actions">
                        <button class="btn btn-outline btn-sm" disabled=move ||busy.get() on:click=move |_|{if busy.get_untracked(){return;}set_before.set(None);set_refresh.update(|v|*v+=1);}>"Refresh latest"</button>
                        <button class="btn btn-outline btn-sm" disabled=move ||busy.get() ||page.get().next_before.is_none() on:click=move |_|{if !busy.get_untracked(){set_before.set(page.get().next_before);}}>"Older messages"</button>
                    </div>
                    <div role="status" aria-live="polite">{move || if busy.get(){"Loading…"}else{""}}</div>
                    <Show when=move ||error.get().is_some()><p role="alert">{move ||error.get()}</p></Show>
                    <Show when=move ||!busy.get() &&error.get().is_none() &&page.get().items.is_empty()><p>"No notifications yet. Messages are created for new registrations with a verified email address."</p></Show>
                    <p class="notification-scroll-hint">"Swipe across the table to see status and retry actions."</p>
                    <div class="notification-table-scroll">
                        <table class="notification-table">
                            <thead><tr><th>"Attendee"</th><th>"Message"</th><th>"Status"</th><th>"Attempts"</th><th>"Action"</th></tr></thead>
                            <tbody>{move ||page.get().items.into_iter().map(|item|{
                                let id=item.id;
                                let can_retry=item.status=="failed";
                                let name=item.recipient_name.unwrap_or_default();
                                let email=item.recipient_email.unwrap_or_default();
                                let status=match item.status.as_str(){"uncertain"=>"Needs review","pending"=>"Queued","accepted"=>"Accepted","sending"=>"Sending","failed"=>"Failed","cancelled"=>"Cancelled",_=>"Unknown"};
                                let kind=match item.kind.as_str(){"registration"=>"Registration","reminder"=>"Event reminder","deposit_confirmed"=>"Deposit confirmed","deposit_rejected"=>"Slip rejected",_=>"Notification"};
                                view! {<tr>
                                    <td><span>{name}</span><br/><span>{email}</span></td><td>{kind}</td><td>{status}<br/><small>{item.error_code.unwrap_or_default()}</small></td><td>{item.attempts}</td>
                                    <td>{if can_retry {view!{<button class="btn btn-outline btn-sm" disabled=move ||busy.get() on:click=move |_|{
                                        if busy.get_untracked(){return;}
                                        set_busy.set(true);set_error.set(None);
                                        let event=event_id.get_value();
                                        let epoch=request_epoch.get_value();
                                        let request=epoch.fetch_add(1,Ordering::Relaxed)+1;
                                        leptos::task::spawn_local(async move {
                                            let result=api::api_post_json::<serde_json::Value>(&format!("/events/{}/notifications/retry",urlencoding::encode(&event)),&serde_json::json!({"notification_id":id})).await;
                                            if epoch.load(Ordering::Relaxed)!=request {return;}
                                            match result {
                                                Ok(_)=>set_refresh.update(|v|*v+=1),
                                                Err(e)=>set_error.set(Some(e.to_string())),
                                            }
                                            set_busy.set(false);
                                        });
                                    }>"Retry"</button>}.into_any()}else{view!{<span>"—"</span>}.into_any()}}</td>
                                </tr>}
                            }).collect_view()}</tbody>
                        </table>
                    </div>
                </div>
            </Show>
        </div>
    }
}
