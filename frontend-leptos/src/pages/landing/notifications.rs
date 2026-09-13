use leptos::prelude::*;
use serde::Deserialize;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Deserialize)]
struct InboxItem {
    id: i64,
    title: String,
    body: String,
    event_name: String,
    action_url: String,
    action_label: String,
    read_at: Option<i64>,
}

#[derive(Clone, Default, Deserialize)]
struct InboxPage {
    items: Vec<InboxItem>,
    unread_count: i64,
    next_before: Option<i64>,
}

#[component]
pub(super) fn NotificationInbox() -> impl IntoView {
    let (page, set_page) = signal(None::<InboxPage>);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (before, set_before) = signal(None::<i64>);
    let request_epoch = Arc::new(AtomicU64::new(0));
    let cleanup_epoch = Arc::clone(&request_epoch);
    on_cleanup(move || {
        cleanup_epoch.fetch_add(1, Ordering::Relaxed);
    });
    let request_epoch = StoredValue::new(request_epoch);

    Effect::new(move |_| {
        let cursor = before.get();
        let epoch = request_epoch.get_value();
        let request = epoch.fetch_add(1, Ordering::Relaxed) + 1;
        set_busy.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            let query = cursor
                .map(|value| format!("?before={value}"))
                .unwrap_or_default();
            let result =
                crate::api::api_get_json::<InboxPage>(&format!("/my-notifications{query}")).await;
            if epoch.load(Ordering::Relaxed) != request {
                return;
            }
            match result {
                Ok(mut value) => {
                    if cursor.is_some() {
                        set_page.update(|current| {
                            if let Some(current) = current {
                                current.items.append(&mut value.items);
                                current.unread_count = value.unread_count;
                                current.next_before = value.next_before;
                            } else {
                                *current = Some(value);
                            }
                        });
                    } else {
                        set_page.set(Some(value));
                    }
                }
                Err(error) => set_error.set(Some(error.to_string())),
            }
            set_busy.set(false);
        });
    });

    let read_all = move |_| {
        if busy.get_untracked() {
            return;
        }
        set_busy.set(true);
        set_error.set(None);
        let epoch = request_epoch.get_value();
        let request = epoch.fetch_add(1, Ordering::Relaxed) + 1;
        leptos::task::spawn_local(async move {
            let result = crate::api::api_post_json::<serde_json::Value>(
                "/my-notifications/read-all",
                &serde_json::json!({}),
            )
            .await;
            if epoch.load(Ordering::Relaxed) != request {
                return;
            }
            match result {
                Ok(_) => {
                    set_page.update(|value| {
                        if let Some(value) = value {
                            value.unread_count = 0;
                            for item in &mut value.items {
                                item.read_at = Some(1);
                            }
                        }
                    });
                }
                Err(error) => set_error.set(Some(error.to_string())),
            }
            set_busy.set(false);
        });
    };

    let load_older = move |_| {
        if busy.get_untracked() {
            return;
        }
        if let Some(cursor) = page.get_untracked().and_then(|value| value.next_before) {
            set_before.set(Some(cursor));
        }
    };

    move || {
        let value = page.get();
        let unread_count = value.as_ref().map_or(0, |value| value.unread_count);
        let has_more = value.as_ref().and_then(|value| value.next_before).is_some();
        let items = value.map_or_else(Vec::new, |value| value.items);
        view! {
            <section class="attendee-inbox" aria-labelledby="attendee-inbox-title">
                <div class="attendee-inbox-header">
                    <div>
                        <h2 id="attendee-inbox-title" class="landing-reg-title">"Notifications"</h2>
                        <p class="attendee-inbox-summary">
                            {if unread_count == 0 { "You're all caught up.".to_string() } else { format!("{unread_count} unread") }}
                        </p>
                    </div>
                    {if unread_count > 0 { Some(view! {
                        <button class="btn btn-outline btn-xs" disabled=move || busy.get() on:click=read_all>
                            {move || if busy.get() { "Saving…" } else { "Mark all read" }}
                        </button>
                    }) } else { None }}
                </div>
                <Show when=move || error.get().is_some()>
                    <p role="alert">{move || error.get()}</p>
                </Show>
                <Show when=move || busy.get() && page.get().is_none()>
                    <p role="status">"Loading notifications…"</p>
                </Show>
                <Show when=move || !busy.get() && error.get().is_none() && page.get().is_some_and(|value| value.items.is_empty())>
                    <p>"No notifications yet."</p>
                </Show>
                <div class="attendee-inbox-list">
                    {items.into_iter().map(|item| {
                        let unread = item.read_at.is_none();
                        let id = item.id;
                        let mark_read = move |_| {
                            if busy.get_untracked() {
                                return;
                            }
                            set_busy.set(true);
                            set_error.set(None);
                            let epoch = request_epoch.get_value();
                            let request = epoch.fetch_add(1, Ordering::Relaxed) + 1;
                            leptos::task::spawn_local(async move {
                                let path = format!("/my-notifications/{id}/read");
                                let result = crate::api::api_post_json::<serde_json::Value>(
                                    &path,
                                    &serde_json::json!({}),
                                )
                                .await;
                                if epoch.load(Ordering::Relaxed) != request {
                                    return;
                                }
                                match result {
                                    Ok(_) => set_page.update(|page| if let Some(page) = page
                                            && let Some(item) = page.items.iter_mut().find(|item| item.id == id)
                                            && item.read_at.is_none()
                                        {
                                            item.read_at = Some(1);
                                            page.unread_count = page.unread_count.saturating_sub(1);
                                        }),
                                    Err(error) => set_error.set(Some(error.to_string())),
                                }
                                set_busy.set(false);
                            });
                        };
                        view! {
                            <article class="attendee-inbox-item" class:attendee-inbox-item--unread=unread>
                                <div class="attendee-inbox-copy">
                                    <span class="attendee-inbox-event">{item.event_name}</span>
                                    <h3>{item.title}</h3>
                                    <p>{item.body}</p>
                                </div>
                                <div class="attendee-inbox-actions">
                                    {unread.then(|| view! { <button class="btn btn-outline btn-xs" disabled=move || busy.get() on:click=mark_read>"Mark read"</button> })}
                                    <a class="btn btn-primary btn-sm" href=item.action_url>{item.action_label}" →"</a>
                                </div>
                            </article>
                        }
                    }).collect::<Vec<_>>()}
                </div>
                {has_more.then(|| view! {
                    <button class="btn btn-outline btn-sm" disabled=move || busy.get() on:click=load_older>
                        {move || if busy.get() { "Loading…" } else { "Older notifications" }}
                    </button>
                })}
            </section>
        }
    }
}
