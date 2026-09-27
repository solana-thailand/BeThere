//! Admin dashboard actions: each spawns one API call (or a batch) and
//! reports the outcome through a toast.

use std::collections::HashSet;

use leptos::prelude::*;

use super::csv::download_csv;
use super::types::Notify;
use crate::api::{self, GenerateQrData};
use crate::components::ToastType;

/// Spawn QR code generation task.
pub(super) fn spawn_qr_generation(
    force: bool,
    event_id: Option<String>,
    set_qr_generating: WriteSignal<bool>,
    set_qr_result: WriteSignal<Option<GenerateQrData>>,
    notify: Notify,
) {
    set_qr_generating.set(true);
    leptos::task::spawn_local(async move {
        match api::generate_qrs(force, event_id.as_deref()).await {
            Ok(data) => {
                let count = data.generated;
                let skipped = data.skipped;
                let msg = if skipped > 0 {
                    format!("Generated {count} QR codes ({skipped} skipped)")
                } else {
                    format!("Generated {count} QR codes")
                };
                notify.toast(&msg, ToastType::Success);
                set_qr_result.set(Some(data));
                api::invalidate_attendee_cache();
                // Refresh attendee list after generation
                notify.reload();
            }
            Err(err) => {
                log::error!("[admin] QR generation failed: {err}");
                notify.toast(&format!("QR generation failed: {err}"), ToastType::Error);
            }
        }
        set_qr_generating.set(false);
    });
}

/// Flush the worker's attendee and column-mapping caches, then reload.
pub(super) fn spawn_flush_cache(
    event_id: Option<String>,
    set_busy: WriteSignal<bool>,
    notify: Notify,
) {
    set_busy.set(true);
    leptos::task::spawn_local(async move {
        match api::flush_cache(event_id.as_deref()).await {
            Ok(_) => {
                notify.toast(
                    "Cache flushed — attendee list & column mapping refreshed",
                    ToastType::Success,
                );
                notify.reload();
            }
            Err(e) => {
                notify.toast(&format!("Flush failed: {}", e.message), ToastType::Error);
            }
        }
        set_busy.set(false);
    });
}

/// Toast for a batch: success when nothing failed, warning otherwise.
fn batch_toast(notify: Notify, failed: u32, partial: String, complete: String) {
    match failed {
        0 => notify.toast(&complete, ToastType::Success),
        _ => notify.toast(&partial, ToastType::Warning),
    }
}

/// Check in every selected attendee, one request each.
pub(super) fn spawn_bulk_check_in(
    ids: Vec<String>,
    event_id: Option<String>,
    set_busy: WriteSignal<bool>,
    set_selected: WriteSignal<HashSet<String>>,
    notify: Notify,
) {
    set_busy.set(true);
    leptos::task::spawn_local(async move {
        let mut succeeded = 0u32;
        let mut failed = 0u32;

        for id in ids {
            match api::check_in(&id, event_id.as_deref(), false).await {
                Ok(_) => succeeded += 1,
                Err(e) => {
                    failed += 1;
                    log::warn!("[admin] bulk check-in failed for {id}: {e}");
                }
            }
        }

        batch_toast(
            notify,
            failed,
            format!("Checked in {succeeded}, {failed} failed"),
            format!("Checked in {succeeded} attendees"),
        );
        api::invalidate_attendee_cache();
        set_selected.set(HashSet::new());
        notify.reload();
        set_busy.set(false);
    });
}

/// Set a manual refund status on every selected attendee. `body` is the
/// shared request; it is sent once per id.
pub(super) fn spawn_manual_refund(
    ids: Vec<String>,
    body: api::ManualRefundRequest,
    set_busy: WriteSignal<bool>,
    set_show_form: WriteSignal<bool>,
    set_selected: WriteSignal<HashSet<String>>,
    notify: Notify,
) {
    set_busy.set(true);
    leptos::task::spawn_local(async move {
        let mut succeeded = 0u32;
        let mut failed = 0u32;

        log::info!(
            "[admin] manual refund: sending {} requests, status={}, link={:?}",
            ids.len(),
            body.refund_status,
            body.refund_link
        );

        for id in ids {
            match api::mark_manual_refund(&id, &body).await {
                Ok(resp) => {
                    succeeded += 1;
                    log::info!("[admin] manual refund OK for {id}: {resp:?}");
                }
                Err(e) => {
                    failed += 1;
                    log::warn!("[admin] manual refund failed for {id}: {e}");
                }
            }
        }

        batch_toast(
            notify,
            failed,
            format!("Set refund status for {succeeded}, {failed} failed"),
            format!("Set refund status for {succeeded} attendees"),
        );
        api::invalidate_attendee_cache();
        set_selected.set(HashSet::new());
        notify.reload();
        set_show_form.set(false);
        set_busy.set(false);
    });
}

/// Download the walk-in CSV for one event.
pub(super) fn spawn_walkin_export(event_id: String, set_busy: WriteSignal<bool>, notify: Notify) {
    set_busy.set(true);
    leptos::task::spawn_local(async move {
        match api::export_walkin_csv(&event_id).await {
            Ok(data) => {
                download_csv(&data.filename, &data.csv);
                notify.toast(
                    &format!("Exported {} walk-in attendees", data.count),
                    ToastType::Success,
                );
            }
            Err(e) => {
                notify.toast(&format!("Walk-in export failed: {e}"), ToastType::Error);
            }
        }
        set_busy.set(false);
    });
}

/// Download the cross-event audience CSV (ALL events).
///
/// This is the unique cross-event view: deduped by email across every event,
/// with per-email participation metrics (events joined, check-ins, etc.).
/// It intentionally does NOT scope to the currently selected event — the
/// per-event detail is already covered by the "Export CSV" button.
/// No event needs to be selected.
pub(super) fn spawn_audience_export(set_busy: WriteSignal<bool>, notify: Notify) {
    set_busy.set(true);
    leptos::task::spawn_local(async move {
        // None ⇒ aggregate across ALL events (matches the backend default).
        match api::export_audience_csv(None).await {
            Ok(data) => match (data.filename.as_deref(), data.csv.as_deref()) {
                (Some(f), Some(c)) => {
                    download_csv(f, c);
                    notify.toast(
                        &format!("Exported {} distinct emails", data.total),
                        ToastType::Success,
                    );
                    // Orphan event_id warning — attendees from
                    // unregistered events appear here but can't be
                    // selected in the per-event admin dashboard.
                    if !data.unregistered_event_ids.is_empty() {
                        let n = data.unregistered_event_ids.len();
                        let preview = data
                            .unregistered_event_ids
                            .iter()
                            .take(3)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ");
                        let more = if n > 3 {
                            format!(" (+{} more)", n - 3)
                        } else {
                            String::new()
                        };
                        notify.toast(
                            &format!(
                                "{n} unregistered event(s) not in the event selector: \
                                 {preview}{more}. Their attendees are visible here \
                                 but not in per-event views.",
                            ),
                            ToastType::Warning,
                        );
                    }
                }
                _ => {
                    notify.toast(
                        "Audience export returned no CSV payload",
                        ToastType::Warning,
                    );
                }
            },
            Err(e) => {
                notify.toast(&format!("Audience export failed: {e}"), ToastType::Error);
            }
        }
        set_busy.set(false);
    });
}

/// Append this event's walk-ins to its Google Sheet.
pub(super) fn spawn_walkin_sync(
    event_id: String,
    set_busy: WriteSignal<bool>,
    set_result: WriteSignal<Option<api::WalkinSyncResponse>>,
    notify: Notify,
) {
    set_busy.set(true);
    set_result.set(None);
    leptos::task::spawn_local(async move {
        match api::sync_walkins(&event_id).await {
            Ok(data) => {
                let msg = if data.errors.is_empty() {
                    format!(
                        "Synced {} walk-in attendees ({} already synced)",
                        data.synced, data.skipped
                    )
                } else {
                    format!(
                        "Synced {} of {} walk-ins ({} errors)",
                        data.synced,
                        data.total_walkins,
                        data.errors.len()
                    )
                };
                let toast_type = if data.errors.is_empty() {
                    ToastType::Success
                } else {
                    ToastType::Warning
                };
                notify.toast(&msg, toast_type);
                set_result.set(Some(data));
                api::invalidate_attendee_cache();
                notify.reload();
            }
            Err(e) => {
                notify.toast(&format!("Walk-in sync failed: {e}"), ToastType::Error);
            }
        }
        set_busy.set(false);
    });
}

/// Spend an attendee's rolling credit on this event. `busy` holds the ids
/// with a request in flight so their row buttons stay disabled.
pub(super) fn spawn_apply_credit(
    attendee_id: String,
    event_id: Option<String>,
    set_busy: WriteSignal<HashSet<String>>,
    notify: Notify,
) {
    set_busy.update(|ids| {
        ids.insert(attendee_id.clone());
    });
    leptos::task::spawn_local(async move {
        let body = api::ApplyCreditRequest {
            event_id: event_id.unwrap_or_default(),
        };
        match api::apply_credit(&attendee_id, &body).await {
            Ok(_) => {
                api::invalidate_attendee_cache();
                notify.reload();
                notify.toast(
                    "Rolling credit applied \u{2014} registration completed",
                    ToastType::Success,
                );
            }
            Err(e) => {
                notify.toast(&format!("Apply credit failed: {e}"), ToastType::Error);
            }
        }
        set_busy.update(|ids| {
            ids.remove(&attendee_id);
        });
    });
}

/// Flip an attendee between In-Person and Online.
pub(super) fn spawn_participation_switch(
    attendee_id: String,
    event_id: Option<String>,
    mode: &'static str,
    set_busy: WriteSignal<HashSet<String>>,
    notify: Notify,
) {
    set_busy.update(|ids| {
        ids.insert(attendee_id.clone());
    });
    leptos::task::spawn_local(async move {
        match api::update_participation_type(&attendee_id, event_id.as_deref(), mode).await {
            Ok(_) => {
                api::invalidate_attendee_cache();
                notify.reload();
                notify.toast(&format!("Switched to {mode}"), ToastType::Success);
            }
            Err(e) => {
                notify.toast(&format!("Switch failed: {e}"), ToastType::Error);
            }
        }
        set_busy.update(|ids| {
            ids.remove(&attendee_id);
        });
    });
}

/// Delete an attendee from this event.
pub(super) fn spawn_delete_attendee(
    attendee_id: String,
    event_id: Option<String>,
    set_deleting: WriteSignal<HashSet<String>>,
    notify: Notify,
) {
    set_deleting.update(|ids| {
        ids.insert(attendee_id.clone());
    });
    leptos::task::spawn_local(async move {
        match api::delete_attendee(&attendee_id, event_id.as_deref()).await {
            Ok(()) => {
                notify.toast("Attendee deleted", ToastType::Success);
                api::invalidate_attendee_cache();
                notify.reload();
            }
            Err(e) => {
                notify.toast(&format!("Delete failed: {e}"), ToastType::Error);
            }
        }
        set_deleting.update(|ids| {
            ids.remove(&attendee_id);
        });
    });
}
